<script setup lang="ts">
import { computed } from 'vue';
import DeviceIconGlyph from '../icons/DeviceIcon.vue';
import LineIcon from '../icons/LineIcon.vue';
import type { DeviceIconName } from '../icons/DeviceIcon.vue';
import { DEVICE_ICONS } from '../../types/device';
import type { DiscoveredDevice } from '../../types/discover';
import { deviceIdent } from '../../lib/deviceIdentity';

const { device, active = false, added = false, checked = false } = defineProps<{
  device: DiscoveredDevice;
  active?: boolean;
  /** 同一硬件已在配置里：仍可点开编辑，保存 = 更新该设备。 */
  added?: boolean;
  /** 多选状态：勾中的设备一起添加。 */
  checked?: boolean;
}>();

const emit = defineEmits<{ select: [] }>();

const iconName = computed<DeviceIconName>(() =>
  (DEVICE_ICONS as readonly string[]).includes(device.icon)
    ? (device.icon as DeviceIconName)
    : 'mouse',
);

/** 链接类型是中性信息；设备强调色只在下方色盘里出现，避免红色芯片被读成告警。 */
const chipStyle = computed(() => ({
  color: 'var(--ink-2)',
  backgroundColor: 'var(--card)',
}));

const stateText = computed(() => {
  if (added) return '已添加';
  if (!device.hit) return '未命中';
  if (device.level == null) return '可读取';
  return `${device.level}%`;
});
</script>

<template>
  <button
    type="button"
    class="scan-row"
    :class="{ 'scan-row--active': active, 'scan-row--added': added, 'scan-row--checked': checked }"
    :aria-pressed="checked"
    @click="emit('select')"
  >
    <span class="scan-row__check" :class="{ 'scan-row__check--on': checked }" aria-hidden="true">
      <LineIcon v-if="checked" name="check" :size="10.2" :stroke="2.21" />
    </span>
    <span class="scan-row__icon">
      <DeviceIconGlyph :name="iconName" :size="18.7" />
    </span>
    <span class="scan-row__main">
      <span class="scan-row__name">{{ device.label }}</span>
      <span class="scan-row__sub">{{ deviceIdent(device) }}</span>
    </span>
    <span class="scan-row__chip" :style="chipStyle">{{ device.link || 'HID' }}</span>
    <span
      class="scan-row__state"
      :class="{ 'scan-row__state--hit': device.hit && !added, 'scan-row__state--added': added }"
    >
      {{ stateText }}
    </span>
  </button>
</template>

<style scoped>
.scan-row {
  display: flex;
  flex: none;
  gap: 8.5px;
  align-items: center;
  width: 100%;
  height: 45.9px;
  padding: 0 11.9px 0 10.2px;
  border: 1px solid var(--card-line);
  border-radius: var(--r-tile);
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  color: var(--ink);
  text-align: left;
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

@media (hover: hover) {
  .scan-row:hover {
    border-color: var(--card-line-hover);
  }
}

.scan-row--active,
.scan-row:focus-visible {
  border-color: var(--accent);
  background: var(--accent-soft);
  outline: none;
}

.scan-row--checked {
  border-color: color-mix(in srgb, var(--accent) 55%, transparent);
}

.scan-row__check {
  display: grid;
  flex: none;
  place-items: center;
  width: 15.3px;
  height: 15.3px;
  border: 1.4px solid color-mix(in srgb, var(--ink-3) 55%, transparent);
  border-radius: 5.1px;
  color: var(--accent-fg);
  transition:
    border-color var(--speed) var(--ease),
    background-color var(--speed) var(--ease);
}

.scan-row__check--on {
  border-color: var(--accent);
  background: var(--accent);
}

.scan-row--added {
  opacity: 0.78;
}

.scan-row--added.scan-row--active,
.scan-row--added:focus-visible {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.scan-row__icon {
  display: grid;
  flex: none;
  place-items: center;
  width: 25.5px;
  height: 25.5px;
  color: var(--ink);
  opacity: 0.9;
}

.scan-row__main {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.scan-row__name {
  overflow: hidden;
  color: var(--ink);
  font-size: 12.75px;
  font-weight: 700;
  line-height: 17px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.scan-row__sub {
  overflow: hidden;
  color: var(--ink-2);
  font-size: 11.9px;
  font-weight: 400;
  line-height: 14.45px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.scan-row__chip {
  display: inline-flex;
  flex: none;
  align-items: center;
  height: 18.7px;
  padding: 0 6.8px;
  border: 1px solid color-mix(in srgb, var(--chip) 26%, transparent);
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--chip) 15%, var(--chip-bg-mix));
  color: color-mix(in srgb, var(--chip) var(--chip-text-weight), var(--chip-text-mix));
  font-size: 10.2px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

.scan-row__state {
  flex: none;
  min-width: 39.1px;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 11.9px;
  font-weight: 700;
  line-height: 1;
  text-align: right;
  white-space: nowrap;
}

.scan-row__state--hit {
  color: var(--green-text);
}

.scan-row__state--added {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 18.7px;
  min-width: 0;
  padding: 0 6.8px;
  border: 1px solid color-mix(in srgb, var(--green-text) 28%, transparent);
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--green-text) 10%, var(--chip-bg-mix));
  color: var(--green-text);
  font-family: var(--font-body);
  text-align: center;
}
</style>
