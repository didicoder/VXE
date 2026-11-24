<script setup lang="ts">
import type { Component } from 'vue';
import MenuToggleIcon from './icons/MenuToggleIcon.vue';

interface SegmentedOption {
  value: string;
  label: string;
  /** 带收纳图标的选项：图标与文字同在一个胶囊内，点图标只发 toggle-expand。 */
  expandable?: boolean;
  /** 可选的前置预览组件（托盘样式等），不影响纯文本选项。 */
  preview?: Component;
  previewProps?: Record<string, unknown>;
}

const {
  modelValue,
  options,
  expanded = null,
} = defineProps<{
  modelValue: string;
  options: SegmentedOption[];
  /** 当前处于展开态的选项 value（只影响收纳图标的动画状态）。 */
  expanded?: string | null;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: string];
  'toggle-expand': [value: string];
  /** 每次点选项本体都会发出，包括点击当前已选项（不会被 update 吞掉）。 */
  select: [value: string];
}>();

function select(value: string) {
  emit('select', value);
  if (value === modelValue) return;
  emit('update:modelValue', value);
}
</script>

<template>
  <div class="segmented" role="radiogroup">
    <div
      v-for="option in options"
      :key="option.value"
      class="segmented__item"
      :class="{ 'segmented__item--active': option.value === modelValue }"
      role="radio"
      :aria-checked="option.value === modelValue"
      tabindex="0"
      @click="select(option.value)"
      @keydown.enter.prevent="select(option.value)"
      @keydown.space.prevent="select(option.value)"
    >
      <button
        v-if="option.expandable"
        type="button"
        class="segmented__expander"
        :aria-expanded="expanded === option.value"
        aria-label="展开选项"
        @click.stop="emit('toggle-expand', option.value)"
      >
        <MenuToggleIcon :size="14" :active="expanded === option.value" />
      </button>
      <component
        :is="option.preview"
        v-if="option.preview"
        v-bind="option.previewProps"
        class="segmented__preview"
      />
      <span>{{ option.label }}</span>
    </div>
  </div>
</template>

<style scoped>
.segmented {
  display: flex;
  align-items: stretch;
  flex: none;
  width: 300px;
  height: 40px;
  padding: 3px;
  border: 1px solid var(--seg-line);
  border-radius: var(--seg-radius);
  background: var(--seg-bg);
  box-shadow: var(--seg-shadow);
}

.segmented__item {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-width: 0;
  padding: 0;
  border: 1px solid transparent;
  border-radius: calc(var(--seg-radius) - 3px);
  background: transparent;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    color var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.segmented__item:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}

.segmented__expander {
  display: grid;
  flex: none;
  place-items: center;
  width: 16px;
  height: 16px;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  cursor: pointer;
}

.segmented__expander:hover {
  color: var(--ink);
}

.segmented__preview {
  display: block;
  flex: none;
  width: 24px;
  height: 24px;
}

.segmented__item--active {
  border-color: var(--seg-on-line);
  background: var(--seg-on-bg);
  box-shadow: var(--seg-on-shadow);
  color: var(--ink);
  font-weight: 700;
}

@media (hover: hover) {
  .segmented__item:not(.segmented__item--active):hover {
    color: var(--ink);
  }
}
</style>