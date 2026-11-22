<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import BatteryOrb from '../BatteryOrb.vue';
import LineIcon from '../icons/LineIcon.vue';
import TrashIcon from '../icons/TrashIcon.vue';
import { bandColor, bandOf } from '../../lib/bands';
import { colorHex } from '../../lib/colors';
import { cardImageBox } from '../../lib/photoLayout';
import { rightAngle, rotatedCardImages } from '../../lib/rotatedCardImage';
import {
  cardChip,
  cardLink,
  cardLow,
  cardPower,
  cardView,
} from '../../lib/deviceCard';
import { levelOf } from '../../lib/state';
import { percent } from '../../lib/format';
import { deviceIdent } from '../../lib/deviceIdentity';
import type { BandColors, ColorKey } from '../../types/battery';
import type { DeviceConfig, DeviceStatus } from '../../types/device';

const props = withDefaults(
  defineProps<{
    device: DeviceConfig;
    status: DeviceStatus | null;
    image: string | null;
    good: number;
    warn: number;
    bandColors: BandColors;
    mode: 'overview' | 'devices';
    editing?: boolean;
    active?: boolean;
    /** 删除按钮已点过一次，等待二次确认。 */
    pendingDelete?: boolean;
  }>(),
  { editing: false, active: false, pendingDelete: false },
);

const emit = defineEmits<{
  open: [];
  settings: [];
  delete: [];
}>();

/** 悬停 / 键盘聚焦时播放 trash-v2 掀盖动画，移出倒放。 */
const trashHover = ref(false);

const view = computed(() => cardView(props.device));
const display = computed(() =>
  props.status ? levelOf(props.status) : { level: null, live: false },
);
const level = computed(() => display.value.level);
const low = computed(() => cardLow(props.status, props.good, props.warn));
const pctText = computed(() => percent(level.value));

const band = computed(() => bandOf(level.value, props.good, props.warn));
const colorKey = computed<ColorKey>(() =>
  bandColor(band.value, props.bandColors, props.device.colors),
);
const battColor = computed(() =>
  level.value == null ? 'var(--faint)' : bandToken(colorKey.value),
);
/** 没有自选图时中部展示的电量光环：无读数走 faint 静止态。 */
const orbTone = computed(() => (level.value == null ? 'none' : band.value));

function bandToken(key: ColorKey): string {
  if (key === 'green') return 'var(--good)';
  if (key === 'blue') return 'var(--mid)';
  if (key === 'red') return 'var(--low)';
  return colorHex(key);
}

const fillWidth = computed(() =>
  level.value == null ? 1.2 : Math.max(1.2, 18 * (level.value / 100)).toFixed(2),
);
const chip = computed(() => cardChip(props.status, level.value, low.value));
const ident = computed(() => deviceIdent(props.device));
const link = computed(() => cardLink(props.device, props.status));
const power = computed(() => cardPower(props.status));

/** 竖排水印：先设 100px 量出字串长度，再按卡片宽高回推字号（长短词都不撑破）。 */
const cardEl = ref<HTMLElement | null>(null);
const wmEl = ref<HTMLElement | null>(null);

/** 卡片实测尺寸与图片固有尺寸：用于按卡片长边 80% 归一化图片。 */
const cardSize = ref({ w: 0, h: 0 });
const natural = ref({ w: 0, h: 0 });

/** 图片按“卡片长边占比”定尺寸；旋转外接框再夹到卡片内，永不超出。 */
const imageBox = computed(() =>
  cardImageBox(
    natural.value.w,
    natural.value.h,
    cardSize.value.w,
    cardSize.value.h,
    view.value.rot,
    view.value.ratio,
  ),
);

/** 0/90/180/270 共享原尺寸画布预旋转，避免先缩放再旋转造成二次重采样。 */
const preRot = computed(() => rightAngle(view.value.rot));
const rotatedSrc = ref<string | null>(null);

watch(
  [() => props.image, preRot],
  ([src, angle], _previous, onCleanup) => {
    rotatedSrc.value = null;
    if (!src || angle === 0) return;
    const lease = rotatedCardImages.acquire(src, angle);
    let current = true;
    onCleanup(() => {
      current = false;
      lease.release();
    });
    void lease.ready.then((url) => {
      if (current) rotatedSrc.value = url;
    }).catch(() => {
      if (current) rotatedSrc.value = null;
    });
  },
  { immediate: true, flush: 'sync' },
);

/** 画布尚未产出时先退回 CSS 旋转，避免出现一帧尺寸互换的畸变。 */
const appliedRot = computed(() =>
  preRot.value && rotatedSrc.value ? preRot.value : 0,
);
const displayImage = computed(() =>
  appliedRot.value ? rotatedSrc.value : props.image,
);
/** 90/270 预旋转后图片自身宽高互换；布局外接框仍按原始方向计算。 */
const displayBox = computed(() => {
  const box = imageBox.value;
  if (!box) return null;
  return appliedRot.value === 90 || appliedRot.value === 270
    ? { width: box.height, height: box.width }
    : box;
});
const displayRot = computed(() => (appliedRot.value ? 0 : view.value.rot));

// 原图尺寸独立预载：预旋转图加载后自然宽高已互换，不能再拿它当布局基准。
watch(
  () => props.image,
  (src, _previous, onCleanup) => {
    natural.value = { w: 0, h: 0 };
    if (!src) return;
    const probe = new Image();
    let current = true;
    onCleanup(() => {
      current = false;
      probe.onload = null;
      probe.onerror = null;
      probe.src = '';
    });
    probe.onload = () => {
      if (!current) return;
      natural.value = { w: probe.naturalWidth, h: probe.naturalHeight };
      void nextTick(fitWm);
    };
    probe.src = src;
  },
  { immediate: true },
);

function measureCard(): void {
  const card = cardEl.value;
  if (!card) return;
  cardSize.value = { w: card.clientWidth, h: card.clientHeight };
  fitWm();
}

function fitWm(): void {
  const card = cardEl.value;
  const wm = wmEl.value;
  if (!card || !wm) return;
  const width = card.clientWidth;
  const height = card.clientHeight;
  if (!width || !height) return;
  wm.style.fontSize = '100px';
  const run = wm.offsetHeight || 100;
  const size = width * 0.22;
  const cap = ((height * 0.76) * 100) / run;
  wm.style.fontSize = `${Math.min(size, cap).toFixed(2)}px`;
}

let observer: ResizeObserver | null = null;

onMounted(() => {
  if (typeof ResizeObserver !== 'undefined' && cardEl.value) {
    observer = new ResizeObserver(() => measureCard());
    observer.observe(cardEl.value);
  }
  void nextTick(measureCard);
});

onBeforeUnmount(() => {
  observer?.disconnect();
  observer = null;
});

watch(
  () => [view.value.wm, props.mode],
  () => {
    void nextTick(fitWm);
  },
);

</script>

<template>
  <article
    ref="cardEl"
    class="pcard"
    :class="[
      `pcard--${mode}`,
      {
        'hide-link': !view.fields.link,
        'hide-ident': !view.fields.ident,
        'hide-power': !view.fields.power,
        'is-active': active,
        'is-editing': editing,
      },
    ]"
    :style="{
      '--img-w': displayBox ? `${displayBox.width}px` : '100%',
      '--img-h': displayBox ? `${displayBox.height}px` : '100%',
      '--dx': `${view.dx}%`,
      '--dy': `${view.dy}%`,
      '--rot': `${displayRot}deg`,
      '--batt': battColor,
    }"
    data-drag-item
    @click.stop="emit('open')"
  >
    <button
      v-if="!editing"
      type="button"
      class="pcard__grip"
      aria-label="拖动排序"
      title="按住拖动排序"
      @click.stop
    >
      <LineIcon name="move" :size="15" :stroke="1.5" />
    </button>

    <button
      v-if="editing"
      type="button"
      class="pcard__del"
      :class="{ 'is-pending': pendingDelete }"
      :aria-label="pendingDelete ? '再点一次确认删除' : '删除卡片'"
      :title="pendingDelete ? '再点一次确认删除' : '删除卡片'"
      @mouseenter="trashHover = true"
      @mouseleave="trashHover = false"
      @focus="trashHover = true"
      @blur="trashHover = false"
      @click.stop="emit('delete')"
    >
      <TrashIcon :size="16" :active="trashHover || pendingDelete" />
    </button>

    <button
      v-if="editing"
      type="button"
      class="pcard__cfg"
      aria-label="设置这张卡片"
      title="设置这张卡片"
      @click.stop="emit('settings')"
    >
      <LineIcon name="sliders" :size="16" :stroke="1.9" />
    </button>

    <span ref="wmEl" class="pcard__wm">{{ view.wm }}</span>

    <div class="pcard__top">
      <div class="pcard__brand">{{ view.brand }}</div>
      <div class="pcard__model">{{ view.name }}</div>
      <div class="pcard__batt" :class="low ? 'low' : 'ok'">
        <svg viewBox="0 0 30 16" aria-hidden="true">
          <rect
            x="1"
            y="1.6"
            width="24"
            height="12.8"
            rx="3.4"
            fill="none"
            stroke="currentColor"
            stroke-opacity=".45"
            stroke-width="1.4"
          />
          <rect
            x="26.2"
            y="5.2"
            width="2.6"
            height="5.6"
            rx="1.2"
            fill="currentColor"
            fill-opacity=".45"
          />
          <rect
            class="batt__fill"
            x="3"
            y="3.6"
            :width="fillWidth"
            height="8.8"
            rx="2"
          />
        </svg>
        <span class="batt__pct">{{ pctText }}</span>
        <span v-if="low" class="batt__tag">低电量</span>
      </div>
    </div>

    <div class="pcard__stage" aria-hidden="true" />
    <div class="pcard__media">
      <div class="pcard__glow" :class="{ off: !view.glow }" />
      <div class="pcard__frame">
        <img
          v-if="image"
          class="pcard__img"
          :class="{ 'no-shadow': !view.shadow }"
          :src="displayImage ?? undefined"
          :alt="view.name"
          draggable="false"
        >
        <BatteryOrb
          v-else
          class="pcard__orb"
          :tone="orbTone"
          :color="battColor"
        />
      </div>
    </div>

    <div class="pcard__bot">
      <div class="pcard__meta">
        <div class="mi mi--link"><span>连接方式</span><b><svg class="mi__icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M4.5 12.5a10.5 10.5 0 0 1 15 0" /><path d="M8 16a6 6 0 0 1 8 0" /><circle cx="12" cy="19.2" r="1.3" fill="currentColor" stroke="none" /></svg>{{ link }}</b></div>
        <div class="mi mi--ident"><span>设备标识</span><b class="mono">{{ ident }}</b></div>
        <div class="mi mi--power"><span>供电状态</span><b>{{ power }}</b></div>
      </div>
      <span class="chip" :class="chip.tone">{{ chip.text }}</span>
    </div>
  </article>
</template>

<style scoped>
/* 卡片本体：竖版海报卡（实物图 + 竖排水印 + 底部三行字段） */
.pcard {
  position: relative;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
  width: 100%;
  padding: 6.6% 7% 6.2%;
  overflow: hidden;
  isolation: isolate;
  border: 1px solid var(--card-line);
  border-radius: var(--r-panel);
  background: var(--pcard-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  box-shadow: var(--card-shadow);
  /* 裁进圆角：backdrop-filter 模糊层/外投影不再漏出灰色底边与圆角毛边 */
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  container-type: inline-size;
  transition:
    box-shadow 0.26s ease,
    border-color 0.18s ease,
    transform 0.26s var(--ease);
}

.pcard:hover {
  border-color: var(--card-line-hover);
  box-shadow: var(--card-shadow-hover);
  transform: translateY(-1px);
}

.pcard--overview {
  /* 高度由总览栅格行高给定：全部卡片一屏显示，不再固定 424 */
  height: 100%;
}

/* 总览不做整组缩放：文字按卡片实际宽度放大，闪电与水印保持原比例 */
.pcard--overview .pcard__brand {
  font-size: clamp(22px, 11cqw, 56px);
}

.pcard--overview .pcard__model {
  font-size: clamp(12px, 5.2cqw, 22px);
}

.pcard--overview .batt__pct {
  font-size: clamp(12px, 5.2cqw, 20px);
}

.pcard--overview .batt__tag {
  font-size: clamp(10px, 4cqw, 14px);
}

.pcard--overview .mi span {
  font-size: clamp(11px, 4.4cqw, 16px);
}

.pcard--overview .mi b {
  font-size: clamp(13px, 6cqw, 20px);
}

.pcard--overview .chip {
  font-size: clamp(11px, 4.4cqw, 16px);
}

/* 总览设备很多时卡片很窄：状态胶囊放不下就隐藏，避免被卡片边缘裁切 */
@container (max-width: 175px) {
  .pcard--overview .chip {
    display: none;
  }

  /* 卡片太小放不下三行字段：保留连接方式与设备标识，收紧间距 */
  .pcard--overview .mi--power {
    display: none;
  }

  .pcard--overview .pcard__meta {
    gap: 1.2cqw;
  }
}

.pcard--devices {
  /* 高度由设备页栅格行高给定：2 列一行 / 3 列两行铺满整页 */
  height: 100%;
}

.pcard.is-active {
  border-color: var(--accent);
  /* 外圈高亮会被 mask 裁掉，改用内圈 inset 环 */
  box-shadow: inset 0 0 0 2px color-mix(in srgb, var(--accent) 45%, transparent),
    var(--card-shadow-hover);
}

.pcard__grip {
  position: absolute;
  top: 9px;
  right: 9px;
  z-index: 6;
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  padding: 0;
  border: 0;
  border-radius: 7px;
  background: rgb(127 127 127 / 14%);
  color: var(--ink-3);
  cursor: grab;
  touch-action: none;
}

.pcard__grip:hover {
  background: rgb(127 127 127 / 26%);
  color: var(--ink);
}

.pcard__grip:active {
  cursor: grabbing;
}

.pcard__cfg {
  position: absolute;
  top: 9px;
  right: 9px;
  z-index: 6;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  padding: 0;
  border: 1px solid var(--card-line);
  border-radius: 9px;
  background: var(--card);
  color: var(--ink-2);
  cursor: pointer;
  transition:
    background 0.18s ease,
    color 0.18s ease,
    transform 0.18s ease;
}

/* 删除按钮：与右侧设置按钮同尺寸，放在它左边 */
.pcard__del {
  position: absolute;
  top: 9px;
  right: 45px;
  z-index: 6;
  display: grid;
  place-items: center;
  width: 30px;
  height: 30px;
  padding: 0;
  border: 1px solid var(--card-line);
  border-radius: 9px;
  background: var(--card);
  color: var(--ink-2);
  cursor: pointer;
  transition:
    background 0.18s ease,
    color 0.18s ease,
    transform 0.18s ease;
}

/* 编辑态两个按钮会压住标题行，给右上留出按钮宽度 */
.pcard.is-editing .pcard__top {
  padding-right: 72px;
}

@media (hover: hover) {
  .pcard__del:hover {
    border-color: color-mix(in srgb, var(--red-text) 45%, transparent);
    background: var(--danger-bg);
    color: var(--red-text);
  }

  .pcard__del:hover :deep(svg) {
    transform: scale(1.06);
  }
}

.pcard__del.is-pending {
  border-color: transparent;
  background: var(--red-text);
  color: #fff;
}

.pcard__cfg:hover {
  background: var(--accent);
  color: var(--accent-fg);
  transform: scale(1.06);
}

/* 竖排背景字 */
.pcard__wm {
  position: absolute;
  top: 50%;
  right: 3.2%;
  z-index: 0;
  overflow: hidden;
  color: var(--pcard-wm);
  font-weight: 800;
  line-height: 1;
  letter-spacing: -0.02em;
  white-space: nowrap;
  pointer-events: none;
  user-select: none;
  writing-mode: vertical-rl;
  transform: translateY(-50%);
  transition:
    transform 0.35s var(--ease),
    color 0.3s ease;
}

.pcard:hover .pcard__wm {
  transform: translate(-2px, -50%);
}

/* 顶部：品牌 / 名称 / 电量 */
.pcard__top {
  position: relative;
  z-index: 3;
}

.pcard__brand {
  font-size: 8.6cqw;
  font-size: clamp(20px, 8.6cqw, 44px);
  font-weight: 800;
  line-height: 1;
  letter-spacing: -0.03em;
  color: var(--ink);
}

.pcard__model {
  margin-top: 0.9cqw;
  color: var(--ink-2);
  font-size: 3.7cqw;
  font-size: clamp(11px, 3.7cqw, 20px);
  line-height: 1.25;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pcard__batt {
  display: inline-flex;
  align-items: center;
  gap: 1.6cqw;
  margin-top: 2.6cqw;
  color: var(--ink);
}

.pcard__batt svg {
  width: 6.8cqw;
  height: 3.6cqw;
  overflow: visible;
}

.batt__fill {
  fill: var(--batt);
}

.batt__pct {
  font-family: var(--font-num);
  font-size: 3.4cqw;
  font-size: clamp(11px, 3.4cqw, 16px);
  font-weight: 700;
  font-variant-numeric: tabular-nums;
}

.batt__tag {
  display: inline-flex;
  align-items: center;
  padding: 0.6cqw 2cqw;
  border-radius: var(--r-pill);
  background: var(--danger-bg);
  color: var(--red-text);
  font-size: clamp(9px, 2.6cqw, 12px);
  font-weight: 600;
}

.pcard__batt.low .batt__pct {
  color: var(--red-text);
}

/* 舞台只作为栅格中间的占位行，图片层绝对覆盖整张卡片并居中 */
.pcard__stage {
  position: relative;
  z-index: 1;
  min-height: 0;
  margin: 2.4% 0 2%;
}

.pcard__media {
  position: absolute;
  inset: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}

/* 统一图片框：宽高比取设备页 3 列实测的 207:114。
 * 2 列卡片更高时框只按宽度放大并居中，图片与设备页 3 列等比，
 * 不会因为卡片变高就把图片单独拉大；缩小/偏移/旋转仍按框百分比。 */
.pcard__frame {
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  max-height: 100%;
  aspect-ratio: 207 / 114;
}

.pcard__glow {
  position: absolute;
  top: 52%;
  left: 50%;
  width: 78%;
  height: 78%;
  background: radial-gradient(closest-side, var(--pcard-glow), transparent 72%);
  filter: blur(6px);
  opacity: 0.95;
  transform: translate(-50%, -50%);
}

.pcard__glow.off {
  display: none;
}

.pcard__img {
  position: relative;
  flex: none;
  /* 尺寸由 cardImageBox 按“卡片长边占比”算好；这里只负责 contain 与滤镜/变换。 */
  width: var(--img-w, 100%);
  height: var(--img-h, 100%);
  object-fit: contain;
  filter: var(--card-img-shadow);
  transform: translate(var(--dx, 0%), var(--dy, 0%)) rotate(var(--rot, 0deg));
  transition:
    transform 0.3s var(--ease),
    filter 0.3s ease;
  -webkit-user-drag: none;
  user-select: none;
}

.pcard__img.no-shadow {
  filter: none;
}

/* 没有实物图时的电量光环：边长按卡片约 46% 定，再由画框高度兜底。
 * 波纹最大扩散到 1.6×，正好落在卡片内，不压标题与底部字段。 */
.pcard__orb {
  flex: none;
  --orb-size: min(46cqw, 100%);
}

/* 底部：三行字段 + 状态芯片 */
.pcard__bot {
  position: relative;
  z-index: 3;
  display: flex;
  align-items: flex-end;
  gap: 2cqw;
}

.pcard__meta {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 2.2cqw;
  min-width: 0;
}

.mi {
  display: flex;
  flex-direction: column;
  gap: 0.7cqw;
  min-width: 0;
}

.mi span {
  color: var(--ink-3);
  font-size: 2.9cqw;
  font-size: clamp(10px, 2.9cqw, 13px);
  font-weight: 600;
  line-height: 1.1;
}

.mi b {
  display: inline-flex;
  align-items: center;
  gap: 0.4em;
  overflow: hidden;
  color: var(--ink);
  font-size: 3.6cqw;
  font-size: clamp(12px, 3.6cqw, 16px);
  font-weight: 700;
  line-height: 1.15;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* 字段值前的小图标（连接方式的信号标） */
.mi__icon {
  flex: none;
  width: 1em;
  height: 1em;
  transform: translateY(-0.04em);
}

.mi b.mono {
  font-family: "Cascadia Code", Consolas, "Segoe UI", monospace;
  letter-spacing: 0;
}

.chip {
  display: inline-flex;
  align-items: center;
  gap: 1cqw;
  margin-left: auto;
  padding: 1cqw 2.4cqw;
  border-radius: var(--r-pill);
  font-size: clamp(10px, 2.8cqw, 14px);
  font-weight: 600;
  white-space: nowrap;
}

.chip.live {
  background: var(--live-bg);
  color: var(--green-text);
}

.chip.warn {
  background: var(--danger-bg);
  color: var(--red-text);
}

.pcard.hide-link .mi--link,
.pcard.hide-ident .mi--ident,
.pcard.hide-power .mi--power {
  display: none;
}
</style>