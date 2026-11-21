<script setup lang="ts">
import { computed } from 'vue';
import BatteryBar from '../BatteryBar.vue';
import BatteryRing from '../BatteryRing.vue';
import DeviceIcon, { type DeviceIconName } from '../icons/DeviceIcon.vue';
import LineIcon from '../icons/LineIcon.vue';
import { bandColor, bandOf, isCharging } from '../../lib/bands';
import { colorHex } from '../../lib/colors';
import { levelOf } from '../../lib/state';
import type { BandColors, ColorKey } from '../../types/battery';
import type { DeviceStatus } from '../../types/device';

const props = defineProps<{
  status: DeviceStatus;
  good: number;
  warn: number;
  bandColors: BandColors;
}>();

/** 图标优先；没选过按设备类型回退（V16 `widgets.device_icon`）。 */
function deviceIconOf(status: DeviceStatus): DeviceIconName {
  if (status.icon) return status.icon;
  const profile = status.profile.toLowerCase();
  if (profile.startsWith('bt-audio') || status.link.includes('音频') || status.link.includes('耳机')) {
    return 'headset';
  }
  return 'mouse';
}

/** 档位色 → token：三档走 `--good/--mid/--low`，其余走六色板。 */
function bandToken(key: ColorKey): string {
  if (key === 'green') return 'var(--good)';
  if (key === 'blue') return 'var(--mid)';
  if (key === 'red') return 'var(--low)';
  return colorHex(key);
}

const display = computed(() => levelOf(props.status));
const band = computed(() => bandOf(display.value.level, props.good, props.warn));
const colorKey = computed<ColorKey>(() =>
  bandColor(band.value, props.bandColors, props.status.colors),
);
const ringColor = computed(() =>
  display.value.level == null ? 'var(--faint)' : bandToken(colorKey.value),
);
const barColor = computed(() =>
  display.value.live
    ? ringColor.value
    : `color-mix(in srgb, ${ringColor.value} 55%, var(--card))`,
);
const chipColor = computed(() => colorHex(props.status.color));
const iconName = computed(() => deviceIconOf(props.status));
const ident = computed(() => {
  const vid = props.status.vid ?? '';
  const pid = props.status.pid ?? '';
  return vid || pid ? `${vid}:${pid}` : ' ';
});
const label = computed(() => props.status.label);
const link = computed(() => props.status.link || '2.4G');
const charging = computed(() => isCharging(props.status));
</script>

<template>
  <article
    class="card glass-card glass-card--lift"
    :style="{ '--chip': chipColor }"
    data-drag-item
  >
    <button
      type="button"
      class="card__grip"
      aria-label="拖动排序"
      title="按住拖动排序"
      @click.stop
    >
      <LineIcon name="move" :size="15" :stroke="1.5" />
    </button>

    <span class="card__ring">
      <BatteryRing :level="display.level" :size="84.05" :thickness="8.05" :color="ringColor" />
      <DeviceIcon class="card__glyph" :name="iconName" :size="39.47" />
    </span>

    <span class="card__chip">{{ link }}</span>

    <span class="card__label">{{ label }}</span>
    <span class="card__ident">
      {{ ident }}<span v-if="charging" class="card__charge"> · 充电中</span>
    </span>

    <BatteryBar
      class="card__bar"
      :level="display.level"
      :width="260.98"
      :height="8.05"
      :radius="4.03"
      :color="barColor"
    />
  </article>
</template>

<style scoped>
/* 401×132 的总览卡：所有尺寸按卡高 132 从 V16 `widgets.overview_card` 推出。
 * 表面（玻璃 + 悬停抬升）走 .glass-card(.--lift)。 */
.card {
  position: relative;
  width: 100%;
  height: 132px;
  overflow: hidden;
  cursor: default;
}

/* 右上角拖拽把手 */
.card__grip {
  position: absolute;
  top: 6px;
  right: 6px;
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

.card__grip:hover {
  background: rgb(127 127 127 / 26%);
  color: var(--ink);
}

.card__grip:active {
  cursor: grabbing;
}

.card__ring {
  position: absolute;
  top: 18.45px;
  left: 20.06px;
  display: block;
  width: 84.05px;
  height: 84.05px;
}

.card__glyph {
  position: absolute;
  top: 22.3px;
  left: 22.3px;
}

/* 链接芯片：水平居中在环轴上，与进度条同一条基线 */
.card__chip {
  position: absolute;
  top: 91.54px;
  left: 62.09px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 21px;
  padding: 0 8.5px;
  border: 1px solid color-mix(in srgb, var(--chip) 26%, transparent);
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--chip) 15%, var(--chip-bg-mix));
  color: color-mix(in srgb, var(--chip) var(--chip-text-weight), var(--chip-text-mix));
  font-family: var(--font-body);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  transform: translateX(-50%);
}

.card__label {
  position: absolute;
  top: 49.49px;
  left: 122.07px;
  max-width: 260.98px;
  overflow: hidden;
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.01em;
  text-overflow: ellipsis;
  white-space: nowrap;
  transform: translateY(-50%);
  color: var(--ink);
}

.card__ident {
  position: absolute;
  top: 76.55px;
  left: 122.07px;
  overflow: hidden;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  transform: translateY(-50%);
}

.card__charge {
  color: var(--ink-good);
}

.card__bar {
  position: absolute;
  top: 98.59px;
  left: 122.07px;
}
</style>
