<script setup lang="ts">
import { computed } from 'vue';

const {
  level,
  size = 72,
  thickness = 8,
  color = 'var(--good)',
  track = 'var(--track)',
  start = 150,
  sweep = 240,
  showText = false,
} = defineProps<{
  level: number | null;
  size?: number;
  thickness?: number;
  color?: string;
  track?: string;
  start?: number;
  sweep?: number;
  showText?: boolean;
}>();

function polar(cx: number, cy: number, r: number, deg: number) {
  const rad = (deg * Math.PI) / 180;
  return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
}

function round(value: number) {
  return Math.round(value * 1000) / 1000;
}

function arcPath(cx: number, cy: number, r: number, from: number, span: number) {
  if (Math.abs(span) < 0.1) return '';
  const a = polar(cx, cy, r, from);
  const b = polar(cx, cy, r, from + span);
  const largeArc = Math.abs(span) > 180 ? 1 : 0;
  const sweepFlag = span >= 0 ? 1 : 0;
  return `M${round(a.x)} ${round(a.y)} A${round(r)} ${round(r)} 0 ${largeArc} ${sweepFlag} ${round(b.x)} ${round(b.y)}`;
}

const center = computed(() => size / 2);
const radius = computed(() => Math.max(0.1, (size - thickness) / 2));
const strokeWidth = computed(() => Math.max(0.1, Math.min(thickness, size)));

const trackPath = computed(() =>
  arcPath(center.value, center.value, radius.value, start, sweep),
);

const pct = computed(() => {
  if (level == null || !Number.isFinite(level)) return null;
  return Math.min(100, Math.max(0, level));
});

const progressPath = computed(() => {
  if (pct.value == null || pct.value <= 0) return '';
  return arcPath(
    center.value,
    center.value,
    radius.value,
    start,
    (sweep * pct.value) / 100,
  );
});

const textSize = computed(() => size * 0.28);
</script>

<template>
  <svg
    class="battery-ring"
    :width="size"
    :height="size"
    :viewBox="`0 0 ${size} ${size}`"
    fill="none"
    aria-hidden="true"
    focusable="false"
  >
    <path
      v-if="trackPath"
      :d="trackPath"
      :stroke="track"
      :stroke-width="strokeWidth"
      stroke-linecap="round"
    />
    <path
      v-if="progressPath"
      :d="progressPath"
      :stroke="color"
      :stroke-width="strokeWidth"
      stroke-linecap="round"
    />
    <text
      v-if="showText && pct != null"
      :x="center"
      :y="center"
      :font-size="textSize"
      fill="var(--txt)"
      font-family="var(--font-num)"
      font-weight="700"
      text-anchor="middle"
      dominant-baseline="central"
    >
      {{ Math.round(pct) }}
    </text>
  </svg>
</template>

<style scoped>
.battery-ring {
  display: block;
  flex: none;
  overflow: visible;
}
</style>
