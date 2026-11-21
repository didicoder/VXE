<script setup lang="ts">
/**
 * 托盘悬停卡：设计宽 243.1，设计尺寸即 1×，当前只乘 tip_scale。
 * base 仅用于旧配置兼容/迁移，不参与当前尺寸。
 *
 * 顶部箭头 8.5、左右内边距 17、头部 32.3、行高 42.5、最多 6 行、无按钮。
 * 显示/隐藏完全由 Rust 的托盘 hover 控制，这里不做自动隐藏；2 s 读一次缓存兜底。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { storeToRefs } from 'pinia';
import TipRow from '../components/tooltip/TipRow.vue';
import { filterStatusesByDisplay } from '../lib/deviceCard';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowMotion } from '../composables/useWindowMotion';
import { useWindowScale } from '../composables/useWindowScale';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';

defineProps<{ windowLabel?: string }>();

/** 悬停卡最多 6 行（再多也没人看）。 */
const MAX_ROWS = 6;
/** 兜底同步间隔：错过 `devices-updated` 也能在 2 s 内追平。 */
const SYNC_MS = 2000;

const config = useConfigStore();
const devices = useDevicesStore();
const { settings } = storeToRefs(config);
const { statuses } = storeToRefs(devices);

const scale = useWindowScale('tip');
useThemeClass();
useWallpaper();
const { entering, leaving, enter, leave } = useWindowMotion();

const visible = computed(() =>
  filterStatusesByDisplay(statuses.value, settings.value.display_tip).slice(0, MAX_ROWS),
);
const syncedAt = ref<number | null>(null);
const syncText = computed(() => {
  const ts = syncedAt.value;
  if (ts == null) return '刷新 --:--:--';
  const date = new Date(ts);
  const hh = String(date.getHours()).padStart(2, '0');
  const mm = String(date.getMinutes()).padStart(2, '0');
  const ss = String(date.getSeconds()).padStart(2, '0');
  return `刷新 ${hh}:${mm}:${ss}`;
});

let unlistenUpdated: UnlistenFn | null = null;
let unlistenOpen: UnlistenFn | null = null;
let unlistenClose: UnlistenFn | null = null;
let timer: ReturnType<typeof setInterval> | null = null;

async function sync(): Promise<void> {
  await devices.loadCached();
  syncedAt.value = Date.now();
}

async function hideWindow(): Promise<void> {
  await leave(async () => {
    try {
      await getCurrentWindow().hide();
    } catch {
      /* 非 Tauri 环境忽略 */
    }
  });
}

onMounted(async () => {
  unlistenUpdated = await listen('devices-updated', () => {
    void sync();
  });
  unlistenOpen = await listen('tooltip-open', () => {
    void enter();
    void sync();
  });
  unlistenClose = await listen('tooltip-close', () => {
    void hideWindow();
  });
  await config.load();
  await sync();
  timer = setInterval(() => {
    void sync();
  }, SYNC_MS);
});

onUnmounted(() => {
  unlistenUpdated?.();
  unlistenOpen?.();
  unlistenClose?.();
  if (timer !== null) {
    clearInterval(timer);
    timer = null;
  }
});
</script>

<template>
  <section
    class="tip"
    :class="{ 'window-enter': entering, 'window-leave': leaving }"
    :style="{ '--s': scale }"
  >
    <span class="tip__arrow" />
    <div class="tip__card glass-window">
      <header class="tip__head">
        <span>VXE</span>
        <span>{{ syncText }}</span>
      </header>
      <TipRow
        v-for="device in visible"
        :key="device.id"
        :device="device"
        :good="settings.good_level"
        :warn="settings.warn_level"
        :band-colors="settings.band_colors"
      />
      <p v-if="!visible.length" class="tip__empty">还没有添加设备</p>
    </div>
  </section>
</template>

<style scoped>
.tip {
  position: relative;
  width: 243.1px;
  padding-top: 6.8px;
  transform: scale(var(--s));
  transform-origin: top left;
  color: var(--txt);
  font-family: var(--font-body);
}

.tip__arrow {
  position: absolute;
  top: 2.55px;
  left: 50%;
  width: 8.5px;
  height: 8.5px;
  border-top: 1px solid var(--card-line);
  border-left: 1px solid var(--card-line);
  border-top-left-radius: 1.7px;
  background: var(--card-bg);
  transform: translateX(-50%) rotate(45deg);
}

/* 悬停卡：壁纸走 .glass-window；描边用 inset，避免 1px 边框把卡高顶出窗口 */
.tip__card {
  padding: 0 17px 3.4px;
  border: 0;
  border-radius: var(--r-tile);
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
}

.tip__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 32.3px;
  color: var(--ink-3);
  font-size: 11.05px;
  font-weight: 400;
  line-height: 1;
}

.tip__empty {
  display: flex;
  align-items: center;
  height: 42.5px;
  margin: 0;
  color: var(--ink-3);
  font-size: 11.05px;
  font-weight: 400;
  line-height: 1;
}
</style>
