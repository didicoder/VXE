<script setup lang="ts">
/**
 * 刷新图标：双弧双箭头，动画复刻 useanimations「refresh-v2」。
 *
 * Lottie（30 fps / 40 帧）：0° → 10% 回拉 -15° → 65% 转到 370° →
 * 80% 回落 360° 并停住。每次 busy 由 false 变 true 重播一次。
 */
import { computed, onBeforeUnmount, ref, watch } from 'vue';

const {
  size = 18,
  stroke = 2,
  color = 'currentColor',
  busy = false,
} = defineProps<{
  size?: number;
  stroke?: number;
  color?: string;
  busy?: boolean;
}>();

const svgStroke = computed(() => (stroke * 24) / Math.max(1, size));
const playing = ref(false);
let frame: number | null = null;

function cancelFrame(): void {
  if (frame !== null) {
    cancelAnimationFrame(frame);
    frame = null;
  }
}

watch(
  () => busy,
  (value) => {
    if (!value) return;
    cancelFrame();
    playing.value = false;
    frame = requestAnimationFrame(() => {
      frame = null;
      playing.value = true;
    });
  },
  { immediate: true },
);

onBeforeUnmount(cancelFrame);
</script>

<template>
  <svg
    class="refresh-icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
    focusable="false"
    @animationend="playing = false"
  >
    <g
      class="refresh-icon__spin"
      :class="{ 'refresh-icon__spin--on': playing }"
      :stroke="color"
      :stroke-width="svgStroke"
      stroke-linecap="round"
      stroke-linejoin="round"
    >
      <path d="M23 4 L23 10 L17 10" />
      <path d="M1 20 L1 14 L7 14" />
      <path
        d="M3.51 9 C4.541 6.087 6.994 3.906 10.008 3.224 C13.022 2.542 16.175 3.454 18.36 5.64 L23 10"
      />
      <path
        d="M1 14 L5.64 18.36 C7.825 20.546 10.978 21.458 13.992 20.776 C17.006 20.094 19.459 17.913 20.49 15"
      />
    </g>
  </svg>
</template>

<style scoped>
.refresh-icon {
  display: block;
  flex: none;
  overflow: visible;
}

.refresh-icon__spin {
  transform-box: view-box;
  transform-origin: 12px 12px;
}

.refresh-icon__spin--on {
  animation: refresh-v2-spin 1.33s both;
}

@keyframes refresh-v2-spin {
  0% {
    transform: rotate(0deg);
    animation-timing-function: cubic-bezier(0, 0, 0.58, 1);
  }

  10% {
    transform: rotate(-15deg);
    animation-timing-function: cubic-bezier(0.42, 0, 0.58, 1);
  }

  65% {
    transform: rotate(370deg);
    animation-timing-function: cubic-bezier(0.42, 0, 0.58, 1);
  }

  80%,
  100% {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .refresh-icon__spin--on {
    animation: none;
  }
}
</style>
