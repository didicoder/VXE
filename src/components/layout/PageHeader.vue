<script setup lang="ts">
import LineIcon from '../icons/LineIcon.vue';
import RefreshIcon from '../icons/RefreshIcon.vue';
import ThemeToggle from '../ThemeToggle.vue';
import type { CircleOrigin } from '../../lib/themeTransition';
import type { UiTheme } from '../../types/config';

const {
  title,
  subtitle,
  theme,
  refreshing = false,
  showAdd = true,
  photoMode = false,
  photoTools = false,
  editing = false,
  cols = 3,
} = defineProps<{
  title: string;
  subtitle: string;
  theme: UiTheme;
  refreshing?: boolean;
  showAdd?: boolean;
  /** 实物卡片 UI：标题右侧显示网格按钮 / 每行张数。 */
  photoMode?: boolean;
  /** 当前页是设备页（只有这里出现网格按钮与每行张数）。 */
  photoTools?: boolean;
  editing?: boolean;
  cols?: 2 | 3;
}>();

const emit = defineEmits<{
  add: [];
  theme: [origin: CircleOrigin];
  refresh: [origin: CircleOrigin];
  minimize: [];
  close: [];
  toggleEdit: [];
  cols: [value: 2 | 3];
}>();

/** 刷新动画圆心：按钮中心（视口坐标）。 */
function emitRefresh(event: MouseEvent): void {
  const target = event.currentTarget;
  if (!(target instanceof Element)) {
    emit('refresh', { x: 0, y: 0 });
    return;
  }
  const rect = target.getBoundingClientRect();
  emit('refresh', { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 });
}
</script>

<template>
  <!-- 页头整块（标题 + 空白 + 按钮间隙）都是拖动区；按钮自身没有该属性，不会误拖 -->
  <header class="ph" data-tauri-drag-region>
    <Transition name="ph-text" mode="out-in">
      <h1 :key="title" class="ph__title">{{ title }}</h1>
    </Transition>
    <Transition name="ph-text" mode="out-in">
      <p :key="subtitle" class="ph__subtitle">{{ subtitle }}</p>
    </Transition>

    <button
      v-if="photoMode && photoTools"
      type="button"
      class="ph__cfg"
      :class="{ on: editing }"
      :title="editing ? '退出卡片编辑' : '卡片设置（进入编辑）'"
      aria-label="卡片设置"
      @click="emit('toggleEdit')"
    >
      <svg class="gridicon" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <rect x="3.2" y="3.2" width="5" height="5" rx="1.5" />
        <rect x="9.5" y="3.2" width="5" height="5" rx="1.5" />
        <rect x="15.8" y="3.2" width="5" height="5" rx="1.5" />
        <rect x="3.2" y="9.5" width="5" height="5" rx="1.5" />
        <rect x="9.5" y="9.5" width="5" height="5" rx="1.5" />
        <rect x="15.8" y="9.5" width="5" height="5" rx="1.5" />
        <rect x="3.2" y="15.8" width="5" height="5" rx="1.5" />
        <rect x="9.5" y="15.8" width="5" height="5" rx="1.5" />
        <rect x="15.8" y="15.8" width="5" height="5" rx="1.5" />
      </svg>
    </button>

    <div v-if="photoMode && photoTools" class="ph__seg" title="每行卡片张数">
      <button type="button" :class="{ on: cols === 2 }" @click="emit('cols', 2)">2 张</button>
      <button type="button" :class="{ on: cols === 3 }" @click="emit('cols', 3)">3 张</button>
    </div>

    <div class="ph__actions">
      <button
        v-if="showAdd"
        type="button"
        class="ph__add"
        @click="emit('add')"
      >
        <LineIcon name="plus" :size="18" :stroke="2" />
        <span>添加设备</span>
      </button>

      <ThemeToggle :theme="theme" @toggle="emit('theme', $event)" />

      <button type="button" class="icon-btn" aria-label="刷新" @click="emitRefresh">
        <RefreshIcon :size="18" :stroke="2" :busy="refreshing" />
      </button>

      <button type="button" class="icon-btn" aria-label="最小化" @click="emit('minimize')">
        <span class="ph__minus"></span>
      </button>

      <button
        type="button"
        class="icon-btn icon-btn--close"
        aria-label="关闭"
        @click="emit('close')"
      >
        <LineIcon name="close" :size="18" :stroke="3.2" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.ph {
  position: relative;
  height: 88px;
}

/* 标题 (200,46) 30 bold / 副标 (200,74) 15 regular：left 相对内容区左缘 */
.ph__title {
  position: absolute;
  top: 46px;
  left: 0;
  margin: 0;
  font-size: 30px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.015em;
  white-space: nowrap;
  transform: translateY(-50%);
  pointer-events: none;
  color: var(--ink);
}

.ph__subtitle {
  position: absolute;
  top: 74px;
  left: 0;
  margin: 0;
  color: var(--ink-2);
  font-size: 15px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  transform: translateY(-50%);
  pointer-events: none;
}

/* 设备页标题右侧的网格设置按钮（9 点） */
.ph__cfg {
  position: absolute;
  top: 46px;
  left: 76px;
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  padding: 0;
  border: 1px solid transparent;
  border-radius: 10px;
  background: transparent;
  color: var(--ink-3);
  cursor: pointer;
  transform: translateY(-50%);
  transition:
    background var(--speed) var(--ease),
    color var(--speed) var(--ease),
    border-color var(--speed) var(--ease);
}

.ph__cfg:hover {
  border-color: var(--ctrl-line);
  background: var(--ctrl-bg);
  color: var(--ink);
}

.ph__cfg.on {
  border-color: transparent;
  background: var(--accent);
  color: var(--accent-fg);
  box-shadow: 0 6px 16px -6px color-mix(in srgb, var(--accent) 90%, transparent);
}

.gridicon {
  display: block;
  width: 20px;
  height: 20px;
}

.gridicon rect {
  transform-box: fill-box;
  transform-origin: center;
  transition: transform 0.22s var(--ease);
}

.ph__cfg:hover .gridicon rect {
  transform: scale(1.18);
}

.ph__cfg:hover .gridicon rect:nth-child(1) { transition-delay: 0ms; }
.ph__cfg:hover .gridicon rect:nth-child(2) { transition-delay: 30ms; }
.ph__cfg:hover .gridicon rect:nth-child(3) { transition-delay: 60ms; }
.ph__cfg:hover .gridicon rect:nth-child(4) { transition-delay: 90ms; }
.ph__cfg:hover .gridicon rect:nth-child(5) { transition-delay: 120ms; }
.ph__cfg:hover .gridicon rect:nth-child(6) { transition-delay: 150ms; }
.ph__cfg:hover .gridicon rect:nth-child(7) { transition-delay: 180ms; }
.ph__cfg:hover .gridicon rect:nth-child(8) { transition-delay: 210ms; }
.ph__cfg:hover .gridicon rect:nth-child(9) { transition-delay: 240ms; }

/* 每行张数：与网格按钮同排 */
.ph__seg {
  position: absolute;
  top: 46px;
  left: 120px;
  display: flex;
  align-items: center;
  gap: 2px;
  height: 30px;
  padding: 2px;
  border: 1px solid var(--ctrl-line);
  border-radius: 9px;
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  transform: translateY(-50%);
}

.ph__seg button {
  height: 24px;
  padding: 0 9px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition:
    background 0.16s ease,
    color 0.16s ease;
}

.ph__seg button:hover {
  color: var(--ink);
}

.ph__seg button.on {
  background: #fff;
  color: #16181d;
  box-shadow: 0 1px 3px rgba(20, 20, 45, 20%);
}

/* 标题文字只淡入淡出，保留 translateY(-50%) 的垂直居中 */
.ph-text-enter-active {
  transition: opacity var(--dur-page-enter) var(--ease-out);
}

.ph-text-leave-active {
  transition: opacity var(--dur-page-exit) var(--ease-in-out);
}

.ph-text-enter-from,
.ph-text-leave-to {
  opacity: 0;
}

/* 右上从右往左：关闭 / 最小化 / 刷新 / 主题（44×44、间距 4），添加设备 142×44 */
.ph__actions {
  position: absolute;
  top: 20px;
  right: 0;
  display: flex;
  align-items: center;
  gap: 4px;
  height: 44px;
}

.ph__add {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  width: 142px;
  height: 44px;
  margin-right: 4px;
  padding: 0 0 2px;
  border: 1px solid var(--cta-line);
  border-radius: var(--r-pill);
  background: var(--cta-bg);
  box-shadow: var(--cta-shadow);
  color: var(--cta-fg);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    transform var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.ph__add:hover {
  transform: translateY(-1px);
  box-shadow: var(--cta-shadow-hover);
}

.ph__add:active {
  transform: translateY(0);
}

/* 最小化：20×3 实心横线（V16 画的是没有圆帽的 line） */
.ph__minus {
  width: 18px;
  height: 2.5px;
  border-radius: 1.25px;
  background: currentColor;
}

</style>
