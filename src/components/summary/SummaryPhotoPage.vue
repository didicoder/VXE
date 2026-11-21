<script setup lang="ts">
import { computed, ref } from 'vue';
import { storeToRefs } from 'pinia';
import DevicePhotoCard from '../devices/DevicePhotoCard.vue';
import EmptyDeviceCard from '../devices/EmptyDeviceCard.vue';
import SummaryHero from './SummaryHero.vue';
import {
  filterDevicesByDisplay,
  filterStatusesByDisplay,
} from '../../lib/deviceCard';
import { percent } from '../../lib/format';
import { reorderSubset } from '../../lib/deviceOrder';
import { overviewFitLayout } from '../../lib/photoLayout';
import { stats as deviceStats } from '../../lib/state';
import * as ipc from '../../lib/ipc';
import { useCardImagesStore } from '../../stores/cardImages';
import { useConfigStore } from '../../stores/config';
import { useDevicesStore } from '../../stores/devices';
import { useDragReorder } from '../../composables/useDragReorder';
import type { DeviceConfig } from '../../types/device';

const emit = defineEmits<{ 'open-devices': [] }>();

const config = useConfigStore();
const deviceStore = useDevicesStore();
const cardImages = useCardImagesStore();
const { settings, devices } = storeToRefs(config);
const { statuses, lastRefresh } = storeToRefs(deviceStore);

const statusById = computed(
  () => new Map(statuses.value.map((status) => [status.id, status])),
);

/** 按「总览显示设备」过滤：显示已连接时，拔掉的接收器 / 关着的手柄不进卡片也不进总数。 */
const presentDevices = computed(() =>
  filterDevicesByDisplay(devices.value, statusById.value, settings.value.display_summary),
);
const presentStatuses = computed(() =>
  filterStatusesByDisplay(statuses.value, settings.value.display_summary),
);
const summary = computed(() => deviceStats(presentStatuses.value));

const gridEl = ref<HTMLElement | null>(null);

/** 总览不跟随设备页列数：所有卡片塞进一屏（不滚动），少放大、多缩小，整组居中。
 *  空状态用两张占位卡，因此按 2 张排。 */
const fit = computed(() => overviewFitLayout(presentDevices.value.length || 2));
const gridStyle = computed(() => ({
  width: `${fit.value.gridWidth.toFixed(2)}px`,
  height: `${fit.value.gridHeight.toFixed(2)}px`,
  '--card-w': `${fit.value.cardWidth.toFixed(2)}px`,
  '--card-h': `${fit.value.cardHeight.toFixed(2)}px`,
}));

/** 只显示用户自选图；没有自选图时卡片画电量光环。 */
function imageFor(device: DeviceConfig): string | null {
  return cardImages.imageFor(device.id);
}

const drag = useDragReorder({
  container: gridEl,
  handleSelector: '.pcard__grip',
  onCommit(from, to) {
    // 与设备页同款：拖拽下标是「可见项」的，按 id 把顺序写回完整列表。
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

function clockText(ms: number | null): string {
  if (ms == null) return '--:--:--';
  const date = new Date(ms);
  const pad = (value: number) => String(value).padStart(2, '0');
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

const lastText = computed(() => clockText(lastRefresh.value));
const lowText = computed(() => (summary.value.min == null ? '--' : percent(summary.value.min)));

async function startScan(): Promise<void> {
  try {
    await ipc.showAdd();
  } catch (cause) {
    console.error('show_add failed', cause);
  }
}
</script>

<template>
  <section class="page">
    <SummaryHero
      :total="summary.total"
      :online="summary.online"
      :avg="summary.avg"
      :min="summary.min"
      :good="settings.good_level"
      :warn="settings.warn_level"
      @add="startScan"
    />

    <p class="head">设备 · {{ summary.total }}</p>
    <p class="head__hint">全部卡片自动排布 · 去「设备」页调整卡片</p>

    <div class="gridwrap">
      <div
        ref="gridEl"
        class="grid fit-grid"
        :style="gridStyle"
        @pointerdown="drag.onPointerDown"
      >
          <DevicePhotoCard
            v-for="device in presentDevices"
            :key="device.id"
            :device="device"
            :status="statusById.get(device.id) ?? null"
            :image="imageFor(device)"
            :good="settings.good_level"
            :warn="settings.warn_level"
            :band-colors="settings.band_colors"
            mode="overview"
            @open="emit('open-devices')"
          />

          <template v-if="presentDevices.length === 0">
            <EmptyDeviceCard v-for="n in 2" :key="n" />
          </template>
      </div>
    </div>

    <div class="status">
      <span>上次刷新 {{ lastText }} · 自动刷新 {{ settings.refresh_seconds }} 秒</span>
      <span>最低电量 {{ lowText }} · 数据来源：USB HID · 仅在本机处理</span>
    </div>
  </section>
</template>

<style scoped>
.page {
  position: relative;
  width: 816px;
  height: 632px;
}

.head {
  position: absolute;
  top: 146px;
  left: 0;
  margin: 0;
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.01em;
  transform: translateY(-50%);
  color: var(--ink);
}

.head__hint {
  position: absolute;
  top: 148px;
  right: 0;
  margin: 0;
  color: var(--ink-3);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  transform: translateY(-50%);
}

.gridwrap {
  position: absolute;
  top: 168px;
  bottom: 40px;
  left: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 816px;
  overflow: hidden;
}

/* 总览自适应：固定卡片尺寸在区域中心换行，最后一行也居中，不出现滚动条 */
.grid {
  display: flex;
  flex-wrap: wrap;
  align-content: center;
  justify-content: center;
  gap: 14px;
}

.grid.fit-grid :deep(.pcard),
.grid.fit-grid :deep(.ecard) {
  flex: none;
  width: var(--card-w);
  height: var(--card-h);
  /* flex 项里的百分比 padding 按整组宽度算，会把内容压向卡片中间；
     改用卡片自身宽度做基准，保持和设备页卡片相同的相对内边距。 */
  padding: calc(var(--card-w) * 0.066) calc(var(--card-w) * 0.07)
    calc(var(--card-w) * 0.062);
}

.status {
  position: absolute;
  top: 610px;
  left: 0;
  display: flex;
  justify-content: space-between;
  width: 816px;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  transform: translateY(-50%);
}
</style>