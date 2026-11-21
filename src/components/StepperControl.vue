<script setup lang="ts">
import { computed } from 'vue';
import LineIcon from './icons/LineIcon.vue';

const {
  modelValue,
  min,
  max,
  step = 1,
  format,
} = defineProps<{
  modelValue: number;
  min?: number;
  max?: number;
  step?: number;
  format?: (value: number) => string;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: number];
}>();

const displayValue = computed(() =>
  format ? format(modelValue) : String(modelValue),
);

const canDecrease = computed(
  () => min == null || modelValue > min,
);
const canIncrease = computed(
  () => max == null || modelValue < max,
);

function stepPrecision(value: number) {
  const text = String(value);
  if (text.includes('e-')) return 6;
  const dot = text.indexOf('.');
  return dot === -1 ? 0 : text.length - dot - 1;
}

function clampValue(value: number) {
  let next = value;
  if (min != null) next = Math.max(min, next);
  if (max != null) next = Math.min(max, next);
  return next;
}

function commit(direction: 1 | -1) {
  const delta = (step || 1) * direction;
  const precision = Math.min(6, stepPrecision(step || 1));
  const factor = 10 ** precision;
  const next = clampValue(
    Math.round((modelValue + delta) * factor) / factor,
  );
  if (next !== modelValue) emit('update:modelValue', next);
}

function onWheel(event: WheelEvent) {
  if (event.deltaY === 0) return;
  commit(event.deltaY < 0 ? 1 : -1);
}
</script>

<template>
  <div class="stepper" @wheel.prevent="onWheel">
    <button
      type="button"
      class="stepper__button"
      :disabled="!canDecrease"
      aria-label="减少"
      @click="commit(-1)"
    >
      <LineIcon name="minus" :size="13" :stroke="2.1" />
    </button>
    <span class="stepper__value">{{ displayValue }}</span>
    <button
      type="button"
      class="stepper__button"
      :disabled="!canIncrease"
      aria-label="增加"
      @click="commit(1)"
    >
      <LineIcon name="plus" :size="13" :stroke="2.1" />
    </button>
  </div>
</template>

<style scoped>
.stepper {
  display: flex;
  align-items: center;
  flex: none;
  width: 150px;
  height: 44px;
  padding: 0 8px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
}

.stepper__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex: none;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--ink);
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

@media (hover: hover) {
  .stepper__button:not(:disabled):hover {
    background: var(--stepper-hover);
  }
}

.stepper__button:disabled {
  cursor: default;
  opacity: 0.35;
}

.stepper__value {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  /* 1 会裁掉 y/p/_ 等下伸部（overflow:hidden + 省略号），留出完整字形高度。 */
  line-height: 1.35;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
