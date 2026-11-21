<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch, type CSSProperties } from 'vue';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { storeToRefs } from 'pinia';
import AppSidebar from '../components/layout/AppSidebar.vue';
import PageHeader from '../components/layout/PageHeader.vue';
import DevicesPage from '../components/devices/DevicesPage.vue';
import DevicesPhotoPage from '../components/devices/DevicesPhotoPage.vue';
import RefreshVeil from '../components/refresh/RefreshVeil.vue';
import SummaryPage from '../components/summary/SummaryPage.vue';
import SummaryPhotoPage from '../components/summary/SummaryPhotoPage.vue';
import SettingsPage from '../components/settings/SettingsPage.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowScale } from '../composables/useWindowScale';
import { dateLine } from '../lib/format';
import * as ipc from '../lib/ipc';
import {
  circleRadius,
  startCircleThemeTransition,
  type CircleOrigin,
} from '../lib/themeTransition';
import { useCardImagesStore } from '../stores/cardImages';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';
import { useUiStore, type UiPage } from '../stores/ui';

defineProps<{ windowLabel?: string }>();

const PAGES: Record<UiPage, string> = {
  summary: '总览',
  devices: '设备',
  settings: '设置',
};

function isPage(value: unknown): value is UiPage {
  return value === 'summary' || value === 'devices' || value === 'settings';
}

const config = useConfigStore();
const deviceStore = useDevicesStore();
const cardImages = useCardImagesStore();
const ui = useUiStore();

/** 只算缩放；窗口物理尺寸由 Rust `set_size(1040×scale, 720×scale)` 决定。 */
const scale = useWindowScale('ui');
useThemeClass();
useWallpaper();

const { settings } = storeToRefs(config);
const { statuses, refreshing } = storeToRefs(deviceStore);
const { page, theme } = storeToRefs(ui);

/** 3.1：默认实物卡片 UI，设置里可切回 3.0 玻璃卡片。 */
const photo = computed(() => settings.value.ui_style !== 'classic');
const collapsed = computed(() => settings.value.sidebar_collapsed);

/** 设备页实物卡片的编辑态 / 当前打开的卡片设置。 */
const photoEditing = ref(false);
const activeCardId = ref<string | null>(null);

const devicesPage = ref<InstanceType<typeof DevicesPage> | null>(null);
const photoDevicesPage = ref<InstanceType<typeof DevicesPhotoPage> | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const unlisten: UnlistenFn[] = [];

/** 刷新遮罩：圆形揭开 → 保持加载态 → 淡出到新数据。 */
type RefreshPhase = 'idle' | 'loading' | 'leaving';
const REFRESH_HOLD_MS = 550;
const REFRESH_FADE_MS = 220;
const refreshPhase = ref<RefreshPhase>('idle');
const refreshCircle = ref({ x: 0, y: 0, r: 0 });
let refreshRun = 0;

const rootStyle = computed<CSSProperties>(() => ({ '--s': String(scale.value) }));
const refreshVeilStyle = computed<CSSProperties>(() => ({
  '--refresh-x': `${refreshCircle.value.x}px`,
  '--refresh-y': `${refreshCircle.value.y}px`,
  '--refresh-r': `${refreshCircle.value.r}px`,
}));

/** 只算真正接在本机上的设备（`absent` = 后端没读到这台硬件，不进列表也不进计数）。 */
const presentStatuses = computed(() =>
  statuses.value.filter((status) => status.state !== 'absent'),
);

/** 骨架数量跟当前页真实条目一致，避免 1 台设备闪出 5 行占位。 */
const refreshCount = computed(() => {
  const count = presentStatuses.value.length;
  if (page.value === 'summary') {
    return photo.value ? Math.min(3, count) : Math.min(4, count);
  }
  if (page.value === 'devices') {
    return photo.value ? Math.min(4, Math.max(2, count)) : Math.min(5, Math.max(1, count));
  }
  return 3;
});
const title = computed(() => PAGES[page.value]);

/** 配置里有、但这会儿没接上的设备数（只提一句，不占卡片位）。 */
const offlineCount = computed(() =>
  Math.max(0, config.devices.length - presentStatuses.value.length),
);

const subtitle = computed(() => {
  const count = presentStatuses.value.length;
  const offline = offlineCount.value > 0 ? `（另有 ${offlineCount.value} 台未连接）` : '';
  if (page.value === 'devices') {
    return photo.value
      ? `${count} 台设备${offline} · 网格按钮可编辑卡片，右侧可选每行张数`
      : `${count} 台设备 · 可添加、重命名或移除`;
  }
  if (page.value === 'settings') return '刷新频率、颜色阈值与启动项';
  return `${dateLine()} · ${count} 台设备`;
});

/** 设备卡片图片随设备列表 / 自选图标记变化重新载入。 */
const imageSignature = computed(() =>
  config.devices.map((device) => `${device.id}:${device.card_img ?? ''}`).join('|'),
);

watch(imageSignature, () => {
  void cardImages.loadAll(config.devices);
});

function onNav(next: UiPage): void {
  if (next !== page.value) {
    photoEditing.value = false;
    activeCardId.value = null;
  }
  page.value = next;
}

function togglePhotoEdit(): void {
  photoEditing.value = !photoEditing.value;
  if (!photoEditing.value) activeCardId.value = null;
}

function onPhotoCols(value: 2 | 3): void {
  void config.setSetting('photo_cols', value);
}

function toggleSidebar(): void {
  void config.setSetting('sidebar_collapsed', !settings.value.sidebar_collapsed);
}

watch(photo, (isPhoto) => {
  if (!isPhoto) {
    photoEditing.value = false;
    activeCardId.value = null;
  }
});

function dismissDevices(): boolean {
  const target = photo.value ? photoDevicesPage.value : devicesPage.value;
  return target?.dismissTransient() ?? false;
}

async function onAdd(): Promise<void> {
  try {
    await ipc.showAdd();
  } catch (cause) {
    console.error('show_add failed', cause);
  }
}

function onTheme(origin: CircleOrigin): void {
  const next = theme.value === 'dark' ? 'light' : 'dark';
  startCircleThemeTransition(
    origin,
    async () => {
      await ui.setTheme(next);
      // 等 `<html>` 的 theme class 落到 DOM 后再截新帧，否则新帧还是旧主题。
      await nextTick();
    },
    // reverse auto（baiwumm CIRCLE）：切暗正向扩散、切亮向触发点收起。
    { reverse: next === 'light' },
  );
}

/** 按钮中心（视口坐标）换算到遮罩局部坐标；页头 88px 不覆盖。 */
function measureRefreshCircle(origin: CircleOrigin): void {
  const host = contentRef.value;
  if (!host) return;
  const rect = host.getBoundingClientRect();
  const factor = scale.value || 1;
  const x = (origin.x - rect.left) / factor;
  const y = (origin.y - rect.top) / factor - 88;
  const width = rect.width / factor;
  const height = rect.height / factor - 88;
  refreshCircle.value = {
    x,
    y,
    r: circleRadius({ x, y }, width, height) + 8,
  };
}

/** 托盘菜单「立即刷新」没有鼠标位置：以页头刷新按钮中心作为圆形动画起点。 */
function headerRefreshOrigin(): CircleOrigin {
  const host = contentRef.value;
  if (!host) return { x: 0, y: 0 };
  const button = host.querySelector<HTMLElement>('button[aria-label="刷新"]');
  const rect = (button ?? host).getBoundingClientRect();
  return { x: rect.left + rect.width / 2, y: rect.top + rect.height / 2 };
}

async function onRefresh(origin: CircleOrigin): Promise<void> {
  if (refreshPhase.value !== 'idle') return;
  if (page.value === 'devices') dismissDevices();
  measureRefreshCircle(origin);
  const run = ++refreshRun;
  refreshPhase.value = 'loading';

  const hold = new Promise<void>((resolve) => setTimeout(resolve, REFRESH_HOLD_MS));
  await Promise.all([deviceStore.refreshNow(), hold]);
  if (run !== refreshRun) return;

  refreshPhase.value = 'leaving';
  await new Promise<void>((resolve) => setTimeout(resolve, REFRESH_FADE_MS));
  if (run === refreshRun) refreshPhase.value = 'idle';
}

/** 菜单/预览卡等外部入口：统一由主窗采集并播放圆形刷新，完成后广播 done。 */
async function onRefreshRequest(): Promise<void> {
  try {
    if (refreshPhase.value !== 'idle') {
      // 面板正在刷新：不打断动效，只补一次采集。
      await deviceStore.refreshNow();
    } else {
      await onRefresh(headerRefreshOrigin());
    }
  } finally {
    void emit(ipc.REFRESH_DONE_EVENT);
  }
}

async function onMinimize(): Promise<void> {
  await getCurrentWindow().minimize();
}

async function onClose(): Promise<void> {
  await getCurrentWindow().hide();
  ui.closeMain();
}

async function onQuit(): Promise<void> {
  await ipc.quitApp();
}

/** Esc：先收设备页的临时态（选择器 / 确认删除 / 改名框），再关窗。 */
function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || event.defaultPrevented) return;
  if (page.value === 'devices' && dismissDevices()) return;
  void onClose();
}

/** 点主窗空白处、页头或侧栏都算「点到别处」：退出删除确认等临时态。 */
function onSurfaceClick(): void {
  if (page.value === 'devices') dismissDevices();
}

/** 窗口失焦（点到桌面/别的窗口）同样收掉临时态。
 * 但实物卡片 UI 例外：抽屉不能在失焦时收起——打开系统选图框（或任何文件对话框）
 * 都会让主窗失焦，抽屉一毁，换图事件就发给了已卸载的组件，表现为「选完图没反应」。
 * classic 页没有取图入口，保留原行为（收掉改名框 / 删除确认）。 */
function onBlur(): void {
  if (!photo.value && page.value === 'devices') dismissDevices();
}

onMounted(async () => {
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('blur', onBlur);
  await config.load();
  await deviceStore.loadCached();
  await deviceStore.refreshNow();
  deviceStore.startPolling(settings.value.refresh_seconds);
  void cardImages.loadAll(config.devices);
  await subscribe();
});

/** 订阅 Rust 侧事件：`show-page` 切页、`config-changed` 重读配置。 */
async function subscribe(): Promise<void> {
  try {
    unlisten.push(
      await listen<string>('show-page', (event) => {
        if (isPage(event.payload)) page.value = event.payload;
      }),
    );
    unlisten.push(
      await listen(ipc.REFRESH_REQUEST_EVENT, () => {
        void onRefreshRequest();
      }),
    );
    unlisten.push(
      await listen('config-changed', () => {
        void config.load();
      }),
    );
    unlisten.push(
      await listen<string>(ipc.CARD_IMAGE_CHANGED_EVENT, (event) => {
        if (event.payload) void cardImages.loadOne(event.payload);
      }),
    );
    } catch (cause) {
    console.error('事件订阅失败', cause);
  }
}

/** 设置页改刷新间隔后立刻换挡，不必重启应用。 */
watch(
  () => settings.value.refresh_seconds,
  (seconds) => {
    deviceStore.startPolling(seconds);
  },
);

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('blur', onBlur);
  for (const off of unlisten) off();
  unlisten.length = 0;
  deviceStore.stopPolling();
});
</script>

<template>
  <div class="main-window glass-window" :style="rootStyle" data-window="main" @click="onSurfaceClick">
    <AppSidebar
      :page="page"
      :theme="theme"
      :variant="photo ? 'photo' : 'classic'"
      :collapsed="collapsed"
      @nav="onNav"
      @quit="onQuit"
      @toggle-collapse="toggleSidebar"
    />

    <div
      ref="contentRef"
      class="main-content"
      :class="{ 'main-content--min': collapsed }"
      :aria-busy="refreshPhase !== 'idle'"
    >
      <PageHeader
        :title="title"
        :subtitle="subtitle"
        :theme="theme"
        :refreshing="refreshing"
        :show-add="page !== 'settings'"
        :photo-mode="photo"
        :photo-tools="photo && page === 'devices'"
        :editing="photoEditing"
        :cols="settings.photo_cols"
        @add="onAdd"
        @theme="onTheme"
        @refresh="onRefresh"
        @minimize="onMinimize"
        @close="onClose"
        @toggle-edit="togglePhotoEdit"
        @cols="onPhotoCols"
      />

      <!-- 刷新「loading」期隐藏页面内容：骨架卡直接落在窗口壁纸（页面背景）上 -->
      <div class="page-host" :class="{ 'page-host--hidden': refreshPhase === 'loading' }">
        <Transition name="page" mode="out-in">
          <SummaryPhotoPage
            v-if="page === 'summary' && photo"
            key="summary-photo"
            @open-devices="onNav('devices')"
          />
          <SummaryPage v-else-if="page === 'summary'" key="summary" />
          <DevicesPhotoPage
            v-else-if="page === 'devices' && photo"
            key="devices-photo"
            ref="photoDevicesPage"
            :editing="photoEditing"
            :active-id="activeCardId"
            @open-card="activeCardId = $event"
            @close-card="activeCardId = null"
          />
          <DevicesPage v-else-if="page === 'devices'" key="devices" ref="devicesPage" />
          <!-- 设置页自己从视口 y=96 起排（页头 88 + 8），容器高度 624 -->
          <SettingsPage v-else key="settings" class="settings-host" />
        </Transition>
      </div>

      <RefreshVeil
        v-if="refreshPhase !== 'idle'"
        :class="{ 'refresh-veil--min': collapsed }"
        :page="page"
        :count="refreshCount"
        :leaving="refreshPhase === 'leaving'"
        :photo="photo"
        :cols="settings.photo_cols"
        :style="refreshVeilStyle"
      />
    </div>
  </div>
</template>

<style scoped>
/* 设计尺寸 1040×720，整页按 --s 缩放；窗口物理尺寸由 Rust 设置。壁纸走 .glass-window */
.main-window {
  position: relative;
  width: 1040px;
  height: 720px;
  overflow: hidden;
  border-radius: var(--r-window);
  box-shadow: var(--win-shadow);
  color: var(--txt);
  transform: scale(var(--s, 1));
  transform-origin: top left;
}

.main-content {
  position: absolute;
  top: 0;
  left: 180px;
  width: 860px;
  height: 720px;
  padding: 0 24px 0 20px;
  transition:
    left 0.26s var(--ease),
    width 0.26s var(--ease),
    padding 0.26s var(--ease);
}

/* 侧栏收起：内容区右扩到 980，里面的 816 页面重新居中（(980-816)/2 = 82） */
.main-content--min {
  left: 60px;
  width: 980px;
  padding: 0 82px;
}

.settings-host {
  display: block;
  height: 624px;
  margin-top: 8px;
}

/* 刷新 loading 期隐藏页面内容（保留占位），骨架卡落在窗口壁纸上 */
.page-host--hidden {
  visibility: hidden;
}

/* 页切换：旧页先淡出 140ms，新页再淡入 200ms；只动 opacity/transform */
.page-enter-active {
  transition:
    opacity var(--dur-page-enter) var(--ease-out),
    transform var(--dur-page-enter) var(--ease-out);
}

.page-leave-active {
  transition:
    opacity var(--dur-page-exit) var(--ease-in-out),
    transform var(--dur-page-exit) var(--ease-in-out);
}

.page-enter-from {
  opacity: 0;
  transform: translateY(6px);
}

.page-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
