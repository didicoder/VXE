<script setup lang="ts">
/**
 * 预览小窗的设备行：电量圆环（环心闪电）+ 名称 + 状态芯片 + 电量数字。
 *
 * 几何按 V16 `widgets.compact_row`（ROW_H=72、图标槽 44 @ 16、文字列 x=74、
 * 数字右对齐 22 numbold）。环色取电量档位色，环心闪电取设备强调色。
 */
import { computed } from 'vue';
import BatteryRing from '../BatteryRing.vue';
import StateChip from '../StateChip.vue';
import LineIcon from '../icons/LineIcon.vue';
import { bandColor, bandOf, isCharging } from '../../lib/bands';
import { COLOR_HEX, colorHex } from '../../lib/colors';
import { levelOf, levelText } from '../../lib/state';
import type { BandColors, ColorKey } from '../../types/battery';
import type { DeviceStatus } from '../../types/device';

/**
 * 档位色 → CSS 颜色。good/mid/low 与强调色板的绿/蓝/红是同一组值，
 * 走主题 token 才能拿到 light 变体；紫/橙/青没有 token，用色板原值。
 */
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

const emit = defineEmits<{
  select: [];
}>();

const display = computed(() => levelOf(props.device));
const level = computed(() => display.value.level);
const live = computed(() => display.value.live);
const charging = computed(() => isCharging(props.device));
const name = computed(() => props.device.label || props.device.id);

/** 即使没读数也用 FAINT 画环，别把环画成彩色。 */
const baseColor = computed(() => {
  if (level.value == null) return 'var(--faint)';
  const key = bandColor(
    bandOf(level.value, props.good, props.warn),
    props.bandColors,
    props.device.colors,
  );
  return BAND_VAR[key];
});

/** 上次读数：环压到底色上（V16 `mix(col, CARD, 0.45)`），数字用 muted。 */
const ringColor = computed(() =>
  live.value
    ? baseColor.value
    : `color-mix(in srgb, ${baseColor.value} 55%, var(--card))`,
);
const pctColor = computed(() =>
  live.value
    ? baseColor.value
    : `color-mix(in srgb, ${baseColor.value} 65%, var(--card))`,
);
/** 环心闪电跟随设备强调色；没有读数时转灰。 */
const boltColor = computed(() =>
  live.value ? colorHex(props.device.color) : 'var(--gray)',
);
</script>

<template>
  <div
    class="popup-row"
    role="button"
    tabindex="0"
    @click="emit('select')"
    @keydown.enter.prevent="emit('select')"
    @keydown.space.prevent="emit('select')"
  >
    <span class="popup-row__icon">
      <BatteryRing
        :level="level"
        :size="30"
        :thickness="3"
        :color="ringColor"
      />
      <svg
        class="popup-row__glyph"
        viewBox="0 0 24 24"
        width="13.5"
        height="13.5"
        aria-hidden="true"
      >
        <path
          d="M13.5 2.5 L6 13.5 H11 L10 21.5 L18 10.5 H12.5 L13.5 2.5 Z"
          :fill="boltColor"
          stroke="none"
        />
      </svg>
    </span>

    <span class="popup-row__text">
      <span class="popup-row__name">{{ name }}</span>
      <StateChip
        class="popup-row__state"
        :state="device.state"
        :charging="device.charging"
      />
    </span>

    <span class="popup-row__value">
      <LineIcon
        v-if="charging"
        name="bolt"
        :size="8.25"
        :stroke="1.2"
        color="var(--good)"
      />
      <span class="popup-row__num" :class="{ 'is-dim': !live }">
        {{ levelText(level) }}
      </span>
      <span class="popup-row__pct" :style="{ color: pctColor }">%</span>
    </span>
  </div>
</template>

<style scoped>
.popup-row {
  position: relative;
  display: block;
  flex: none;
  width: 100%;
  height: 54px;
  padding: 0;
  border: 1px solid var(--card-line);
  border-radius: var(--r-tile);
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  text-align: left;
  cursor: pointer;
  will-change: transform;
  backface-visibility: hidden;
  transition:
    transform 0.26s var(--ease),
    box-shadow 0.26s ease,
    border-color 0.18s ease;
}

.popup-row:focus-visible {
  outline: 1.5px solid var(--accent);
  outline-offset: -1.125px;
}

@media (hover: hover) {
  .popup-row:hover {
    transform: translateY(var(--lift));
    border-color: var(--card-line-hover);
    box-shadow: var(--card-shadow-hover);
  }
}

.popup-row__icon {
  position: absolute;
  top: 10.5px;
  left: 12px;
  display: grid;
  place-items: center;
  width: 33px;
  height: 33px;
}

.popup-row__glyph {
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
}

.popup-row__text {
  position: absolute;
  top: 13.5px;
  left: 55.5px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  width: 177px;
}

.popup-row__name {
  display: block;
  overflow: hidden;
  padding: 2px 0;
  margin: -2px 0;
  color: var(--ink);
  font-size: 11.25px;
  font-weight: 700;
  line-height: 11.25px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.popup-row__state {
  display: block;
  overflow: hidden;
  padding: 2px 0;
  margin: -2px 0;
  color: var(--ink-3);
  font-size: 9.75px;
  font-weight: 400;
  line-height: 9.75px;
  white-space: nowrap;
}

.popup-row__value {
  position: absolute;
  top: 12.75px;
  right: 12px;
  display: flex;
  align-items: center;
  gap: 3px;
}

.popup-row__num {
  font-family: var(--font-num);
  font-size: 15.75px;
  font-weight: 700;
  line-height: 15.75px;
}

.popup-row__num.is-dim {
  color: var(--muted);
}

.popup-row__pct {
  color: var(--muted);
  font-family: var(--font-body);
  font-size: 9px;
  font-weight: 700;
  line-height: 9px;
}
</style>
