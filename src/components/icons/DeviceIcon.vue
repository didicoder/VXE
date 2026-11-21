<script setup lang="ts">
import { useId } from 'vue';

export type DeviceIconName =
  | 'mouse'
  | 'keyboard'
  | 'headset'
  | 'earbuds'
  | 'gamepad'
  | 'phone';

const {
  name,
  size = 24,
  color = 'currentColor',
} = defineProps<{
  name: DeviceIconName;
  size?: number;
  color?: string;
}>();

const maskId = `device-icon-${useId()}`;
const maskUrl = `url(#${maskId})`;

const keyboardTopKeys = [1.6, 7.045, 12.49, 17.935];

// V16 paint.py 的 Xbox 轮廓（左右各一半，k=size/36）；这里按 24 网格等比缩到 0.6。
const PAD_HALF: ReadonlyArray<readonly [number, number]> = [
  [0, -13.9],
  [5.5, -13.9],
  [9.3, -12.9],
  [12.8, -10.3],
  [15.2, -7.9],
  [16.3, -5.9],
  [18.1, -0.3],
  [19.7, 4.8],
  [20, 7.6],
  [19.5, 10.7],
  [17.9, 12.8],
  [15.3, 14],
  [14.5, 13.6],
  [9, 8.1],
  [6, 6.8],
  [0, 6.8],
];

function smoothClosedPath(
  points: ReadonlyArray<readonly [number, number]>,
  scale: number,
  cx: number,
  cy: number,
): string {
  const num = (value: number) => Math.round(value * 1000) / 1000;
  const n = points.length;
  let d = '';
  for (let i = 0; i < n; i += 1) {
    const p0 = points[(i - 1 + n) % n];
    const p1 = points[i];
    const p2 = points[(i + 1) % n];
    const p3 = points[(i + 2) % n];
    const x1 = cx + p1[0] * scale;
    const y1 = cy + p1[1] * scale;
    const c1x = cx + (p1[0] + (p2[0] - p0[0]) / 6) * scale;
    const c1y = cy + (p1[1] + (p2[1] - p0[1]) / 6) * scale;
    const c2x = cx + (p2[0] - (p3[0] - p1[0]) / 6) * scale;
    const c2y = cy + (p2[1] - (p3[1] - p1[1]) / 6) * scale;
    const x2 = cx + p2[0] * scale;
    const y2 = cy + p2[1] * scale;
    if (i === 0) d += `M${num(x1)} ${num(y1)}`;
    d += ` C${num(c1x)} ${num(c1y)} ${num(c2x)} ${num(c2y)} ${num(x2)} ${num(y2)}`;
  }
  return `${d} Z`;
}

const gamepadLoop: ReadonlyArray<readonly [number, number]> = [
  ...PAD_HALF,
  ...PAD_HALF.slice(1, -1)
    .reverse()
    .map(([x, y]): [number, number] => [-x, y]),
];
const gamepadPath = smoothClosedPath(gamepadLoop, 0.6, 12, 12);
</script>

<template>
  <svg
    class="device-icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    :fill="color"
    aria-hidden="true"
    focusable="false"
  >
    <template v-if="name === 'mouse'">
      <mask
        :id="maskId"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="24"
        height="24"
      >
        <rect x="4.56" y="0" width="14.88" height="24" rx="7.44" fill="white" />
        <g stroke="black" stroke-width="2.6" stroke-linecap="round" fill="none">
          <line x1="12" y1="0" x2="12" y2="9.6" />
          <line x1="4.56" y1="9.6" x2="19.44" y2="9.6" />
        </g>
      </mask>
      <rect
        x="4.56"
        y="0"
        width="14.88"
        height="24"
        rx="7.44"
        :fill="color"
        :mask="maskUrl"
      />
    </template>

    <template v-else-if="name === 'keyboard'">
      <mask
        :id="maskId"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="24"
        height="24"
      >
        <rect x="0" y="5.066" width="24" height="13.868" rx="2.496" fill="white" />
        <g fill="black">
          <rect
            v-for="(x, index) in keyboardTopKeys"
            :key="index"
            :x="x"
            y="7.111"
            width="4.467"
            height="2.667"
            rx="0.622"
          />
          <rect x="1.6" y="12.8" width="3.2" height="2.667" rx="0.622" />
          <rect x="5.778" y="12.8" width="16.622" height="2.667" rx="0.622" />
        </g>
      </mask>
      <rect
        x="0"
        y="5.066"
        width="24"
        height="13.868"
        rx="2.496"
        :fill="color"
        :mask="maskUrl"
      />
    </template>

    <template v-else-if="name === 'headset'">
      <path
        d="M2.51 10.21 A10.1 12.63 0 0 1 21.49 10.21"
        fill="none"
        :stroke="color"
        stroke-width="3.79"
        stroke-linecap="butt"
      />
      <rect x="0.07" y="11.75" width="5.79" height="12.25" rx="2.53" :fill="color" />
      <rect x="18.14" y="11.75" width="5.79" height="12.25" rx="2.53" :fill="color" />
    </template>

    <template v-else-if="name === 'earbuds'">
      <circle cx="6.24" cy="7.2" r="3.84" :fill="color" />
      <circle cx="17.76" cy="7.2" r="3.84" :fill="color" />
      <rect x="4.8" y="7.2" width="2.88" height="12.96" rx="1.44" :fill="color" />
      <rect x="16.32" y="7.2" width="2.88" height="12.96" rx="1.44" :fill="color" />
    </template>

    <template v-else-if="name === 'gamepad'">
      <mask
        :id="maskId"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="24"
        height="24"
      >
        <path :d="gamepadPath" fill="white" />
        <circle cx="6.18" cy="8.34" r="1.56" fill="black" />
        <circle cx="15.12" cy="11.82" r="1.56" fill="black" />
      </mask>
      <path :d="gamepadPath" :fill="color" :mask="maskUrl" />
    </template>

    <template v-else-if="name === 'phone'">
      <mask
        :id="maskId"
        maskUnits="userSpaceOnUse"
        x="0"
        y="0"
        width="24"
        height="24"
      >
        <rect x="5.28" y="0.48" width="13.44" height="23.04" rx="3.2256" fill="white" />
        <rect x="9.8496" y="1.86" width="4.3008" height="0.6936" rx="0.5376" fill="black" />
        <rect x="9.312" y="21.6768" width="5.376" height="0.4608" rx="0.4032" fill="black" />
      </mask>
      <rect
        x="5.28"
        y="0.48"
        width="13.44"
        height="23.04"
        rx="3.2256"
        :fill="color"
        :mask="maskUrl"
      />
    </template>
  </svg>
</template>

<style scoped>
.device-icon {
  display: block;
  flex: none;
  overflow: visible;
}
</style>
