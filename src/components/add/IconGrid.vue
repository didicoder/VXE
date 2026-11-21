<script setup lang="ts">
import DeviceIconGlyph from '../icons/DeviceIcon.vue';
import { DEVICE_ICONS, type DeviceIcon } from '../../types/device';

const { modelValue } = defineProps<{ modelValue: DeviceIcon }>();

const emit = defineEmits<{
  'update:modelValue': [value: DeviceIcon];
}>();

const LABELS: Record<DeviceIcon, string> = {
  mouse: '鼠标',
  keyboard: '键盘',
  headset: '头戴耳机',
  earbuds: '入耳耳机',
  gamepad: '手柄',
  phone: '手机',
};
</script>

<template>
  <div class="icon-grid" role="group" aria-label="图标">
    <button
      v-for="key in DEVICE_ICONS"
      :key="key"
      type="button"
      class="icon-cell"
      :class="{ 'icon-cell--active': modelValue === key }"
      :aria-label="LABELS[key]"
      :aria-pressed="modelValue === key"
      :title="LABELS[key]"
      @click="emit('update:modelValue', key)"
    >
      <DeviceIconGlyph :name="key" :size="18.7" />
    </button>
  </div>
</template>

<style scoped>
.icon-grid {
  display: grid;
  grid-template-columns: repeat(6, 34px);
  grid-auto-rows: 34px;
  gap: 5.1px;
  width: 229.5px;
  height: 28.9px;
}

.icon-cell {
  display: grid;
  place-items: center;
  width: 28.9px;
  height: 28.9px;
  padding: 0;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  opacity: 0.9;
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

@media (hover: hover) {
  .icon-cell:hover {
    border-color: var(--ctrl-line-hover);
  }
}

.icon-cell--active,
.icon-cell:focus-visible {
  border-color: var(--accent);
  background: var(--accent-soft);
  opacity: 1;
  outline: none;
}
</style>
