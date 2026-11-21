<script setup lang="ts">
/**
 * 主题切换开关（Uiverse "polite-elephant-80"）：圆月牙 ↔ 太阳光芒。
 *
 * 点击时把「点击处（视口坐标）」作为圆形扩散动画的圆心抛给上层，
 * 由 `lib/themeTransition` 的 CIRCLE 过渡消费（750ms ease-in-out）。
 */
import type { CircleOrigin } from '../lib/themeTransition';
import type { UiTheme } from '../types/config';

const { theme } = defineProps<{ theme: UiTheme }>();

const emit = defineEmits<{
  toggle: [origin: CircleOrigin];
}>();

/** 圆心 = 点击位置；取不到坐标时退回按钮中心。 */
function originOf(event: MouseEvent): CircleOrigin {
  if (event.clientX > 0 || event.clientY > 0) {
    return { x: event.clientX, y: event.clientY };
  }
  const target = event.currentTarget;
  if (!(target instanceof Element)) return { x: 0, y: 0 };
  const rect = target.getBoundingClientRect();
  return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
}

function onClick(event: MouseEvent): void {
  emit('toggle', originOf(event));
}
</script>

<template>
  <button
    type="button"
    class="toggle-btn"
    :class="{ 'is-dark': theme === 'dark' }"
    :aria-label="theme === 'dark' ? '切换到浅色主题' : '切换到深色主题'"
    :title="theme === 'dark' ? '切换到浅色主题' : '切换到深色主题'"
    @click="onClick"
  >
    <span class="sun-rays" aria-hidden="true"></span>
    <span class="main-circle" aria-hidden="true"></span>
  </button>
</template>

<style scoped>
.toggle-btn {
  --bg: var(--switch-bg);
  --fg: var(--switch-fg);
  --dimensions: 44px;
  position: relative;
  display: flex;
  align-items: center;
  justify-content: center;
  width: var(--dimensions);
  height: var(--dimensions);
  margin: 0;
  padding: 0;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background-color: var(--bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--fg);
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.toggle-btn:hover {
  border-color: var(--ctrl-line-hover);
  box-shadow: var(--ctrl-shadow-hover);
}

.toggle-btn .main-circle {
  --dimensions: 15px;
  position: absolute;
  width: var(--dimensions);
  height: var(--dimensions);
  border-radius: 50%;
  background-color: var(--fg);
  transition: transform 0.4s ease 0.2s;
}

.toggle-btn .main-circle::after {
  position: absolute;
  top: 1px;
  right: -1px;
  --dimensions: 11px;
  width: var(--dimensions);
  height: var(--dimensions);
  border-radius: 50%;
  background-color: var(--bg);
  content: "";
  transform: scale(1);
  transform-origin: right top;
  transition: transform 0.4s ease 0.2s;
}

.toggle-btn .sun-rays {
  position: absolute;
  display: grid;
  place-items: center;
  transform: scale(0);
  transition: transform 0.4s ease 0.2s;
}

.toggle-btn .sun-rays,
.toggle-btn .sun-rays::before,
.toggle-btn .sun-rays::after {
  --width: 3px;
  --height: 4px;
  position: absolute;
  width: var(--width);
  height: var(--height);
  background-color: var(--fg);
  box-shadow: 0 10px 0 var(--fg), 0 -10px 0 var(--fg);
}

.toggle-btn .sun-rays::before {
  content: "";
  transform: rotate(120deg);
}

.toggle-btn .sun-rays::after {
  content: "";
  transform: rotate(240deg);
}

.toggle-btn.is-dark .sun-rays {
  transform: scale(1);
}

.toggle-btn.is-dark .main-circle::after {
  transform: scale(0);
}
</style>
