<script setup lang="ts">
/**
 * 托盘右键菜单（212×170，窗口即卡面）。
 *
 * 原生菜单无法适配 VXE 的卡面/字重；这里用第六个预热点窗口承载。
 * Rust 负责按托盘锚点定位、隐藏悬停卡/预览并聚焦；前端负责主题、动效与动作。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import LineIcon from '../components/icons/LineIcon.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowMotion } from '../composables/useWindowMotion';
import * as ipc from '../lib/ipc';
import { initializeWindow } from '../lib/windowLifecycle';
import { useConfigStore } from '../stores/config';
import appIconDark from '../assets/app-icon-dark.png';
import appIconLight from '../assets/app-icon-light.png';

defineProps<{ windowLabel?: string }>();

const config = useConfigStore();
const themeClass = useThemeClass();
useWallpaper();
/** 菜单设计尺寸 212×170 即 1×，不再跟随主窗缩放。 */
const scale = 1;
const { entering, leaving, enter, leave } = useWindowMotion();
const isLight = computed(() => themeClass.value === 'theme-light');
const busy = ref(false);

let unlistenOpen: UnlistenFn | null = null;

async function hideWindow(): Promise<boolean> {
  return leave(() => getCurrentWindow().hide());
}

async function run(action: () => void | Promise<void>): Promise<void> {
  if (busy.value) return;
  busy.value = true;
  try {
    if (!(await hideWindow())) return;
    await action();
  } finally {
    busy.value = false;
  }
}

function openPanel(): void {
  void run(() => ipc.showMain('summary'));
}

/** 交给 Rust 刷新：可见面板根据开始事件播放遮罩，不需要创建主窗。 */
function refresh(): void {
  void run(() => emit(ipc.REFRESH_REQUEST_EVENT));
}

function quit(): void {
  void run(() => ipc.quitApp());
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || event.defaultPrevented) return;
  event.preventDefault();
  void hideWindow();
}

function onBlur(): void {
  if (!busy.value) void hideWindow();
}

onMounted(() => initializeWindow(async () => {
  await config.load();
  unlistenOpen = await listen('tray-menu-open', () => {
    void enter();
    void config.load();
  });
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('blur', onBlur);
}));

onUnmounted(() => {
  unlistenOpen?.();
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('blur', onBlur);
});
</script>

<template>
  <section
    class="tray-menu"
    :class="{ 'window-enter': entering, 'window-leave': leaving }"
    :style="{ '--s': scale }"
  >
    <div class="menu-card glass-window" role="menu" aria-label="托盘菜单">
      <header class="menu-head">
        <img
          class="menu-head__icon"
          :src="isLight ? appIconLight : appIconDark"
          alt=""
          aria-hidden="true"
          draggable="false"
        >
        <span class="menu-head__title">VXE</span>
        <span class="menu-head__meta">本地 HID</span>
      </header>

      <button type="button" class="menu-item" role="menuitem" @click="openPanel">
        <LineIcon name="gauge" :size="17" :stroke="1.8" />
        <span>打开面板</span>
      </button>

      <button type="button" class="menu-item" role="menuitem" @click="refresh">
        <LineIcon name="refresh" :size="17" :stroke="1.8" />
        <span>立即刷新</span>
      </button>

      <span class="menu-sep" role="separator" />

      <button
        type="button"
        class="menu-item menu-item--quit"
        role="menuitem"
        @click="quit"
      >
        <LineIcon name="power" :size="17" :stroke="1.8" />
        <span>退出应用</span>
      </button>
    </div>
  </section>
</template>

<style scoped>
.tray-menu {
  width: 212px;
  height: 170px;
  transform: scale(var(--s));
  transform-origin: top left;
  color: var(--txt);
  font-family: var(--font-body);
}

.menu-card {
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  padding: 8px;
  overflow: hidden;
  border: 0;
  border-radius: var(--r-card);
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
}

.menu-head {
  display: flex;
  flex: none;
  gap: 9px;
  align-items: center;
  height: 34px;
  padding: 0 6px;
}

.menu-head__icon {
  display: block;
  flex: none;
  width: 24px;
  height: 24px;
}

.menu-head__title {
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

.menu-head__meta {
  margin-left: auto;
  color: var(--muted);
  font-size: 11.5px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
}

.menu-item {
  display: flex;
  flex: none;
  gap: 10px;
  align-items: center;
  width: 100%;
  height: 37px;
  padding: 0 10px;
  border: 0;
  border-radius: var(--r-ctrl);
  background: transparent;
  color: var(--txt);
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
  text-align: left;
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.menu-item span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.menu-item:hover {
  background: var(--surface-hover);
}

.menu-item:focus-visible {
  background: var(--surface-hover);
  outline: 1px solid var(--accent);
  outline-offset: -1px;
}

.menu-item--quit:hover {
  background: color-mix(in srgb, var(--low) 12%, var(--card));
  color: var(--ink-low);
}

.menu-sep {
  display: block;
  flex: none;
  height: 1px;
  margin: 4px 6px;
  background: var(--surface-stroke);
}
</style>
