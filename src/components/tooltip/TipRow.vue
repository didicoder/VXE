<script setup lang="ts">
/**
 * 悬停卡的一行设备：名称 + 状态 + 电量（13 bold）+ 底部 4.5 进度条。
 *
 * 几何按 V16 `tooltip.py`：行高 50、名称中线 y=11、进度条 y=28、
 * 左右内边距 = PAD 20（条宽 246）。
 */
import { computed } from 'vue';
import BatteryBar from '../BatteryBar.vue';
import LineIcon from '../icons/LineIcon.vue';
import { bandColor, bandOf, isCharging } from '../../lib/bands';
import { COLOR_HEX } from '../../lib/colors';
import { percent } from '../../lib/format';
import { levelOf, stateText } from '../../lib/state';
import type { BandColors, ColorKey } from '../../types/battery';
import type { DeviceStatus } from '../../types/device';

/** 档位色 → CSS 颜色（绿/蓝/红走主题 token，紫/橙/青用色板原值）。 */
const BAND_VAR: Record<ColorKey, string> = {
  green: 'var(--good)',
  blue: 'var(--mid)',
  red: 'var(--low)',
  purple: COLOR_HEX.purple,
  amber: COLOR_HEX.amber,
  cyan: COLOR_HEX.cyan,
};

const props = defineProps<{
  device: DeviceStatus;
  good: number;
  warn: number;
  bandColors: BandColors;
}>();

const display = computed(() => levelOf(props.device));
const label = computed(() => props.device.label || props.device.id);
const state = computed(() => stateText(props.device.state, props.device.charging));
const levelText = computed(() => percent(display.value.level));
const charging = computed(() => isCharging(props.device));

/** 休眠/掉线时退回上次读数，进度条压到底色上（V16 `mix(col, CARD, 0.5)`）。 */
const barColor = computed(() => {
  const level = display.value.level;
  const base =
    level == null
      ? 'var(--faint)'
      : BAND_VAR[
          bandColor(
            bandOf(level, props.good, props.warn),
            props.bandColors,
            props.device.colors,
          )
        ];
  return display.value.live
    ? base
    : `color-mix(in srgb, ${base} 50%, var(--card-soft))`;
});
</script>

<template>
  <div class="tip-row">
    <div class="tip-row__head">
      <span class="tip-row__name">{{ label }}</span>
      <span class="tip-row__state">{{ state }}</span>
      <span class="tip-row__level" :class="{ 'is-dim': !display.live }">
        <LineIcon
          v-if="charging"
          name="bolt"
          :size="9.35"
          :stroke="1.36"
          color="var(--good)"
        />
        {{ levelText }}
      </span>
    </div>
    <BatteryBar
      class="tip-row__bar"
      :level="display.level"
      :width="209.1"
      :height="3.825"
      :radius="1.912"
      :color="barColor"
    />
  </div>
</template>

<style scoped>
.tip-row {
  position: relative;
  flex: none;
  height: 42.5px;
  padding-top: 4.25px;
  transition: transform 0.26s var(--ease);
  will-change: transform;
  backface-visibility: hidden;
}

.tip-row:hover {
  transform: translateY(var(--lift));
}

.tip-row__head {
  display: flex;
  align-items: center;
  gap: 5.1px;
  height: 10.2px;
}

.tip-row__name {
  /* 13px 行盒装不下粗体字形的下沿（约 14px），会被 overflow 裁掉：
     上下各垫 2px 再用负 margin 抵消，裁剪盒放大到 17px，视觉位置不变 */
  overflow: hidden;
  max-width: 127.5px;
  padding: 1.7px 0;
  margin: -1.7px 0;
  color: var(--ink);
  font-size: 11.05px;
  font-weight: 700;
  line-height: 11.05px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.tip-row__state {
  flex: none;
  color: var(--ink-3);
  font-size: 11.05px;
  font-weight: 400;
  line-height: 11.05px;
  white-space: nowrap;
}

.tip-row__level {
  display: inline-flex;
  flex: none;
  align-items: center;
  gap: 2.55px;
  margin-left: auto;
  color: var(--ink);
  font-family: var(--font-num);
  font-size: 11.05px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  line-height: 11.05px;
  white-space: nowrap;
}

.tip-row__level.is-dim {
  color: var(--ink-3);
}

.tip-row__bar {
  margin-top: 7.65px;
}
</style>
