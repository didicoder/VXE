/**
 * useanimations `trash-v2`（MIT）的 Vue 适配：0.17s 掀盖 / 合盖微动画。
 *
 * 悬停播放、移出倒放；SVG 描边统一改成 `currentColor`，跟随按钮文字颜色。
 * 素材：`src/assets/trash2.json`（useanimations trash-v2，MIT）。
 */
<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { AnimationItem } from 'lottie-web';
import lottie from 'lottie-web/build/player/lottie_light';
import animationData from '../../assets/trash2.json';

const { size = 18, active = false } = defineProps<{
  size?: number;
  active?: boolean;
}>();

const host = ref<HTMLElement | null>(null);
let anim: AnimationItem | null = null;

/** 动画原始描边是黑色，改成 currentColor 才能在深/浅两套主题里都看清。 */
function paintCurrentColor(): void {
  const svg = host.value?.querySelector('svg');
  if (!svg) return;
  svg.querySelectorAll('*').forEach((node) => {
    if (node instanceof SVGElement) {
      node.style.stroke = 'currentColor';
      node.style.fill = 'none';
    }
  });
}

onMounted(() => {
  if (!host.value || typeof lottie.loadAnimation !== 'function') return;
  anim = lottie.loadAnimation({
    container: host.value,
    renderer: 'svg',
    loop: false,
    autoplay: false,
    animationData,
  });
  anim.addEventListener('DOMLoaded', paintCurrentColor);
  anim.goToAndStop(0, true);
  paintCurrentColor();
});

watch(
  () => active,
  (on) => {
    if (!anim) return;
    anim.setDirection(on ? 1 : -1);
    anim.play();
  },
);

onBeforeUnmount(() => {
  anim?.destroy();
  anim = null;
});
</script>

<template>
  <span
    ref="host"
    class="trash-lottie"
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
  />
</template>

<style scoped>
.trash-lottie {
  display: block;
  color: currentColor;
  pointer-events: none;
}

.trash-lottie :deep(svg) {
  display: block;
  width: 100%;
  height: 100%;
}
</style>