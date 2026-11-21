<script setup lang="ts">
import { computed, ref } from 'vue';
import { storeToRefs } from 'pinia';
import DeviceCard from './DeviceCard.vue';
import SummaryHero from './SummaryHero.vue';
import PillButton from '../PillButton.vue';
import { percent } from '../../lib/format';
import { stats as deviceStats } from '../../lib/state';
import { filterStatusesByDisplay } from '../../lib/deviceCard';
import { reorderSubset } from '../../lib/deviceOrder';
import * as ipc from '../../lib/ipc';
import { useConfigStore } from '../../stores/config';
import { useDevicesStore } from '../../stores/devices';
import { useDragReorder } from '../../composables/useDragReorder';

const config = useConfigStore();
const deviceStore = useDevicesStore();
const { settings } = storeToRefs(config);
const { statuses, lastRefresh } = storeToRefs(deviceStore);

/** 按「总览显示设备」过滤：显示已连接时隐藏 absent（后端没读到这台硬件）。 */
const presentStatuses = computed(() =>
  filterStatusesByDisplay(statuses.value, settings.value.display_summary),
);

const summary = computed(() => deviceStats(presentStatuses.value));

/** 总览展示全部设备，栅格区独立滚动（3.1 修复：不再只显示前 4 台）。 */
const gridEl = ref<HTMLElement | null>(null);
const drag = useDragReorder({
  container: gridEl,
  handleSelector: '.card__grip',
  onCommit(from, to) {
    // 栅格只渲染在线设备，按 id 把可见项的新顺序写回完整列表。
    const visible = presentStatuses.value;
    const movingId = visible[from]?.id;
    if (!movingId || visible[to] === undefined) return;
    const order = visible.map((status) => status.id);
    order.splice(from, 1);
    order.splice(to, 0, movingId);
    deviceStore.statuses = reorderSubset(deviceStore.statuses, order);
    config.devices = reorderSubset(config.devices, order);
    void config.reorderDevices(config.devices.map((device) => device.id));
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

    <!-- 区块头：y=312 / 314 -->
    <p class="head">设备 · {{ summary.total }}</p>
    <p class="head__hint">左键托盘图标看小窗，右键打开面板</p>

    <!-- 设备栅格：完整列表放在独立滚动区，卡 401×132 自适应列宽、gap 14 -->
    <div class="gridwrap fade-scroll">
      <div ref="gridEl" class="grid" @pointerdown="drag.onPointerDown">
        <DeviceCard
          v-for="device in presentStatuses"
          :key="device.id"
          :status="device"
          :good="settings.good_level"
          :warn="settings.warn_level"
          :band-colors="settings.band_colors"
        />

        <div v-if="statuses.length === 0" class="empty glass-card">
          <p class="empty__title">还没有添加设备</p>
          <p class="empty__hint">点右上角「添加设备」，从本机 HID 设备里挑一只鼠标或键盘</p>
          <PillButton class="empty__btn" variant="primary" size="sm" icon="search" @click="startScan">
            开始扫描
          </PillButton>
        </div>
      </div>
    </div>

    <!-- 底部状态行：左刷新信息、右最低电量 -->
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

/* 滚动区：从区块头下方到状态行上方；滚动条槽常驻，拇指只在滑动时出现 */
.gridwrap {
  position: absolute;
  top: 168px;
  bottom: 40px;
  left: 0;
  width: 816px;
  overflow-x: hidden;
  overflow-y: auto;
  scrollbar-gutter: stable;
}

.grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
  align-content: start;
}

/* 空态卡：表面走 .glass-card */
.empty {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  grid-column: 1 / -1;
  height: 132px;
  padding: 36px 0 0 28px;
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
  margin-top: 8px;
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