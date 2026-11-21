<script setup lang="ts">
import DeviceIcon from '../icons/DeviceIcon.vue';
import { DEVICE_ICONS, type DeviceIcon as DeviceIconName } from '../../types/device';

const { current } = defineProps<{
  current: DeviceIconName;
}>();

const emit = defineEmits<{
  select: [icon: DeviceIconName];
}>();

const ICON_NAMES: Record<DeviceIconName, string> = {
  mouse: '鼠标',
  keyboard: '键盘',
  headset: '耳机',
  earbuds: '入耳式耳机',
  gamepad: '手柄',
  phone: '手机',
};
</script>

<template>
  <div class="icon-picker">
    <button
      v-for="icon in DEVICE_ICONS"
      :key="icon"
      type="button"
      class="icon-picker__cell"
      :class="{ 'icon-picker__cell--on': icon === current }"
      :aria-label="ICON_NAMES[icon]"
      @click.stop="emit('select', icon)"
    >
      <DeviceIcon :name="icon" :size="28" />
    </button>
  </div>
</template>

<style scoped>
/* 3×2 面板：格 52、间距 8、内边距 10 → 192×132（V16 `_icon_picker`） */
.icon-picker {
  display: grid;
  grid-template-columns: repeat(3, 52px);
  gap: 8px;
  width: 192px;
  height: 132px;
  padding: 10px;
  border: 0;
  border-radius: var(--r-pop);
  background: var(--card-hi);
  box-shadow:
    0 0 0 1px var(--surface-stroke),
    var(--shadow-float),
    var(--surface-highlight);
}

.icon-picker__cell {
  display: grid;
  place-items: center;
  width: 52px;
  height: 52px;
  padding: 0;
  border: 1px solid var(--surface-stroke);
  border-radius: var(--r-ctrl);
  background: var(--card);
  color: var(--txt);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    border-color var(--dur-fast) var(--ease-out);
}

.icon-picker__cell:hover {
  border-color: var(--surface-stroke-strong);
  background: var(--card-btn);
}

.icon-picker__cell--on {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 12%, var(--card-hi));
}
</style>
