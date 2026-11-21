<script setup lang="ts">
import { COLOR_HEX } from '../../lib/colors';
import { COLOR_KEYS, type ColorKey } from '../../types/battery';

const { modelValue } = defineProps<{ modelValue: ColorKey }>();

const emit = defineEmits<{
  'update:modelValue': [value: ColorKey];
}>();

const LABELS: Record<ColorKey, string> = {
  blue: '蓝色',
  red: '红色',
  green: '绿色',
  purple: '紫色',
  amber: '琥珀色',
  cyan: '青色',
};
</script>

<template>
  <div class="swatches" role="group" aria-label="强调色">
    <button
      v-for="key in COLOR_KEYS"
      :key="key"
      type="button"
      class="swatch"
      :class="{ 'swatch--active': modelValue === key }"
      :aria-label="LABELS[key]"
      :aria-pressed="modelValue === key"
      :title="LABELS[key]"
      :style="{ '--dot-color': COLOR_HEX[key] }"
      @click="emit('update:modelValue', key)"
    >
      <span class="swatch__dot" />
    </button>
  </div>
</template>

<style scoped>
.swatches {
  display: inline-flex;
  gap: 5.1px;
  align-items: center;
  height: 28.9px;
}

/* 与 IconGrid 同一套 34px 单元：色点居中，行宽与图标格完全对齐 */
.swatch {
  position: relative;
  display: grid;
  place-items: center;
  width: 28.9px;
  height: 28.9px;
  padding: 0;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease);
}

.swatch--active {
  border-color: var(--accent);
  background: var(--accent-soft);
}

.swatch__dot {
  width: 11.05px;
  height: 11.05px;
  border-radius: 50%;
  background: var(--dot-color);
  transition:
    width var(--speed) var(--ease),
    height var(--speed) var(--ease),
    filter var(--speed) var(--ease);
}

.swatch--active .swatch__dot {
  width: 13.6px;
  height: 13.6px;
}

@media (hover: hover) {
  .swatch:not(.swatch--active):hover {
    border-color: var(--card-line-hover);
  }

  .swatch:not(.swatch--active):hover .swatch__dot {
    filter: brightness(1.12);
  }
}

.swatch:focus-visible {
  outline: 1.4px solid var(--accent);
  outline-offset: -1.19px;
}
</style>
