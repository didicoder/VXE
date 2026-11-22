<script setup lang="ts">
/**
 * useanimations `menu-v4`（MIT）的 Vue 适配：0.7s 汉堡 ↔ 关闭微动画。
 *
 * 原组件是 React 封装（react-useanimations），这里用 lottie-web 直接驱动
 * 同一份 Lottie JSON：`active=false` 定格汉堡，切 true 正放、切 false 倒放。
 * 描边走 currentColor，随主题取色。
 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { AnimationItem } from 'lottie-web';
import lottie from 'lottie-web/build/player/lottie_light';
import animationData from '../../assets/menu4.json';

const {
  size = 20,
  active = false,
} = defineProps<{
  size?: number;
  active?: boolean;
}>();

const host = ref<HTMLDivElement | null>(null);
let anim: AnimationItem | null = null;

onMounted(() => {
  if (!host.value) return;
  anim = lottie.loadAnimation({
    container: host.value,
    renderer: 'svg',
    loop: false,
    autoplay: false,
    animationData: animationData as never,
  });
  anim.goToAndStop(active ? anim.totalFrames - 1 : 0, true);
});

watch(
  () => active,
  (next) => {
    if (!anim) return;
    anim.setDirection(next ? 1 : -1);
    anim.play();
  },
);

onBeforeUnmount(() => {
  anim?.destroy();
  anim = null;
});
</script>

<template>
  <div
    class="menu-icon"
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
  >
    <div ref="host" class="menu-icon__lottie"></div>
  </div>
</template>

<style scoped>
.menu-icon {
  display: grid;
  place-items: center;
  flex: none;
  color: inherit;
}

.menu-icon__lottie {
  width: 100%;
  height: 100%;
}

/* Lottie 描边写死黑色（源文件），统一交给 currentColor 跟主题 */
.menu-icon :deep(svg path),
.menu-icon :deep(svg line),
.menu-icon :deep(svg polyline) {
  stroke: currentColor;
}
</style>
