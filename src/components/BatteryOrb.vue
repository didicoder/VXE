<script setup lang="ts">
/**
 * 电量状态光环：扩散波纹 + 闪电核心。
 *
 * 用作没有自选图片时的卡片默认图形；颜色跟随卡片当前档位色，
 * 低电量时核心呼吸闪烁。尺寸由使用方给定（卡片里按卡片宽度换算），
 * 组件本身只按 100% 填满容器。
 */
import type { BatteryBand } from '../types/battery'

withDefaults(
  defineProps<{
    /** good / mid / low 对应三档电量；none = 没有读数。 */
    tone: BatteryBand | 'none'
    /** 任意 CSS 颜色（卡片传入的 --good/--mid/--low 或自定义色）。 */
    color: string
  }>(),
  { tone: 'none', color: 'var(--faint)' },
)
</script>

<template>
  <div
    class="orb"
    :class="`orb--${tone}`"
    :style="{ '--orb-color': color }"
    aria-hidden="true"
  >
    <span class="orb__ring" />
    <span class="orb__ring" />
    <span class="orb__ring" />
    <span class="orb__core">
      <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
        <path d="M13.5 2.5 L6 13.5 H11 L10 21.5 L18 10.5 H12.5 L13.5 2.5 Z" />
      </svg>
    </span>
  </div>
</template>

<style scoped>
.orb {
  position: relative;
  display: grid;
  place-items: center;
  /* 使用方通过 --orb-size 定边长；缺省填满容器 */
  width: var(--orb-size, 100%);
  height: var(--orb-size, 100%);
  aspect-ratio: 1;
  pointer-events: none;
}

/* 波纹：参考稿的方角环，以档位色为主、与卡片发丝线预混 */
.orb__ring {
  position: absolute;
  inset: 0;
  box-sizing: border-box;
  border: 1.5px solid color-mix(in srgb, var(--orb-color) 52%, var(--card-line));
  border-radius: 35%;
  animation: orb-ripple 4s infinite cubic-bezier(0.2, 0.6, 0.4, 1);
}

.orb__ring:nth-child(1) {
  animation-delay: 0s;
}

.orb__ring:nth-child(2) {
  animation-delay: 1.3s;
}

.orb__ring:nth-child(3) {
  animation-delay: 2.6s;
}

@keyframes orb-ripple {
  0% {
    opacity: 0.85;
    transform: scale(0.8);
  }
  100% {
    opacity: 0;
    transform: scale(1.6);
  }
}

/* 闪电核心 */
.orb__core {
  position: relative;
  z-index: 1;
  display: grid;
  place-items: center;
  width: 60%;
  aspect-ratio: 1;
}

.orb__core svg {
  width: 100%;
  height: 100%;
  filter: drop-shadow(0 0 6px color-mix(in srgb, var(--orb-color) 30%, transparent));
}

.orb__core path {
  fill: var(--orb-color);
  stroke: var(--orb-color);
  stroke-width: 2.5;
  stroke-linejoin: round;
  stroke-linecap: round;
}

/* 低电量：波纹带色 + 核心呼吸光晕与闪烁 */
.orb--low .orb__ring {
  border-color: color-mix(in srgb, var(--orb-color) 68%, var(--card-line));
}

.orb--low .orb__core svg {
  animation:
    orb-glow 2.4s ease-in-out infinite,
    orb-blink 2.4s ease-in-out infinite;
}

@keyframes orb-glow {
  0%,
  100% {
    filter: drop-shadow(0 0 5px color-mix(in srgb, var(--orb-color) 35%, transparent));
  }
  50% {
    filter: drop-shadow(0 0 13px color-mix(in srgb, var(--orb-color) 80%, transparent));
  }
}

@keyframes orb-blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.3;
  }
}

/* 没有读数：三圈保留为固定的灰色构图，不启动动画。 */
.orb--none .orb__ring {
  border-color: color-mix(in srgb, var(--orb-color) 45%, var(--card-line));
  animation: none;
  opacity: 0.28;
}

.orb--none .orb__ring:nth-child(1) {
  transform: scale(0.82);
}

.orb--none .orb__ring:nth-child(2) {
  transform: scale(1.12);
  opacity: 0.2;
}

.orb--none .orb__ring:nth-child(3) {
  transform: scale(1.42);
  opacity: 0.12;
}

.orb--none .orb__core svg {
  opacity: 0.72;
  filter: none;
}

@media (prefers-reduced-motion: reduce) {
  .orb__ring,
  .orb--low .orb__core svg {
    animation: none;
  }

  .orb__ring {
    opacity: 0.3;
    transform: scale(0.9);
  }

  .orb__ring:nth-child(2),
  .orb__ring:nth-child(3) {
    display: none;
  }
}
</style>