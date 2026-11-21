<script setup lang="ts">
import { computed } from 'vue';
import LineIcon from './icons/LineIcon.vue';
import type { LineIconName } from './icons/LineIcon.vue';

type PillSize = 'sm' | 'md' | 'lg';
type PillVariant = 'primary' | 'dark' | 'soft' | 'ghost' | 'danger';

const {
  variant = 'soft',
  disabled = false,
  icon,
  size = 'md',
} = defineProps<{
  variant?: PillVariant;
  disabled?: boolean;
  icon?: LineIconName;
  size?: PillSize;
}>();

const emit = defineEmits<{
  click: [event: MouseEvent];
}>();

const iconSize = computed(() => {
  if (size === 'sm') return 13;
  if (size === 'lg') return 18;
  return 16;
});

function onClick(event: MouseEvent) {
  if (disabled) return;
  emit('click', event);
}
</script>

<template>
  <button
    type="button"
    class="pill"
    :class="[`pill--${variant}`, `pill--${size}`]"
    :disabled="disabled"
    @click="onClick"
  >
    <LineIcon v-if="icon" :name="icon" :size="iconSize" :stroke="1.7" />
    <span v-if="$slots.default" class="pill__label"><slot /></span>
  </button>
</template>

<style scoped>
.pill {
  --pill-h: 40px;

  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: var(--pill-h);
  padding: 0 18px;
  border: 1px solid transparent;
  border-radius: var(--r-pill);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    color var(--speed) var(--ease),
    transform var(--speed) var(--ease),
    filter var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

.pill--sm {
  --pill-h: 34px;

  padding: 0 16px;
  font-size: 13px;
  min-width: 108px;
}

.pill--lg {
  --pill-h: 44px;

  padding: 0 22px;
  font-size: 18px;
}

.pill--primary {
  background: var(--accent);
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 16%);
  color: var(--accent-fg);
}

.pill--dark {
  border-color: var(--ctrl-line);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
}

.pill--soft {
  border-color: var(--ctrl-line);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
}

.pill--ghost {
  background: transparent;
  color: var(--ink);
}

.pill--danger {
  border-color: color-mix(in srgb, var(--low) 24%, transparent);
  background: color-mix(in srgb, var(--low) 14%, var(--card));
  color: var(--ink-low);
}

@media (hover: hover) {
  .pill:not(:disabled):hover {
    transform: translateY(-1px);
    filter: brightness(1.06);
  }

  .pill--soft:not(:disabled):hover,
  .pill--dark:not(:disabled):hover {
    background: var(--ctrl-bg-hover);
    border-color: var(--ctrl-line-hover);
    box-shadow: var(--ctrl-shadow-hover);
    filter: none;
  }

  .pill--ghost:not(:disabled):hover {
    background: var(--card-hi);
  }

  .pill--danger:not(:disabled):hover {
    background: color-mix(in srgb, var(--low) 22%, var(--card));
  }
}

.pill:disabled {
  cursor: default;
  opacity: 0.45;
}

/* 主按钮禁用时退回中性底，避免仍像可点的深蓝 */
.pill--primary:disabled {
  border-color: var(--ctrl-line);
  background: var(--ctrl-bg);
  box-shadow: none;
  color: var(--muted);
  opacity: 1;
}

.pill__label {
  display: block;
}
</style>
