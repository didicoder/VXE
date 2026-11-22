<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, ref } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import PopupWindow from './PopupWindow.vue';
import TooltipWindow from './TooltipWindow.vue';
import * as ipc from '../lib/ipc';
import { initializeWindow } from '../lib/windowLifecycle';
import { useWallpaper } from '../composables/useWallpaper';

type OverlayMode = 'popup' | 'tooltip';

const mode = ref<OverlayMode>('popup');
useWallpaper();
const initialized = ref(false);
const readyChildren = new Set<OverlayMode>();
let resolveChildrenReady: (() => void) | null = null;
const childrenReady = new Promise<void>((resolve) => {
  resolveChildrenReady = resolve;
});
let unlistenMode: UnlistenFn | null = null;
let unlistenPopup: UnlistenFn | null = null;
let unlistenTooltip: UnlistenFn | null = null;

function setMode(next: OverlayMode): void {
  mode.value = next;
}

function childReady(child: OverlayMode): void {
  readyChildren.add(child);
  if (readyChildren.size === 2) resolveChildrenReady?.();
}

async function initialize(): Promise<void> {
  unlistenMode = await listen<OverlayMode>('tray-overlay-mode', (event) => {
    setMode(event.payload === 'tooltip' ? 'tooltip' : 'popup');
  });
  unlistenPopup = await listen('popup-open', () => setMode('popup'));
  unlistenTooltip = await listen('tooltip-open', () => setMode('tooltip'));
  try {
    mode.value = await ipc.getTrayOverlayMode();
  } catch {
    mode.value = 'popup';
  }
  initialized.value = true;
  await nextTick();
  await childrenReady;
}

onMounted(() => initializeWindow(initialize));

onUnmounted(() => {
  unlistenMode?.();
  unlistenPopup?.();
  unlistenTooltip?.();
});
</script>

<template>
  <div class="tray-overlay-host" :data-mode="mode">
    <div v-show="initialized && mode === 'popup'" class="tray-overlay-host__popup">
      <PopupWindow managed :active="mode === 'popup'" window-label="popup" @ready="childReady('popup')" />
    </div>
    <div v-show="initialized && mode === 'tooltip'" class="tray-overlay-host__tooltip">
      <TooltipWindow managed :active="mode === 'tooltip'" window-label="popup" @ready="childReady('tooltip')" />
    </div>
  </div>
</template>

<style scoped>
.tray-overlay-host {
  width: 100%;
  height: 100%;
}

.tray-overlay-host__popup,
.tray-overlay-host__tooltip {
  width: 100%;
  height: 100%;
}
</style>
