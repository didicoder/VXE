<script setup lang="ts">
import { computed, defineAsyncComponent, nextTick, onMounted, onUnmounted, ref, watch, type CSSProperties } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { storeToRefs } from 'pinia';
import AppSidebar from '../components/layout/AppSidebar.vue';
import PageHeader from '../components/layout/PageHeader.vue';
import RefreshVeil from '../components/refresh/RefreshVeil.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowScale } from '../composables/useWindowScale';
import { dateLine } from '../lib/format';
import { filterDevicesByDisplay } from '../lib/deviceCard';
import * as ipc from '../lib/ipc';
import { initializeWindow } from '../lib/windowLifecycle';
import {
  circleRadius,
  startCircleThemeTransition,
  type CircleOrigin,
} from '../lib/themeTransition';
import { useCardImagesStore } from '../stores/cardImages';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';
import { useUiStore, type UiPage } from '../stores/ui';

// 未访问的页面/风格不解析、不挂载；离开页面照常卸载，不 KeepAlive。
const loadDevices = () => import('../components/devices/DevicesPage.vue');
const loadPhotoDevices = () => import('../components/devices/DevicesPhotoPage.vue');
const loadSummary = () => import('../components/summary/SummaryPage.vue');
const loadPhotoSummary = () => import('../components/summary/SummaryPhotoPage.vue');
const loadSettings = () => import('../components/settings/SettingsPage.vue');
const DevicesPage = defineAsyncComponent(loadDevices);
const DevicesPhotoPage = defineAsyncComponent(loadPhotoDevices);
const SummaryPage = defineAsyncComponent(loadSummary);
const SummaryPhotoPage = defineAsyncComponent(loadPhotoSummary);
const SettingsPage = defineAsyncComponent(loadSettings);

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

/** 用户倍率保持 1× 语义；新基准为 832×576，内部设计坐标保留 1040×720。 */
const scale = useWindowScale('ui');
const layoutScale = computed(() => 0.8 * scale.value);
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
const pagesReady = ref(false);

type DevicesPageHandle = { dismissTransient: () => boolean };
const devicesPage = ref<DevicesPageHandle | null>(null);
const photoDevicesPage = ref<DevicesPageHandle | null>(null);
const contentRef = ref<HTMLElement | null>(null);
const unlisten: UnlistenFn[] = [];

/** 刷新遮罩：圆形揭开 → 保持加载态 → 淡出到新数据。 */
type RefreshPhase = 'idle' | 'loading' | 'leaving';
const REFRESH_HOLD_MS = 550;
const REFRESH_FADE_MS = 220;
const refreshPhase = ref<RefreshPhase>('idle');
const refreshCircle = ref({ x: 0, y: 0, r: 0 });
let refreshRun = 0;
let refreshStartedAt = 0;
let refreshTimer: ReturnType<typeof setTimeout> | null = null;

const rootStyle = computed<CSSProperties>(() => ({ '--s': String(layoutScale.value) }));
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

/** 设置/经典页不读图；照片页只请求当前过滤后实际展示的设备。 */
const imageSignature = computed(() => JSON.stringify(
  config.devices.map(device => [device.id, device.card_img ?? '']),
));
const imageIds = computed(() => {
  if (!pagesReady.value || !photo.value || page.value === 'settings') return [];
  const statusById = new Map(statuses.value.map(status => [status.id, status]));
  const display = page.value === 'devices' ? settings.value.display_devices : settings.value.display_summary;
  return filterDevicesByDisplay(config.devices, statusById, display).map(device => device.id);
});
watch(imageSignature, () => cardImages.syncDevices(config.devices), { flush: 'sync' });
watch(imageIds, ids => { void cardImages.ensureImages(ids); });
let restoringPage = false;
let pageIntent = 0;
watch(page, (next) => {
  if (restoringPage) return;
  void ipc.rememberMainPage(next).catch((cause) => {
    console.error('主窗页面记忆失败', cause);
  });
}, { flush: 'sync' });

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
  const factor = host.offsetWidth > 0 && rect.width > 0
    ? rect.width / host.offsetWidth
    : layoutScale.value;
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

let finishingRefresh = false;

function beginRefresh(origin: CircleOrigin): void {
  if (refreshPhase.value !== 'idle') return;
  if (page.value === 'devices') dismissDevices();
  measureRefreshCircle(origin);
  refreshRun += 1;
  refreshStartedAt = Date.now();
  finishingRefresh = false;
  refreshPhase.value = 'loading';
  // 仅为采集失联兜底，不参与窗口加载/显示时序。
  refreshTimer = setTimeout(finishRefresh, 10_000);
}

function finishRefresh(): void {
  if (refreshPhase.value !== 'loading' || finishingRefresh) return;
  finishingRefresh = true;
  if (refreshTimer !== null) clearTimeout(refreshTimer);
  const run = refreshRun;
  const remaining = Math.max(0, REFRESH_HOLD_MS - (Date.now() - refreshStartedAt));
  refreshTimer = setTimeout(() => {
    if (run !== refreshRun) return;
    refreshPhase.value = 'leaving';
    refreshTimer = setTimeout(() => {
      if (run === refreshRun) refreshPhase.value = 'idle';
      refreshTimer = null;
    }, REFRESH_FADE_MS);
  }, remaining);
}

async function onRefresh(origin: CircleOrigin): Promise<void> {
  if (refreshPhase.value !== 'idle') return;
  beginRefresh(origin);
  try {
    await deviceStore.refreshNow();
  } finally {
    finishRefresh();
  }
}

/** 外部刷新由 Rust 采集；主窗只播放动画，不再转发采集请求。 */
function onRefreshStarted(): void {
  if (!document.hidden && !document.documentElement.classList.contains('window-inactive')) {
    beginRefresh(headerRefreshOrigin());
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

onMounted(() => initializeWindow(async () => {
  window.addEventListener('keydown', onKeydown);
  window.addEventListener('blur', onBlur);
  await subscribe();
  const initialIntent = pageIntent;
  try {
    const initialPage = await ipc.getMainPage();
    if (pageIntent === initialIntent && isPage(initialPage)) {
      restoringPage = true;
      page.value = initialPage;
      restoringPage = false;
    }
  } catch { /* 原生提示读取失败时保留默认页。 */ }
  await Promise.all([config.load(), deviceStore.loadCached()]);
  // 先读取真实风格，再挂载页面，避免 classic 用户也先解析默认 photo 页面。
  // 配置读取失败仍展示默认界面，保留设置入口。
  cardImages.syncDevices(config.devices);
  pagesReady.value = true;
  await Promise.all([
    cardImages.ensureImages(imageIds.value),
    page.value === 'settings' ? loadSettings()
      : page.value === 'devices' ? (photo.value ? loadPhotoDevices() : loadDevices())
        : (photo.value ? loadPhotoSummary() : loadSummary()),
  ]);
  // 初始化期间收到更新的页面/设备需求时，等待最新展示图片再发 ready。
  let readyImages: string;
  do {
    readyImages = JSON.stringify(imageIds.value);
    await cardImages.ensureImages(imageIds.value);
  } while (readyImages !== JSON.stringify(imageIds.value));
  await nextTick();
}));

/** 订阅切页、采集动效与卡片图片事件；配置由全局 store 订阅。 */
async function subscribe(): Promise<void> {
  try {
    unlisten.push(
      await listen<string>('show-page', (event) => {
        if (isPage(event.payload)) {
          pageIntent += 1;
          onNav(event.payload);
        }
      }),
    );
    unlisten.push(
      await listen(ipc.REFRESH_STARTED_EVENT, onRefreshStarted),
      await listen(ipc.REFRESH_DONE_EVENT, finishRefresh),
      await listen('devices-updated', finishRefresh),
    );
    unlisten.push(
      await listen<string>(ipc.CARD_IMAGE_CHANGED_EVENT, (event) => {
        if (!event.payload) return;
        cardImages.invalidate(event.payload);
        if (imageIds.value.includes(event.payload)) void cardImages.ensureImages([event.payload]);
      }),
    );
  } catch (cause) {
    console.error('事件订阅失败', cause);
  }
}

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  window.removeEventListener('blur', onBlur);
  for (const off of unlisten) off();
  unlisten.length = 0;
  refreshRun += 1;
  if (refreshTimer !== null) clearTimeout(refreshTimer);
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
        :refreshing="refreshing || refreshPhase !== 'idle'"
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
      <div v-if="pagesReady" class="page-host" :class="{ 'page-host--hidden': refreshPhase === 'loading' }">
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
          <!-- 独立容器固定624高，避免异步页的height:100%与父级同节点样式竞争。 -->
          <div v-else key="settings" class="settings-host">
            <SettingsPage />
          </div>
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
/* 逻辑画布 1040×720；0.8 为新的 1×。zoom 按目标尺寸重排/栅格化文字，
   避免根 transform 把整页合成后再缩小。原生窗口 = 832×576 × 用户倍率。 */
.main-window {
  position: relative;
  width: 1040px;
  height: 720px;
  overflow: hidden;
  border-radius: var(--r-window);
  box-shadow: var(--win-shadow);
  color: var(--txt);
  zoom: var(--s, 0.8);
  --fs-h2: 22px;
  --fs-label: 20px;
  --fs-body: 17px;
  --fs-hint: 15px;
  --fs-micro: 14px;
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

/* 仅主面板补偿小字，浮层仍沿用各自字号；按最终 zoom 尺寸栅格化。 */
.main-window :deep(.nav__label),
.main-window :deep(.quit__label),
.main-window :deep(.ph__add) { font-size: 20px; }
.main-window :deep(.ph__title) { font-size: 32px; }
.main-window :deep(.ph__subtitle) {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  font-size: 17px;
}
.main-window :deep(.ph__seg button) { font-size: 14px; }
.main-window :deep(.info__line) {
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 14px;
}
.main-window :deep(.head__hint),
.main-window :deep(.status),
.main-window :deep(.hero__online),
.main-window :deep(.hero__label) { font-size: 15px; }
.main-window :deep(.row__hint) { font-size: 15px; line-height: 17px; }
.main-window :deep(.row--sub .row__label) { font-size: 17px; line-height: 18px; }
.main-window :deep(.row--sub .row__hint) { font-size: 14px; line-height: 16px; }
.main-window :deep(.pcard__model) { font-size: clamp(13px, 4.1cqw, 22px); }
.main-window :deep(.batt__pct) { font-size: clamp(13px, 3.8cqw, 18px); }
.main-window :deep(.batt__tag) { font-size: clamp(11px, 2.9cqw, 14px); }
.main-window :deep(.mi span) { font-size: clamp(12px, 3.25cqw, 15px); }
.main-window :deep(.mi b) { font-size: clamp(14px, 4cqw, 18px); }
.main-window :deep(.pcard--overview .pcard__model) { font-size: clamp(14px, 5.8cqw, 24px); }
.main-window :deep(.pcard--overview .batt__pct) { font-size: clamp(14px, 5.8cqw, 22px); }
.main-window :deep(.pcard--overview .batt__tag) { font-size: clamp(12px, 4.4cqw, 16px); }
.main-window :deep(.pcard--overview .mi span) { font-size: clamp(13px, 4.9cqw, 18px); }
.main-window :deep(.pcard--overview .mi b) { font-size: clamp(15px, 6.6cqw, 22px); }
.main-window :deep(.segmented__item),
.main-window :deep(.stepper__value) { font-size: 17px; }

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
