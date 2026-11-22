<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, onUpdated, ref, watch } from 'vue';
import { storeToRefs } from 'pinia';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import DevicePhotoCard from './DevicePhotoCard.vue';
import EmptyDeviceCard from './EmptyDeviceCard.vue';
import LineIcon from '../icons/LineIcon.vue';
import { filterDevicesByDisplay } from '../../lib/deviceCard';
import { isImageFile, prepareCardImage } from '../../lib/cardImage';
import { reorderSubset } from '../../lib/deviceOrder';
import { devicesRowHeight } from '../../lib/photoLayout';
import * as ipc from '../../lib/ipc';
import { useCardImagesStore } from '../../stores/cardImages';
import { useConfigStore } from '../../stores/config';
import { useDevicesStore } from '../../stores/devices';
import { useDragReorder } from '../../composables/useDragReorder';
import type { CardFields, DeviceConfig } from '../../types/device';

const props = defineProps<{
  editing: boolean;
  activeId: string | null;
}>();

const emit = defineEmits<{
  'open-card': [id: string];
  'close-card': [];
}>();

const config = useConfigStore();
const deviceStore = useDevicesStore();
const cardImages = useCardImagesStore();
const { devices, settings } = storeToRefs(config);
const { statuses } = storeToRefs(deviceStore);

const statusById = computed(
  () => new Map(statuses.value.map((status) => [status.id, status])),
);

/** 按「设备页显示设备」过滤：显示已连接时，拔掉的接收器 / 关着的手柄不占卡片位。 */
const presentDevices = computed(() =>
  filterDevicesByDisplay(devices.value, statusById.value, settings.value.display_devices),
);
const hiddenCount = computed(() => devices.value.length - presentDevices.value.length);

const gridEl = ref<HTMLElement | null>(null);
const scrollEl = ref<HTMLElement | null>(null);
let visibilityObserver: IntersectionObserver | null = null;
const observedCards = new Set<Element>();

function observeCards(): void {
  const grid = gridEl.value;
  if (!visibilityObserver || !grid) return;
  const cards = new Set(Array.from(grid.querySelectorAll<HTMLElement>('.pcard')));
  for (const card of observedCards) {
    if (cards.has(card as HTMLElement)) continue;
    visibilityObserver.unobserve(card);
    observedCards.delete(card);
  }
  for (const card of cards) {
    if (observedCards.has(card)) continue;
    observedCards.add(card);
    visibilityObserver.observe(card);
  }
}

function resetVisibilityObserver(): void {
  if (!scrollEl.value || typeof IntersectionObserver === 'undefined') return;
  visibilityObserver?.disconnect();
  observedCards.clear();
  visibilityObserver = new IntersectionObserver((entries) => {
    for (const entry of entries) {
      if (!(entry.target instanceof HTMLElement)) continue;
      const inRange = entry.isIntersecting;
      entry.target.dataset.offscreen = inRange ? 'false' : 'true';
      // drag-lift 在 CSS 中豁免离屏暂停，但不强制恢复动画。
      entry.target.classList.toggle('is-offscreen', !inRange);
    }
  }, {
    root: scrollEl.value,
    rootMargin: `${devicesRowHeight(settings.value.photo_cols)}px 0px`,
    threshold: 0,
  });
  observeCards();
}

/** 2 列一行、3 列两行，都按整页高度定卡片行高；多余卡片继续往下滚动。 */
const gridStyle = computed(() => ({
  '--row-h': `${devicesRowHeight(settings.value.photo_cols)}px`,
}));
const drag = useDragReorder({
  container: gridEl,
  handleSelector: '.pcard__grip',
  onCommit(from, to) {
    // 拖拽给的是「可见项下标」，有设备未连接时不能直接拿去动完整列表。
    const visible = presentDevices.value;
    const movingId = visible[from]?.id;
    if (!movingId || visible[to] === undefined) return;
    const order = visible.map((device) => device.id);
    order.splice(from, 1);
    order.splice(to, 0, movingId);
    devices.value = reorderSubset(devices.value, order);
    deviceStore.statuses = reorderSubset(deviceStore.statuses, order);
    void config.reorderDevices(devices.value.map((device) => device.id));
  },
});

/** 设置窗发来的实时改动：只更新主窗内存，松手 / 失焦时由设置窗落盘。 */
const previewPatch = ref<Partial<DeviceConfig>>({});
let unlisten: UnlistenFn[] = [];

/** 删除卡片：第一次点只进入确认态，3 秒内再点一次才真正删。 */
const pendingDeleteId = ref<string | null>(null);
const DELETE_CONFIRM_MS = 3000;
let deleteTimer: ReturnType<typeof setTimeout> | null = null;

function clearDeleteTimer(): void {
  if (deleteTimer !== null) {
    clearTimeout(deleteTimer);
    deleteTimer = null;
  }
}

function onDelete(device: DeviceConfig): void {
  if (pendingDeleteId.value !== device.id) {
    clearDeleteTimer();
    pendingDeleteId.value = device.id;
    deleteTimer = setTimeout(() => {
      pendingDeleteId.value = null;
      deleteTimer = null;
    }, DELETE_CONFIRM_MS);
    return;
  }
  clearDeleteTimer();
  pendingDeleteId.value = null;
  if (props.activeId === device.id) {
    hideInspectorWindow();
    emit('close-card');
  }
  void config.removeDevice(device.id).then((ok) => {
    if (ok) void deviceStore.refreshNow();
    else console.error('device delete failed', config.error);
  });
}

function imageFor(device: DeviceConfig): string | null {
  return cardImages.imageFor(device.id);
}

function deviceFor(device: DeviceConfig): DeviceConfig {
  if (device.id !== props.activeId) return device;
  const patch = previewPatch.value;
  const merged: DeviceConfig = { ...device, ...patch };
  if (patch.card_fields) {
    merged.card_fields = {
      ...(device.card_fields ?? {}),
      ...patch.card_fields,
    } as CardFields;
  }
  return merged;
}

watch(
  () => props.activeId,
  () => {
    previewPatch.value = {};
  },
);

watch(
  () => presentDevices.value.map((device) => device.id),
  async () => {
    await nextTick();
    observeCards();
  },
);

watch(
  () => settings.value.photo_cols,
  async () => {
    await nextTick();
    resetVisibilityObserver();
  },
);

onUpdated(observeCards);

watch(
  () => props.editing,
  (editing) => {
    if (!editing) {
      pendingDeleteId.value = null;
      clearDeleteTimer();
      if (props.activeId) {
        previewPatch.value = {};
        hideInspectorWindow();
        emit('close-card');
      }
    }
  },
);

function openInspector(device: DeviceConfig): void {
  emit('open-card', device.id);
  void ipc.showCardInspector(device.id).catch((cause) => {
    console.error('show_card_inspector failed', cause);
  });
}

function hideInspectorWindow(): void {
  void ipc.hideCardInspector().catch(() => {
    /* 浏览器预览/单测没有命令通道 */
  });
}

function onOpen(device: DeviceConfig): void {
  if (!props.editing) return;
  openInspector(device);
}

function onSettings(device: DeviceConfig): void {
  openInspector(device);
}

/** 拖图到卡片：和设置窗共用同一落盘入口；设置窗通过事件同步新图。 */
async function onDrop(event: DragEvent, device: DeviceConfig): Promise<void> {
  event.preventDefault();
  const file = event.dataTransfer?.files?.[0] ?? null;
  if (!file || !isImageFile(file)) return;
  openInspector(device);
  try {
    await cardImages.save(device.id, await prepareCardImage(file));
    await config.updateDevice(device.id, { card_img: 'custom' });
  } catch (cause) {
    console.error('card image drop failed', cause);
  }
}

async function startScan(): Promise<void> {
  try {
    await ipc.showAdd();
  } catch (cause) {
    console.error('show_add failed', cause);
  }
}

/** 先收设置窗；返回 true 表示吃掉了这次 Esc / 点击。 */
function dismissTransient(): boolean {
  if (!props.activeId) return false;
  previewPatch.value = {};
  hideInspectorWindow();
  emit('close-card');
  return true;
}

defineExpose({ dismissTransient });

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape' || !props.activeId) return;
  event.preventDefault();
  event.stopPropagation();
  dismissTransient();
}

onMounted(async () => {
  resetVisibilityObserver();
  await nextTick();
  observeCards();
  window.addEventListener('keydown', onKeydown, true);
  try {
    unlisten.push(
      await listen<{ id: string; patch: Partial<DeviceConfig> }>('card-preview', (event) => {
        if (event.payload.id !== props.activeId) return;
        const patch = event.payload.patch ?? {};
        previewPatch.value =
          Object.keys(patch).length > 0
            ? { ...previewPatch.value, ...patch }
            : {};
      }),
    );
    unlisten.push(
      await listen('inspector-close', () => {
        previewPatch.value = {};
        if (props.activeId) emit('close-card');
      }),
    );
  } catch {
    /* 浏览器预览/单测没有事件通道，忽略 */
  }
});

onBeforeUnmount(() => {
  clearDeleteTimer();
  window.removeEventListener('keydown', onKeydown, true);
  visibilityObserver?.disconnect();
  visibilityObserver = null;
  for (const card of observedCards) {
    (card as HTMLElement).classList.remove('is-offscreen');
    delete (card as HTMLElement).dataset.offscreen;
  }
  observedCards.clear();
  for (const off of unlisten) off();
  unlisten = [];
  if (props.activeId) hideInspectorWindow();
});
</script>

<template>
  <section class="page">
    <div ref="scrollEl" class="dev-scroll fade-scroll">
      <div
        ref="gridEl"
        class="grid"
        :class="[`cols${settings.photo_cols}`, { 'is-editing': editing }]"
        :style="gridStyle"
        @pointerdown="drag.onPointerDown"
      >
        <DevicePhotoCard
          v-for="device in presentDevices"
          :key="device.id"
          :device="deviceFor(device)"
          :status="statusById.get(device.id) ?? null"
          :image="imageFor(device)"
          :good="settings.good_level"
          :warn="settings.warn_level"
          :band-colors="settings.band_colors"
          mode="devices"
          :editing="editing"
          :active="device.id === activeId"
          :pending-delete="pendingDeleteId === device.id"
          @open="onOpen(device)"
          @settings="onSettings(device)"
          @delete="onDelete(device)"
          @dragover.prevent
          @drop="onDrop($event, device)"
        />

        <button
          v-if="presentDevices.length > 0"
          type="button"
          class="tile-add"
          @click="startScan"
        >
          <LineIcon name="plus" :size="26" :stroke="1.8" />
          <span>添加设备</span>
        </button>

        <template v-else-if="hiddenCount === 0">
          <EmptyDeviceCard
            v-for="n in 2"
            :key="n"
            interactive
            @add="startScan"
          />
        </template>

        <div v-else class="empty glass-card">
          <p class="empty__title">没有已连接的设备</p>
          <p class="empty__hint">
            已配置的 {{ hiddenCount }} 台设备现在都没连上，接上后会自动出现
          </p>
          <button type="button" class="empty__btn" @click="startScan">
            <LineIcon name="plus" :size="16" :stroke="2" />
            <span>添加设备</span>
          </button>
        </div>
      </div>
    </div>

    </section>
</template>

<style scoped>
.page {
  position: relative;
  width: 816px;
  height: 632px;
}

/* 卡片区独立滚动：2 列一行铺满整页（604 高），3 列两行铺满整页（各 295 高） */
.dev-scroll {
  position: absolute;
  inset: 0 0 8px;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: stable;
}

.grid {
  display: grid;
  gap: 14px;
  align-content: start;
  padding-top: 20px;
  grid-auto-rows: var(--row-h);
}

.grid.cols2 {
  grid-template-columns: repeat(2, minmax(0, 1fr));
}

.grid.cols3 {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.grid.is-editing :deep(.pcard) {
  outline: 1px dashed color-mix(in srgb, var(--accent) 55%, transparent);
  outline-offset: -6px;
}

.grid.is-editing :deep(.pcard.is-active) {
  outline-color: var(--accent);
  box-shadow:
    0 0 0 2px color-mix(in srgb, var(--accent) 45%, transparent),
    var(--card-shadow-hover);
}

.grid :deep(.pcard.is-offscreen:not(.drag-lift)),
.grid :deep(.pcard.is-offscreen:not(.drag-lift) *) {
  animation-play-state: paused !important;
}

.tile-add {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  height: 100%;
  border: 1px dashed var(--card-line);
  border-radius: var(--r-panel);
  background: rgb(255 255 255 / 12%);
  color: var(--ink-3);
  font-family: var(--font-body);
  cursor: pointer;
  transition:
    background 0.2s ease,
    color 0.2s ease,
    border-color 0.2s ease;
}

.tile-add:hover {
  border-color: var(--card-line-hover);
  background: rgb(255 255 255 / 22%);
  color: var(--ink);
}

.tile-add span {
  font-size: 13px;
  font-weight: 600;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  grid-column: 1 / -1;
  height: 194px;
  padding: 48px 0 0 28px;
}

.empty__title {
  margin: 0;
  font-size: 16px;
  font-weight: 700;
  line-height: 1;
}

.empty__hint {
  margin: 14px 0 0;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
}

.empty__btn {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  height: 34px;
  margin-top: 12px;
  padding: 0 16px;
  border: 1px solid var(--cta-line);
  border-radius: var(--r-pill);
  background: var(--cta-bg);
  color: var(--cta-fg);
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 700;
  cursor: pointer;
}
</style>