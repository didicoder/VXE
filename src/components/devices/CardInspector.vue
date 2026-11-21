<script setup lang="ts">
import { reactive, ref, watch } from 'vue';
import LineIcon from '../icons/LineIcon.vue';
import PillButton from '../PillButton.vue';
import { prepareCardImage } from '../../lib/cardImage';
import { pickCardImageFile } from '../../lib/ipc';
import { cardView } from '../../lib/deviceCard';
import type { DeviceConfig, DeviceStatus } from '../../types/device';

const props = withDefaults(
  defineProps<{
    device: DeviceConfig;
    status: DeviceStatus | null;
    /** 当前卡片图片；点缩略图等同于点「选择图片」。 */
    image?: string | null;
    /** 当前是否已有用户自选图片（区别于无图时的电量光环）。 */
    custom: boolean;
    /** 落盘失败的原因，由设置窗写回（空串 = 没问题）。 */
    error?: string;
    /** 正在保存图片。 */
    busy?: boolean;
  }>(),
  { image: null, error: '', busy: false },
);

const emit = defineEmits<{
  preview: [patch: Partial<DeviceConfig>];
  commit: [patch: Partial<DeviceConfig>];
  setImage: [dataUrl: string];
  setImageFromPath: [path: string];
  clearImage: [];
  reset: [];
  close: [];
}>();

interface Draft {
  brand: string;
  name: string;
  wm: string;
  ratio: number;
  dx: number;
  dy: number;
  rot: number;
  shadow: boolean;
  glow: boolean;
  link: boolean;
  ident: boolean;
  power: boolean;
}

const draft = reactive<Draft>({
  brand: '',
  name: '',
  wm: '',
  ratio: 80,
  dx: 0,
  dy: 0,
  rot: 0,
  shadow: true,
  glow: true,
  link: true,
  ident: true,
  power: true,
});

function fillDraft(): void {
  const next = cardView(props.device);
  draft.brand = next.brand;
  draft.name = next.name;
  draft.wm = next.wm;
  draft.ratio = next.ratio;
  draft.dx = next.dx;
  draft.dy = next.dy;
  draft.rot = next.rot;
  draft.shadow = next.shadow;
  draft.glow = next.glow;
  draft.link = next.fields.link;
  draft.ident = next.fields.ident;
  draft.power = next.fields.power;
}

watch(
  () => props.device.id,
  () => fillDraft(),
  { immediate: true },
);

/** 输入时只预览；失焦 / 松手 / 切换时提交，避免每按一次键写一次配置。 */
function previewText(patch: Partial<DeviceConfig>): void {
  emit('preview', patch);
}

function commitText(patch: Partial<DeviceConfig>): void {
  emit('commit', patch);
}

function onText(key: 'brand' | 'name' | 'wm', event: Event): void {
  const value = (event.target as HTMLInputElement).value;
  if (key === 'brand') {
    draft.brand = value;
    previewText({ card_brand: value });
  } else if (key === 'name') {
    draft.name = value;
    previewText({ card_name: value });
  } else {
    draft.wm = value;
    previewText({ card_wm: value });
  }
}

function onTextCommit(key: 'brand' | 'name' | 'wm'): void {
  if (key === 'brand') commitText({ card_brand: draft.brand });
  else if (key === 'name') commitText({ card_name: draft.name });
  else commitText({ card_wm: draft.wm });
}

function onSlider(key: 'ratio' | 'dx' | 'dy' | 'rot', event: Event): void {
  const value = Number((event.target as HTMLInputElement).value);
  if (key === 'ratio') {
    draft.ratio = value;
    previewText({ card_ratio: value });
  } else if (key === 'dx') {
    draft.dx = value;
    previewText({ card_dx: value });
  } else if (key === 'dy') {
    draft.dy = value;
    previewText({ card_dy: value });
  } else {
    draft.rot = value;
    previewText({ card_rot: value });
  }
}

function onSliderCommit(key: 'ratio' | 'dx' | 'dy' | 'rot'): void {
  if (key === 'ratio') commitText({ card_ratio: draft.ratio });
  else if (key === 'dx') commitText({ card_dx: draft.dx });
  else if (key === 'dy') commitText({ card_dy: draft.dy });
  else commitText({ card_rot: draft.rot });
}

/** 偏移归零，图片回到卡片正中心。 */
function centerImage(): void {
  draft.dx = 0;
  draft.dy = 0;
  commitText({ card_dx: 0, card_dy: 0 });
}

/** 旋转快捷档：90° / 180°（自由角度用上面的滑杆）。 */
function setRot(value: number): void {
  draft.rot = value;
  commitText({ card_rot: value });
}

function onToggle(key: 'shadow' | 'glow' | 'link' | 'ident' | 'power', event: Event): void {
  const checked = (event.target as HTMLInputElement).checked;
  if (key === 'shadow' || key === 'glow') {
    if (key === 'shadow') draft.shadow = checked;
    else draft.glow = checked;
    emit('commit', key === 'shadow' ? { card_shadow: checked } : { card_glow: checked });
    return;
  }
  draft[key] = checked;
  emit('commit', {
    card_fields: {
      link: draft.link,
      ident: draft.ident,
      power: draft.power,
    },
  });
}

function onReset(): void {
  fillDraft();
  emit('reset');
}

// ---------- 图片 ----------

const fileInput = ref<HTMLInputElement | null>(null);
const imageStatus = ref('');
const reading = ref(false);

/**
 * 取图入口统一走这里：
 * 1. 先用 Win32 系统对话框（owner 挂主窗，必定出现在最前）→ 得到路径 → Rust 直读；
 * 2. 对话框不可用（非 Windows / 命令缺失）再退回 WebView 的 `<input type=file>`。
 */
async function pickImage(): Promise<void> {
  imageStatus.value = '';
  try {
    const path = await pickCardImageFile();
    if (path) {
      emit('setImageFromPath', path);
      return;
    }
    if (path === null) return; // 用户取消
  } catch {
    // 退回下面的 webview 文件选择
  }
  const input =
    fileInput.value ?? (document.getElementById('card-image-input') as HTMLInputElement | null);
  if (!input) return;
  // 每次都清空：同一个文件连选两次也要能触发 change。
  input.value = '';
  input.click();
}

/** WebView 的 File 对象拿不到磁盘路径（api 包也没有 getPathForFile），只能走前端管线。 */
async function onFile(event: Event): Promise<void> {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0] ?? null;
  if (!file) return;
  imageStatus.value = '';
  reading.value = true;
  try {
    emit('setImage', await prepareCardImage(file));
  } catch (cause) {
    imageStatus.value = `${cause instanceof Error ? cause.message : '读取文件失败'}；请重试，或把图片直接拖到卡片上`;
  } finally {
    reading.value = false;
    input.value = '';
  }
}
</script>

<template>
  <aside class="insp" aria-label="卡片设置">
    <div class="insp__hd" data-tauri-drag-region>
      <div data-tauri-drag-region>
        <div class="insp__t" data-tauri-drag-region>卡片设置</div>
        <div class="insp__s" data-tauri-drag-region>{{ device.label }}</div>
      </div>
      <div class="insp__hdbtns">
        <button type="button" class="insp__x" aria-label="关闭" @click="emit('close')">
          <LineIcon name="close" :size="16" :stroke="2.2" />
        </button>
      </div>
    </div>

    <div class="insp__body fade-scroll">
      <div class="sec">
        <p class="lb">卡片图片</p>
        <button
          type="button"
          class="thumb"
          title="点击选择图片"
          aria-label="点击选择卡片图片"
          :disabled="reading || busy"
          @click="pickImage"
        >
          <img v-if="image" :src="image" alt="">
          <span v-else class="thumb__empty">暂无图片</span>
        </button>
        <div class="row row--single">
          <PillButton variant="primary" :disabled="reading || busy" @click="pickImage">
            {{ reading ? '正在读取…' : busy ? '正在保存…' : '选择图片' }}
          </PillButton>
        </div>
        <div v-if="custom" class="row row--single row--gap">
          <PillButton size="sm" variant="soft" @click="emit('clearImage')">
            清除自选图片
          </PillButton>
        </div>
        <p v-if="error" class="hint hint--bad">{{ error }}</p>
        <p v-else-if="imageStatus" class="hint hint--bad">{{ imageStatus }}</p>
        <input
          id="card-image-input"
          ref="fileInput"
          class="file"
          type="file"
          accept="image/*"
          aria-label="选择卡片图片"
          @change="onFile"
        >
      </div>

      <div class="sec">
        <p class="lb">位置与大小</p>
        <div class="slider">
          <div class="slider__hd"><span>大小比例</span><b>{{ draft.ratio }}%</b></div>
          <input
            type="range"
            min="1"
            max="100"
            :value="draft.ratio"
            @input="onSlider('ratio', $event)"
            @change="onSliderCommit('ratio')"
          >
        </div>
        <div class="slider">
          <div class="slider__hd"><span>水平偏移</span><b>{{ draft.dx }}</b></div>
          <input
            type="range"
            min="-40"
            max="40"
            :value="draft.dx"
            @input="onSlider('dx', $event)"
            @change="onSliderCommit('dx')"
          >
        </div>
        <div class="slider">
          <div class="slider__hd"><span>垂直偏移</span><b>{{ draft.dy }}</b></div>
          <input
            type="range"
            min="-40"
            max="40"
            :value="draft.dy"
            @input="onSlider('dy', $event)"
            @change="onSliderCommit('dy')"
          >
        </div>
        <div class="row row--single row--gap">
          <PillButton size="sm" variant="soft" @click="centerImage">图片居中</PillButton>
        </div>
        <div class="slider">
          <div class="slider__hd"><span>旋转角度</span><b>{{ draft.rot }}°</b></div>
          <input
            type="range"
            min="-180"
            max="180"
            step="1"
            :value="draft.rot"
            @input="onSlider('rot', $event)"
            @change="onSliderCommit('rot')"
          >
        </div>
        <div class="row row--gap rot-presets">
          <PillButton size="sm" variant="soft" @click="setRot(0)">0°</PillButton>
          <PillButton size="sm" variant="soft" @click="setRot(90)">90°</PillButton>
          <PillButton size="sm" variant="soft" @click="setRot(180)">180°</PillButton>
        </div>
        <div class="toggles">
          <label class="tg">
            <input
              type="checkbox"
              :checked="draft.shadow"
              @change="onToggle('shadow', $event)"
            ><i></i>图片投影
          </label>
          <label class="tg">
            <input
              type="checkbox"
              :checked="draft.glow"
              @change="onToggle('glow', $event)"
            ><i></i>底部光晕
          </label>
        </div>
      </div>

      <div class="sec">
        <p class="lb">卡片文字</p>
        <div class="field">
          <label for="card-brand">品牌</label>
          <input
            id="card-brand"
            type="text"
            spellcheck="false"
            autocomplete="off"
            :value="draft.brand"
            @input="onText('brand', $event)"
            @change="onTextCommit('brand')"
          >
        </div>
        <div class="field">
          <label for="card-name">设备名称</label>
          <input
            id="card-name"
            type="text"
            spellcheck="false"
            autocomplete="off"
            :value="draft.name"
            @input="onText('name', $event)"
            @change="onTextCommit('name')"
          >
        </div>
        <div class="field">
          <label for="card-wm">背景文字</label>
          <input
            id="card-wm"
            type="text"
            spellcheck="false"
            autocomplete="off"
            :value="draft.wm"
            @input="onText('wm', $event)"
            @change="onTextCommit('wm')"
          >
        </div>
      </div>

      <div class="sec">
        <p class="lb">显示字段</p>
        <div class="toggles">
          <label class="tg">
            <input
              type="checkbox"
              :checked="draft.link"
              @change="onToggle('link', $event)"
            ><i></i>连接方式
          </label>
          <label class="tg">
            <input
              type="checkbox"
              :checked="draft.ident"
              @change="onToggle('ident', $event)"
            ><i></i>设备标识
          </label>
          <label class="tg">
            <input
              type="checkbox"
              :checked="draft.power"
              @change="onToggle('power', $event)"
            ><i></i>供电状态
          </label>
        </div>
      </div>

      <div class="sec sec--last">
        <div class="row row--single">
          <PillButton size="sm" variant="soft" @click="onReset">恢复默认</PillButton>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.insp {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
}

.insp__hd {
  cursor: grab;
  user-select: none;
}

@keyframes insp-in {
  from {
    opacity: 0;
    transform: translateY(8px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.insp__hd {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 20px 22px 16px;
  border-bottom: 1px solid var(--line);
}

.insp__t {
  font-size: 21px;
  font-weight: 700;
  line-height: 1.2;
}

.insp__s {
  margin-top: 4px;
  overflow: hidden;
  color: var(--ink-2);
  font-size: 14.5px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.insp__hdbtns {
  display: flex;
  align-items: center;
  gap: 2px;
  margin-left: auto;
}

.insp__x {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: 9px;
  background: transparent;
  color: var(--ink-3);
  cursor: pointer;
}

.insp__x:hover {
  background: var(--ctrl-bg);
  color: var(--ink);
}

.insp__body {
  flex: 1;
  min-height: 0;
  padding: 8px 22px 24px;
  overflow-y: auto;
}

.sec {
  padding: 16px 0;
  border-bottom: 1px solid var(--line);
}

.sec--last {
  border-bottom: 0;
}

.lb {
  margin: 0 0 12px;
  color: var(--ink);
  font-size: 15px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

/* 缩略图预览：点图等同于点「选择图片」 */
.thumb {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 112px;
  overflow: hidden;
  padding: 0;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: repeating-conic-gradient(rgb(140 150 170 / 12%) 0% 25%, transparent 0% 50%) 0 0 / 16px 16px;
  box-shadow:
    0 10px 28px -18px rgb(20 20 45 / 55%),
    inset 0 1px 0 rgb(255 255 255 / 12%);
  cursor: pointer;
  transition:
    border-color 0.18s ease,
    background-color 0.18s ease,
    box-shadow 0.18s ease;
}

.thumb:hover {
  border-color: var(--ctrl-line-hover);
  background-color: rgb(127 127 127 / 6%);
}

.thumb:disabled {
  opacity: 0.6;
  cursor: default;
}

.thumb img {
  max-width: 88%;
  max-height: 88%;
  object-fit: contain;
  filter: drop-shadow(0 8px 12px rgb(20 20 45 / 18%));
}

.thumb__empty {
  color: var(--ink-3);
  font-size: 13px;
}

.row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 12px;
}

.row--single {
  display: block;
}

.row--gap {
  margin-top: 6px;
}

/* 胶囊自带底部光晕/阴影，浮在单层玻璃上也能看清 */
.insp :deep(.pill) {
  border-color: color-mix(in srgb, var(--ink-3) 26%, transparent);
  box-shadow:
    0 10px 22px -12px rgb(20 20 45 / 42%),
    inset 0 1px 0 rgb(255 255 255 / 14%);
}

.insp :deep(.pill--primary) {
  border-color: transparent;
  box-shadow:
    0 12px 26px -12px color-mix(in srgb, var(--accent) 58%, transparent),
    inset 0 1px 0 rgb(255 255 255 / 20%);
}

/* 设置页同款胶囊：主操作铺满，快捷档三等分 */
.row--single :deep(.pill) {
  width: 100%;
}

.rot-presets :deep(.pill) {
  flex: 1;
  min-width: 0;
}

.hint {
  margin-top: 10px;
  padding: 9px 12px;
  border-radius: 10px;
  background: rgb(127 127 127 / 8%);
  box-shadow:
    0 8px 18px -16px rgb(20 20 45 / 32%),
    inset 0 1px 0 rgb(255 255 255 / 8%);
  color: var(--ink-2);
  font-size: 13px;
  line-height: 1.6;
}

.hint--bad {
  background: var(--danger-bg);
  color: var(--red-text);
}

/* 文件框：不能用 display:none —— 部分 WebView2 版本里对无布局元素
 * 调 .click() 不会弹系统选图框，用 1px 透明盒子最稳。 */
.file {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  border: 0;
  opacity: 0;
  overflow: hidden;
  clip-path: inset(50%);
  pointer-events: none;
}

.field {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 12px;
}

.field label {
  flex: none;
  width: 72px;
  color: var(--ink-2);
  font-size: 14px;
}

.field input[type="text"] {
  flex: 1;
  min-width: 0;
  height: 36px;
  padding: 0 12px;
  border: 1px solid var(--ctrl-line);
  border-radius: 10px;
  background: var(--ctrl-bg);
  box-shadow: 0 8px 18px -14px rgb(20 20 45 / 38%);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 14.5px;
  outline: none;
}

.field input[type="text"]:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 18%, transparent);
}

.slider {
  margin-top: 14px;
}

.slider__hd {
  display: flex;
  justify-content: space-between;
  margin-bottom: 7px;
  color: var(--ink-2);
  font-size: 14px;
}

.slider__hd b {
  color: var(--ink);
  font-variant-numeric: tabular-nums;
}

input[type="range"] {
  width: 100%;
  height: 4px;
  border-radius: 99px;
  outline: none;
  appearance: none;
  background: rgb(127 127 127 / 28%);
}

input[type="range"]::-webkit-slider-thumb {
  width: 15px;
  height: 15px;
  border: 1px solid rgb(25 28 44 / 18%);
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 2px 6px rgb(20 20 45 / 22%);
  cursor: pointer;
  appearance: none;
}

.toggles {
  display: flex;
  flex-wrap: wrap;
  gap: 16px;
  margin-top: 14px;
}

.tg {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  color: var(--ink-2);
  font-size: 14px;
  cursor: pointer;
}

.tg input {
  display: none;
}

.tg i {
  position: relative;
  display: block;
  width: 32px;
  height: 18px;
  border-radius: 99px;
  background: rgb(127 127 127 / 32%);
  transition: background 0.2s ease;
}

.tg i::after {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  box-shadow: 0 1px 3px rgb(20 20 45 / 30%);
  content: "";
  transition: transform 0.2s var(--ease);
}

.tg input:checked + i {
  background: var(--accent);
}

.tg input:checked + i::after {
  transform: translateX(14px);
}
</style>