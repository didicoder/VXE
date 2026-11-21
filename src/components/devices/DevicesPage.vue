<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
import { storeToRefs } from 'pinia';
import DeviceRow from './DeviceRow.vue';
import { filterDevicesByDisplay } from '../../lib/deviceCard';
import { reorderSubset } from '../../lib/deviceOrder';
import { useConfigStore } from '../../stores/config';
import { useDevicesStore } from '../../stores/devices';
import { useDragReorder } from '../../composables/useDragReorder';
import type { BatteryBand, ColorKey, DeviceColors } from '../../types/battery';
import type { DeviceIcon } from '../../types/device';

/** 配色块里正在改的档位；`accent` = 设备强调色（链接芯片）。 */
type ColorPickKind = BatteryBand | 'accent';

const PAGE_ROWS = 5;
const DEL_CONFIRM_MS = 4000;
const CHIP_FONT = '700 18px "Microsoft YaHei", "Segoe UI", sans-serif';
const CHIP_PAD = 10;

const configStore = useConfigStore();
const deviceStore = useDevicesStore();
const { devices, settings } = storeToRefs(configStore);
const { statuses } = storeToRefs(deviceStore);

const offset = ref(0);
const editId = ref<string | null>(null);
const colorPick = ref<{ id: string; kind: ColorPickKind } | null>(null);
const iconPickId = ref<string | null>(null);
const pendingDelId = ref<string | null>(null);
let delTimer: ReturnType<typeof setTimeout> | null = null;

const statusById = computed(
  () => new Map(statuses.value.map((status) => [status.id, status])),
);
/** 按「设备页显示设备」过滤：显示已连接时隐藏 absent（未连接）设备。 */
const visibleDevices = computed(() =>
  filterDevicesByDisplay(devices.value, statusById.value, settings.value.display_devices),
);
const rows = computed(() => visibleDevices.value.slice(offset.value, offset.value + PAGE_ROWS));
const maxOffset = computed(() => Math.max(0, visibleDevices.value.length - PAGE_ROWS));

/** 行拖拽排序：自研指针拖拽，1:1 跟手（缩放容器下第三方库坐标会失真）。 */
const rowsEl = ref<HTMLElement | null>(null);
const drag = useDragReorder({
  container: rowsEl,
  handleSelector: '.row__grip',
  onCommit(from, to) {
    // 拖拽下标是「可见项」的，有设备被过滤时不能直接拿去动完整列表。
    const visible = visibleDevices.value;
    const movingId = visible[from]?.id;
    if (!movingId || visible[to] === undefined) return;
    const order = visible.map((device) => device.id);
    order.splice(from, 1);
    order.splice(to, 0, movingId);
    devices.value = reorderSubset(devices.value, order);
    deviceStore.statuses = reorderSubset(deviceStore.statuses, order);
    void configStore.reorderDevices(devices.value.map((device) => device.id));
  },
});

/** 链接芯片列宽：所有设备取最大值，状态文案才会落在同一条竖线上（V16 同款）。 */
let measureCtx: CanvasRenderingContext2D | null | undefined;

function textWidth(text: string): number {
  if (measureCtx === undefined) {
    measureCtx =
      typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  }
  if (!measureCtx) return text.length * 11;
  measureCtx.font = CHIP_FONT;
  return measureCtx.measureText(text).width;
}

const chipWidth = computed(() => {
  const widths = devices.value.map((device) => textWidth(device.link || '2.4G'));
  return (widths.length > 0 ? Math.max(...widths) : 0) + CHIP_PAD * 2;
});

watch(maxOffset, (max) => {
  if (offset.value > max) offset.value = max;
});

function clearDelTimer(): void {
  if (delTimer !== null) {
    clearTimeout(delTimer);
    delTimer = null;
  }
}

/** 点到别处 / Esc：先收临时态（V16 `mainwin.on_click` 头部）。 */
function dismissTransient(): boolean {
  if (iconPickId.value !== null) {
    iconPickId.value = null;
    return true;
  }
  if (colorPick.value !== null) {
    colorPick.value = null;
    return true;
  }
  if (pendingDelId.value !== null) {
    clearDelTimer();
    pendingDelId.value = null;
    return true;
  }
  if (editId.value !== null) {
    editId.value = null;
    return true;
  }
  return false;
}

defineExpose({ dismissTransient });

function onWheel(event: WheelEvent): void {
  if (visibleDevices.value.length <= PAGE_ROWS) return;
  const step = event.deltaY > 0 ? 1 : -1;
  offset.value = Math.min(maxOffset.value, Math.max(0, offset.value + step));
}

function openIcon(id: string): void {
  dismissTransient();
  iconPickId.value = id;
}

function openColor(id: string, kind: ColorPickKind): void {
  dismissTransient();
  colorPick.value = { id, kind };
}

async function pickColor(id: string, color: ColorKey): Promise<void> {
  const kind = colorPick.value?.id === id ? colorPick.value.kind : null;
  colorPick.value = null;
  if (kind === null) return;
  if (kind === 'accent') {
    await configStore.updateDevice(id, { color });
    return;
  }
  const device = devices.value.find((item) => item.id === id);
  const colors: DeviceColors = { ...(device?.colors ?? {}), [kind]: color };
  await configStore.updateDevice(id, { colors });
}

async function pickIcon(id: string, icon: DeviceIcon): Promise<void> {
  iconPickId.value = null;
  await configStore.updateDevice(id, { icon });
}

async function rename(id: string, text: string): Promise<void> {
  editId.value = null;
  const device = devices.value.find((item) => item.id === id);
  const label = text.trim();
  if (!device || !label || label === device.label) return;
  await configStore.updateDevice(id, { label });
}

/** 改名也是临时态：先收掉别的（确认删除/选择器），再进编辑。 */
function beginEdit(id: string): void {
  dismissTransient();
  editId.value = id;
}

async function remove(id: string): Promise<void> {
  if (pendingDelId.value !== id) {
    clearDelTimer();
    pendingDelId.value = id;
    delTimer = setTimeout(() => {
      pendingDelId.value = null;
      delTimer = null;
    }, DEL_CONFIRM_MS);
    return;
  }
  clearDelTimer();
  pendingDelId.value = null;
  const removed = await configStore.removeDevice(id);
  if (removed) void deviceStore.refreshNow();
}

onUnmounted(clearDelTimer);
</script>

<template>
  <section class="page">
    <div v-if="rows.length > 0" ref="rowsEl" class="rows" @pointerdown="drag.onPointerDown" @click="dismissTransient" @wheel.prevent="onWheel">
      <DeviceRow
        v-for="(device, index) in rows"
        :key="device.id"
        :device="device"
        :status="statusById.get(device.id) ?? null"
        :good="settings.good_level"
        :warn="settings.warn_level"
        :band-colors="settings.band_colors"
        :chip-width="chipWidth"
        :editing="editId === device.id"
        :color-pick="colorPick !== null && colorPick.id === device.id ? colorPick.kind : null"
        :icon-picking="iconPickId === device.id"
        :picker-up="index === PAGE_ROWS - 1"
        :pending-del="pendingDelId === device.id"
        @edit="beginEdit(device.id)"
        @rename="(text) => rename(device.id, text)"
        @cancel-rename="editId = null"
        @open-color="(kind) => openColor(device.id, kind)"
        @close-color="colorPick = null"
        @pick-color="(color) => pickColor(device.id, color)"
        @open-icon="openIcon(device.id)"
        @pick-icon="(icon) => pickIcon(device.id, icon)"
        @remove="remove(device.id)"
      />
    </div>

    <div v-else class="empty glass-card">
      <p class="empty__title">还没有设备</p>
      <p class="empty__hint">点右上角「添加设备」，从本机 HID 设备里挑选</p>
    </div>

    <p v-if="visibleDevices.length > PAGE_ROWS" class="hint">
      滚轮翻页 · {{ offset + 1 }}-{{ Math.min(visibleDevices.length, offset + PAGE_ROWS) }} /
      {{ visibleDevices.length }}
    </p>
  </section>
</template>

<style scoped>
.page {
  position: relative;
  width: 816px;
  height: 632px;
}

/* 行卡 top=108（页面内 20）、高 100、间距 12，五行走一屏 */
.rows {
  position: absolute;
  top: 20px;
  left: 0;
  display: flex;
  flex-direction: column;
  gap: 12px;
  width: 816px;
}

/* 空态：表面走 .glass-card */
.empty {
  position: absolute;
  top: 20px;
  left: 0;
  width: 816px;
  height: 100px;
}

.empty__title {
  position: absolute;
  top: 32px;
  left: 26px;
  margin: 0;
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
  transform: translateY(-50%);
}

.empty__hint {
  position: absolute;
  top: 54px;
  left: 26px;
  margin: 0;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  transform: translateY(-50%);
}

.hint {
  position: absolute;
  top: 576px;
  right: 0;
  margin: 0;
  color: var(--muted);
  font-size: 12px;
  font-weight: 400;
  line-height: 1;
  transform: translateY(-50%);
}
</style>
