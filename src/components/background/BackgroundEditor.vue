<script setup lang="ts">
/**
 * 背景编辑：图片 + 缩放/偏移/磨砂/暗化/旋转。
 *
 * 预览只把草稿发给 Rust 广播到所有窗口，编辑器保持打开；
 * 保存才写 background.dat 与配置；取消/关闭丢弃草稿回到已保存效果。
 * 旋转在预览/保存时用画布烤进图片（90° 步进），不改窗口渲染。
 */
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import PillButton from '../PillButton.vue';
import SegmentedControl from '../SegmentedControl.vue';
import StepperControl from '../StepperControl.vue';
import LineIcon from '../icons/LineIcon.vue';
import wallpaperUrl from '../../assets/wallpaper.jpg';
import { wallpaperGeometry } from '../../lib/wallpaperLayout';
import * as ipc from '../../lib/ipc';
import { trackWindowInitialization } from '../../lib/windowLifecycle';

/** 原图上限 8 MB；超过 MAX_SIDE 的图片在预览/保存时降采样，减轻渲染与内存压力。 */
const BG_MAX_BYTES = 8 * 1024 * 1024;
const MAX_SIDE = 3840;
const ROTATIONS = [
  { value: '0', label: '0°' },
  { value: '90', label: '90°' },
  { value: '180', label: '180°' },
  { value: '270', label: '270°' },
];

const image = ref<string | null>(null);
const ratio = ref(1);
const dx = ref(0);
const dy = ref(0);
const blur = ref(12);
const dim = ref(0);
const rotation = ref('0');
const busy = ref(false);
const ready = ref(false);
const status = ref('');
const fileInput = ref<HTMLInputElement | null>(null);
const bodyEl = ref<HTMLElement | null>(null);
const thumbEl = ref<HTMLElement | null>(null);
const thumbSize = ref<{ width: number; height: number } | null>(null);
const rotatedPreview = ref<string | null>(null);
const rotationKey = ref('');
const rotating = ref(false);
let rotationToken = 0;

const hasImage = computed(() => image.value != null);
/** 预览框跟随面板比例 1040×720；旋转后的草稿优先，缺省显示内置壁纸。 */
const thumbSrc = computed(() => rotatedPreview.value ?? image.value ?? wallpaperUrl);
/** 与主窗同一套几何：整张图 cover×缩放 排版，平移在图片内部进行。 */
const thumbImgStyle = computed<Record<string, string>>(() => {
  const box = thumbEl.value;
  const size = thumbSize.value;
  if (!box || !size) {
    const hidden: Record<string, string> = { opacity: '0' };
    return hidden;
  }
  const g = wallpaperGeometry(
    box.clientWidth,
    box.clientHeight,
    size.width,
    size.height,
    ratio.value,
    dx.value,
    dy.value,
  );
  const filter = blur.value > 0 ? `blur(${(blur.value * 0.34).toFixed(1)}px)` : 'none';
  return {
    width: `${g.width}px`,
    height: `${g.height}px`,
    left: `${g.left}px`,
    top: `${g.top}px`,
    filter,
    opacity: '1',
  };
});

// 预览框拖拽：只平移画面，不改缩放；超出图片范围的部分由主题白/黑填充
let dragOrigin: { x: number; y: number; dx: number; dy: number } | null = null;
const dragging = ref(false);

function clampOffset(value: number): number {
  return Math.min(50, Math.max(-50, Math.round(value)));
}

function onThumbPointerDown(event: PointerEvent): void {
  if (event.button !== 0) return;
  dragOrigin = { x: event.clientX, y: event.clientY, dx: dx.value, dy: dy.value };
  dragging.value = true;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  event.preventDefault();
}

function onThumbPointerMove(event: PointerEvent): void {
  if (!dragOrigin) return;
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  dx.value = clampOffset(dragOrigin.dx + ((event.clientX - dragOrigin.x) / rect.width) * 100);
  dy.value = clampOffset(dragOrigin.dy + ((event.clientY - dragOrigin.y) / rect.height) * 100);
}

function endThumbDrag(event: PointerEvent): void {
  if (!dragOrigin) return;
  dragOrigin = null;
  dragging.value = false;
  const target = event.currentTarget as HTMLElement;
  if (target.hasPointerCapture(event.pointerId)) target.releasePointerCapture(event.pointerId);
}

const formatRatio = (value: number): string => `${value.toFixed(2)}×`;
const formatPercent = (value: number): string => `${value}%`;
const formatBlur = (value: number): string => `${value}px`;

function errorText(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

async function load(): Promise<void> {
  try {
    const state = await ipc.getBackgroundState();
    image.value = state.image;
    ratio.value = state.ratio;
    dx.value = state.dx;
    dy.value = state.dy;
    blur.value = state.blur;
    dim.value = state.dim;
    rotation.value = '0';
    dx.value = clampOffset(dx.value);
    dy.value = clampOffset(dy.value);
    status.value = '';
  } catch (cause) {
    status.value = `读取失败：${errorText(cause)}`;
  }
  ready.value = true;
  await nextTick();
  bodyEl.value?.scrollTo({ top: 0 });
}

function pickImage(): void {
  status.value = '';
  fileInput.value?.click();
}

async function onFile(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0] ?? null;
  input.value = '';
  if (!file) return;
  if (!file.type.startsWith('image/')) {
    status.value = '只支持图片文件';
    return;
  }
  if (file.size > BG_MAX_BYTES) {
    status.value = '图片过大，请换一张不超过 8 MB 的';
    return;
  }
  try {
    const dataUrl = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = () => reject(new Error('读取文件失败'));
      reader.readAsDataURL(file);
    });
    image.value = dataUrl;
    rotation.value = '0';
    status.value = '';
  } catch (cause) {
    status.value = errorText(cause);
  }
}

function clearWallpaper(): void {
  image.value = null;
  ratio.value = 1;
  dx.value = 0;
  dy.value = 0;
  blur.value = 12;
  dim.value = 0;
  rotation.value = '0';
  status.value = '已清除，预览后保存生效';
}

function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error('图片旋转失败'));
    img.src = src;
  });
}

/** 90° 步进旋转；最长边压到 4096 内，重编码 JPEG 控制体积。 */
async function rotatedDataUrl(src: string, angle: number): Promise<string> {
  const img = await loadImage(src);
  const radians = (angle * Math.PI) / 180;
  const quarter = angle % 180 === 0;
  const sourceW = img.naturalWidth;
  const sourceH = img.naturalHeight;
  const outW = quarter ? sourceW : sourceH;
  const outH = quarter ? sourceH : sourceW;
  const shrink = Math.min(1, MAX_SIDE / Math.max(outW, outH));
  const canvas = document.createElement('canvas');
  canvas.width = Math.round(outW * shrink);
  canvas.height = Math.round(outH * shrink);
  const ctx = canvas.getContext('2d');
  if (!ctx) throw new Error('无法创建画布');
  ctx.translate(canvas.width / 2, canvas.height / 2);
  ctx.rotate(radians);
  ctx.drawImage(
    img,
    (-sourceW * shrink) / 2,
    (-sourceH * shrink) / 2,
    sourceW * shrink,
    sourceH * shrink,
  );
  return canvas.toDataURL('image/jpeg', 0.92);
}

async function effectiveImage(): Promise<string | null> {
  const src = image.value;
  if (!src) return null;
  const angle = Number(rotation.value);
  const probe = await loadImage(src);
  const oversized = Math.max(probe.naturalWidth, probe.naturalHeight) > MAX_SIDE;
  if (!angle && !oversized) return src;
  if (angle) {
    const key = `${src.length}:${angle}`;
    if (rotatedPreview.value && rotationKey.value === key) return rotatedPreview.value;
  }
  return rotatedDataUrl(src, angle);
}

// 旋转一改就重出预览图，保证缩略图和最终效果一致
watch(
  [image, rotation],
  async () => {
    const token = ++rotationToken;
    rotatedPreview.value = null;
    rotationKey.value = '';
    const angle = Number(rotation.value);
    if (!image.value || !angle) {
      rotating.value = false;
      return;
    }
    rotating.value = true;
    try {
      const result = await rotatedDataUrl(image.value, angle);
      if (token === rotationToken) {
        rotatedPreview.value = result;
        rotationKey.value = `${image.value.length}:${rotation.value}`;
      }
    } catch (cause) {
      if (token === rotationToken) status.value = errorText(cause);
    } finally {
      if (token === rotationToken) rotating.value = false;
    }
  },
  { immediate: true },
);

// 预览图尺寸变化（换图/旋转）后重新排版
watch(
  thumbSrc,
  async (src) => {
    try {
      const size = await new Promise<{ width: number; height: number }>((resolve, reject) => {
        const probe = new Image();
        probe.onload = () => resolve({ width: probe.naturalWidth, height: probe.naturalHeight });
        probe.onerror = () => reject(new Error('图片读取失败'));
        probe.src = src;
      });
      thumbSize.value = size;
    } catch (cause) {
      thumbSize.value = null;
      status.value = errorText(cause);
    }
  },
  { immediate: true },
);

function draft(rotated: string | null): ipc.BackgroundState {
  return {
    image: rotated,
    ratio: ratio.value,
    dx: dx.value,
    dy: dy.value,
    blur: blur.value,
    dim: dim.value,
  };
}

async function preview(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  try {
    await ipc.previewBackground(draft(await effectiveImage()));
    status.value = '预览中（未保存）';
  } catch (cause) {
    status.value = `预览失败：${errorText(cause)}`;
  } finally {
    busy.value = false;
  }
}

async function save(): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  try {
    const rotated = await effectiveImage();
    await ipc.commitBackground(draft(rotated));
    image.value = rotated;
    rotation.value = '0';
    status.value = '已保存';
    try {
      await ipc.hideBackgroundEditor();
    } catch {
      // 非 Tauri 环境：保存成功后保持窗口
    }
  } catch (cause) {
    status.value = `保存失败：${errorText(cause)}`;
  } finally {
    busy.value = false;
  }
}

let closing = false;

async function cancel(): Promise<void> {
  if (busy.value || closing) return;
  closing = true;
  try {
    try {
      await ipc.cancelBackgroundPreview();
    } catch {
      // 忽略：关窗优先
    }
    try {
      await ipc.hideBackgroundEditor();
    } catch {
      // 非 Tauri 环境
    }
  } finally {
    closing = false;
  }
}

let unlisten: UnlistenFn | null = null;
let unlistenClose: UnlistenFn | null = null;

onMounted(() => trackWindowInitialization((async () => {
  try {
    unlistenClose = await getCurrentWindow().onCloseRequested(async (event) => {
      event.preventDefault();
      await cancel();
    });
  } catch {
    // 浏览器预览没有原生窗口
  }
  await load();
  try {
    unlisten = await listen('bg-editor-open', () => {
      void load();
    });
  } catch {
    // 非 Tauri 环境没有事件总线
  }
})()));

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
  unlistenClose?.();
  unlistenClose = null;
});
</script>

<template>
  <aside class="bgi" aria-label="自定义背景">
    <header class="bgi__hd" data-tauri-drag-region>
      <div data-tauri-drag-region>
        <div class="bgi__title" data-tauri-drag-region>自定义背景</div>
        <div class="bgi__sub" data-tauri-drag-region>调整后可先预览，窗口保持打开</div>
      </div>
      <button type="button" class="bgi__x" aria-label="关闭" @click="cancel">
        <LineIcon name="close" :size="16" :stroke="2.2" />
      </button>
    </header>

    <div class="bgi__preview">
      <p class="lb">背景图片</p>
      <div
        ref="thumbEl"
        class="thumb"
        :class="{ 'thumb--empty': !hasImage, 'thumb--dragging': dragging }"
        @pointerdown="onThumbPointerDown"
        @pointermove="onThumbPointerMove"
        @pointerup="endThumbDrag"
        @pointercancel="endThumbDrag"
      >
        <img class="thumb__wp" :src="thumbSrc" :style="thumbImgStyle" alt="">
        <i v-if="hasImage" class="thumb__dim" :style="{ opacity: dim / 100 }" />
        <span v-if="!hasImage" class="thumb__tag">内置壁纸</span>
        <span class="thumb__hint">拖动预览可调整位置</span>
      </div>
      <div class="actions">
        <input
          ref="fileInput"
          class="file"
          type="file"
          accept="image/*"
          aria-label="选择背景图片"
          @change="onFile"
        >
        <PillButton size="sm" variant="soft" :disabled="busy" @click="pickImage">
          选择图片
        </PillButton>
        <PillButton size="sm" variant="soft" :disabled="busy || !hasImage" @click="clearWallpaper">
          清除壁纸
        </PillButton>
      </div>
    </div>

    <div ref="bodyEl" class="bgi__body fade-scroll">
      <section class="sec">
        <p class="lb">画面</p>
        <div class="ctrl">
          <span class="ctrl__label">缩放</span>
          <StepperControl
            :model-value="ratio"
            :min="1"
            :max="3"
            :step="0.05"
            :format="formatRatio"
            @update:model-value="ratio = $event"
          />
        </div>
        <div class="ctrl">
          <span class="ctrl__label">水平偏移</span>
          <StepperControl
            :model-value="dx"
            :min="-50"
            :max="50"
            :step="1"
            :format="formatPercent"
            @update:model-value="dx = $event"
          />
        </div>
        <div class="ctrl">
          <span class="ctrl__label">垂直偏移</span>
          <StepperControl
            :model-value="dy"
            :min="-50"
            :max="50"
            :step="1"
            :format="formatPercent"
            @update:model-value="dy = $event"
          />
        </div>
        <div class="ctrl">
          <span class="ctrl__label">磨砂强度</span>
          <StepperControl
            :model-value="blur"
            :min="0"
            :max="24"
            :step="1"
            :format="formatBlur"
            @update:model-value="blur = $event"
          />
        </div>
        <div class="ctrl">
          <span class="ctrl__label">暗化</span>
          <StepperControl
            :model-value="dim"
            :min="0"
            :max="60"
            :step="5"
            :format="formatPercent"
            @update:model-value="dim = $event"
          />
        </div>
        <div class="ctrl ctrl--rot">
          <span class="ctrl__label">旋转</span>
          <SegmentedControl
            v-if="hasImage"
            :model-value="rotation"
            :options="ROTATIONS"
            @update:model-value="rotation = $event"
          />
          <span v-else class="ctrl__hint">选择图片后可旋转</span>
        </div>
      </section>
    </div>

    <footer class="bgi__ft">
      <span v-if="status" class="bgi__status" :title="status">{{ status }}</span>
      <div class="bgi__btns">
        <PillButton size="sm" variant="soft" :disabled="busy" @click="cancel">取消</PillButton>
        <PillButton size="sm" variant="soft" :disabled="busy || rotating || !ready" @click="preview">预览</PillButton>
        <PillButton size="sm" variant="primary" :disabled="busy || rotating || !ready" @click="save">保存</PillButton>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.bgi {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
}

.bgi__hd {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex: none;
  height: 64px;
  padding: 0 18px 0 24px;
  border-bottom: 1px solid var(--line);
}

.bgi__title {
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 20px;
}

.bgi__sub {
  margin-top: 3px;
  color: var(--ink-3);
  font-size: 12.5px;
  line-height: 16px;
}

.bgi__x {
  display: grid;
  flex: none;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  color: var(--ink-3);
  cursor: pointer;
}

.bgi__x:hover {
  color: var(--ink);
}

.bgi__body {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 0 22px 10px;
}

/* 预览固定：参数区单独滚动，下滑时仍能看到画面 */
.bgi__preview {
  flex: none;
  padding: 8px 22px 6px;
}

.sec {
  padding: 8px 0 12px;
  border-bottom: 1px solid var(--line);
}

.sec:last-child {
  border-bottom: 0;
}

.lb {
  margin: 0 0 10px;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  line-height: 16px;
}

/* 预览比例与主面板 1040×720 一致；平移走 background-position，
   永远在图片内部移动，不露出空白底。 */
.thumb {
  position: relative;
  display: grid;
  width: 288px;
  max-width: 100%;
  aspect-ratio: 1040 / 720;
  margin: 0 auto;
  place-items: center;
  overflow: hidden;
  border-radius: var(--r-tile);
  background: var(--wallpaper-fill);
  cursor: grab;
  touch-action: none;
  user-select: none;
}

.thumb--dragging {
  cursor: grabbing;
}

.thumb__wp {
  position: absolute;
  max-width: none;
  max-height: none;
  user-select: none;
  -webkit-user-drag: none;
}

.thumb__dim {
  position: absolute;
  inset: 0;
  background: #000;
  pointer-events: none;
}

.thumb__tag {
  position: absolute;
  bottom: 8px;
  left: 10px;
  padding: 2px 8px;
  border-radius: var(--r-chip);
  background: rgb(0 0 0 / 32%);
  color: #fff;
  font-size: 11px;
  line-height: 15px;
}

.thumb__hint {
  position: absolute;
  right: 10px;
  bottom: 8px;
  padding: 2px 8px;
  border-radius: var(--r-chip);
  background: rgb(0 0 0 / 26%);
  color: #fff;
  font-size: 11px;
  line-height: 15px;
  opacity: 0;
  transition: opacity var(--speed) var(--ease);
  pointer-events: none;
}

.thumb:hover .thumb__hint,
.thumb--dragging .thumb__hint {
  opacity: 1;
}

.thumb--empty {
  border: 1px dashed var(--card-line);
}

.actions {
  display: flex;
  justify-content: center;
  gap: 12px;
  margin-top: 14px;
}

.file {
  display: none;
}

.ctrl {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  min-height: 52px;
}

.ctrl + .ctrl {
  margin-top: 2px;
}

.ctrl--rot {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
  padding: 8px 0 2px;
}

.ctrl--rot .ctrl__label {
  text-align: left;
}

/* 度数选择框居中，标题保持左对齐 */
.ctrl--rot :deep(.segmented) {
  align-self: center;
}

.ctrl__label {
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 700;
}

.ctrl__hint {
  color: var(--ink-3);
  font-size: 12.5px;
  line-height: 16px;
}

.bgi__ft {
  display: flex;
  flex-direction: column;
  align-items: stretch;
  flex: none;
  gap: 10px;
  padding: 10px 22px 14px;
  border-top: 1px solid var(--line);
}

.bgi__status {
  overflow: hidden;
  color: var(--ink-3);
  font-size: 12px;
  line-height: 16px;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bgi__btns {
  display: flex;
  gap: 10px;
}

/* 三枚胶囊等分整宽，不再挤压溢出 */
.bgi__btns :deep(.pill) {
  flex: 1;
  min-width: 0;
}
</style>