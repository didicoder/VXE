<script setup lang="ts">
/**
 * 背景编辑器窗口（400×630 设计尺寸，和卡片设置窗同规格）。
 * 显示/隐藏由 Rust 控制；窗口保持打开时点「预览」只广播草稿，保存才落盘。
 */
import { onMounted, onUnmounted } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import BackgroundEditor from '../components/background/BackgroundEditor.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowMotion } from '../composables/useWindowMotion';
import { useWindowScale } from '../composables/useWindowScale';

defineProps<{ windowLabel?: string }>();

const themeClass = useThemeClass();
useWallpaper();
const { entering, leaving, enter } = useWindowMotion();
const scale = useWindowScale('bg');

let unlisten: UnlistenFn | null = null;

onMounted(async () => {
  try {
    unlisten = await listen('bg-editor-open', () => {
      void enter();
    });
  } catch {
    // 非 Tauri 环境没有事件总线
  }
});

onUnmounted(() => {
  unlisten?.();
  unlisten = null;
});
</script>

<template>
  <section
    class="bg-window glass-window"
    :class="[themeClass, { 'window-enter': entering, 'window-leave': leaving }]"
    :style="{ '--s': scale }"
    data-window="bg"
  >
    <BackgroundEditor />
  </section>
</template>

<style scoped>
.bg-window {
  position: absolute;
  top: 0;
  left: 0;
  width: 400px;
  height: 630px;
  overflow: hidden;
  border-radius: 18px;
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
  color: var(--txt);
  transform: scale(var(--s));
  transform-origin: top left;
}
</style>