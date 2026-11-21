<script setup lang="ts">
import { computed } from 'vue';

export type LineIconName =
  | 'gauge'
  | 'mouse'
  | 'sliders'
  | 'plus'
  | 'minus'
  | 'close'
  | 'refresh'
  | 'battery'
  | 'bolt'
  | 'trash'
  | 'pencil'
  | 'check'
  | 'search'
  | 'power'
  | 'sun'
  | 'moon'
  | 'chevron'
  | 'move';

const {
  name,
  size = 24,
  stroke = 1.6,
  color = 'currentColor',
} = defineProps<{
  name: LineIconName;
  size?: number;
  stroke?: number;
  color?: string;
}>();

// V16 的线宽是设计 px，不随图标 size 缩放；viewBox 是 24 单位，
// 所以把「设计 px」换算回视图单位，渲染后的粗细才是 stroke。
const svgStroke = computed(() => (stroke * 24) / Math.max(1, size));
const halfStroke = computed(() => svgStroke.value / 2);
const sliderKnobR = computed(() => Math.max(0.2, 2.6 - halfStroke.value));
const searchR = computed(() => Math.max(0.2, 7.2 - halfStroke.value));
const sunR = computed(() => Math.max(0.2, 6.72 - halfStroke.value));

const sunRays: ReadonlyArray<readonly [number, number, number, number]> = [
  [21.744, 12, 24.768, 12],
  [18.89, 18.89, 21.028, 21.028],
  [12, 21.744, 12, 24.768],
  [5.11, 18.89, 2.972, 21.028],
  [2.256, 12, -0.768, 12],
  [5.11, 5.11, 2.972, 2.972],
  [12, 2.256, 12, -0.768],
  [18.89, 5.11, 21.028, 2.972],
];
</script>

<template>
  <svg
    class="line-icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
    focusable="false"
  >
    <g
      :stroke="color"
      :stroke-width="svgStroke"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <g v-if="name === 'gauge'">
        <path d="M4.518 17.76 A8.64 8.64 0 1 1 19.482 17.76" />
        <path d="M12 13.44 L8.703 9.219" />
        <circle
          cx="12"
          cy="13.44"
          :r="svgStroke * 0.85"
          :fill="color"
          stroke="none"
        />
      </g>

      <g v-else-if="name === 'mouse'">
        <rect
          :x="6.5 + halfStroke"
          :y="3.5 + halfStroke"
          :width="11 - svgStroke"
          :height="17 - svgStroke"
          :rx="Math.max(0.2, 5 - halfStroke)"
        />
        <line x1="12" y1="3.5" x2="12" y2="9.4" />
        <line
          x1="6.5"
          y1="9.4"
          x2="17.5"
          y2="9.4"
          :stroke-width="svgStroke * 0.85"
        />
      </g>

      <g v-else-if="name === 'sliders'">
        <line x1="4" y1="6" x2="5.32" y2="6" />
        <line x1="12.92" y1="6" x2="20" y2="6" />
        <circle cx="9.12" cy="6" :r="sliderKnobR" />
        <line x1="4" y1="12" x2="11.08" y2="12" />
        <line x1="18.68" y1="12" x2="20" y2="12" />
        <circle cx="14.88" cy="12" :r="sliderKnobR" />
        <line x1="4" y1="18" x2="7.24" y2="18" />
        <line x1="14.84" y1="18" x2="20" y2="18" />
        <circle cx="11.04" cy="18" :r="sliderKnobR" />
      </g>

      <g v-else-if="name === 'plus'">
        <line x1="0" y1="12" x2="24" y2="12" />
        <line x1="12" y1="0" x2="12" y2="24" />
      </g>

      <g v-else-if="name === 'minus'">
        <line x1="0" y1="12" x2="24" y2="12" />
      </g>

      <g v-else-if="name === 'close'">
        <line x1="0" y1="0" x2="24" y2="24" />
        <line x1="24" y1="0" x2="0" y2="24" />
      </g>

      <g v-else-if="name === 'refresh'">
        <path d="M16.08 4.933 A8.16 8.16 0 1 1 4.933 7.92" />
        <path
          d="M8.192 11.178 L1.675 10.092 L7.105 4.662 Z"
          :fill="color"
          stroke="none"
        />
      </g>

      <g v-else-if="name === 'battery'">
        <rect
          :x="4 + halfStroke"
          :y="7 + halfStroke"
          :width="14 - svgStroke"
          :height="10 - svgStroke"
          :rx="Math.max(0.2, 2.4 - halfStroke)"
        />
        <rect
          x="18.6"
          y="10"
          width="1.8"
          height="4"
          rx="1.2"
          :fill="color"
          stroke="none"
        />
        <rect
          x="5.6"
          y="8.6"
          width="6.52"
          height="6.8"
          rx="1.2"
          :fill="color"
          stroke="none"
        />
      </g>

      <g v-else-if="name === 'bolt'">
        <path
          d="M13.4 2.6 L5.6 13.6 L11.2 13.6 L9.6 21.4 L18.2 10.2 L12.2 10.2 Z"
          :fill="color"
          stroke="none"
        />
      </g>

      <g v-else-if="name === 'trash'">
        <line x1="5" y1="7" x2="19" y2="7" />
        <line x1="9" y1="4" x2="15" y2="4" />
        <path d="M6.4 9 L7.4 20 L16.6 20 L17.6 9" />
        <line
          x1="10.2"
          y1="12"
          x2="10.2"
          y2="17"
          :stroke-width="svgStroke * 0.9"
        />
        <line
          x1="13.8"
          y1="12"
          x2="13.8"
          y2="17"
          :stroke-width="svgStroke * 0.9"
        />
      </g>

      <g v-else-if="name === 'pencil'">
        <path d="M6 18 L7 14.4 L16.2 5.4 L18.4 7.6 L9.2 16.6 Z" />
      </g>

      <g v-else-if="name === 'check'">
        <path d="M6.5 12.4 L10.4 16.4 L17.8 7.6" />
      </g>

      <g v-else-if="name === 'search'">
        <circle cx="10.56" cy="10.56" :r="searchR" />
        <line x1="15.744" y1="15.744" x2="20.64" y2="20.64" />
      </g>

      <g v-else-if="name === 'power'">
        <path d="M16.955 6.362 A8.64 8.64 0 1 1 7.045 6.362" />
        <line x1="12" y1="4.32" x2="12" y2="14.4" />
      </g>

      <g v-else-if="name === 'sun'">
        <circle cx="12" cy="12" :r="sunR" />
        <line
          v-for="(ray, index) in sunRays"
          :key="index"
          :x1="ray[0]"
          :y1="ray[1]"
          :x2="ray[2]"
          :y2="ray[3]"
        />
      </g>

      <g v-else-if="name === 'moon'">
        <path
          d="M20.7 14.2 A8.7 8.7 0 1 1 9.8 3.3 A7 7 0 0 0 20.7 14.2 Z"
          :fill="color"
          stroke="none"
        />
      </g>

      <g v-else-if="name === 'chevron'">
        <path d="M9.6 7 L15 12 L9.6 17" />
      </g>
      <g v-else-if="name === 'move'" transform="scale(1.5)">
        <circle cx="8" cy="8" r="0.55" fill="currentColor" stroke="none" />
        <path d="M6 4.25 L8 2.25 L10 4.25" />
        <path d="M11.75 6 L13.75 8 L11.75 10" />
        <path d="M10 11.75 L8 13.75 L6 11.75" />
        <path d="M4.25 10 L2.25 8 L4.25 6" />
      </g>
    </g>
  </svg>
</template>

<style scoped>
.line-icon {
  display: block;
  flex: none;
  overflow: visible;
}
</style>
