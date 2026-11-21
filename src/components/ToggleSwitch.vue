<script setup lang="ts">
const {
  modelValue,
  disabled = false,
} = defineProps<{
  modelValue: boolean;
  disabled?: boolean;
}>();

const emit = defineEmits<{
  'update:modelValue': [value: boolean];
}>();

function toggle() {
  if (disabled) return;
  emit('update:modelValue', !modelValue);
}
</script>

<template>
  <button
    type="button"
    class="toggle"
    :class="{ 'toggle--on': modelValue }"
    role="switch"
    :aria-checked="modelValue"
    :disabled="disabled"
    @click="toggle"
  >
    <span class="toggle__knob" />
  </button>
</template>

<style scoped>
.toggle {
  position: relative;
  flex: none;
  width: 40px;
  height: 22px;
  padding: 0;
  border: 1px solid var(--toggle-line);
  border-radius: var(--r-pill);
  background: var(--toggle-track);
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

.toggle--on {
  border-color: var(--accent);
  background: var(--accent);
}

.toggle:disabled {
  cursor: default;
  opacity: 0.45;
}

.toggle__knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: var(--toggle-knob);
  box-shadow:
    0 1px 2px rgba(16, 24, 40, 28%),
    inset 0 1px 0 rgba(255, 255, 255, 85%);
  transition:
    transform var(--speed) var(--ease),
    background-color var(--speed) var(--ease);
}

.toggle--on .toggle__knob {
  background: var(--toggle-knob);
  transform: translateX(18px);
}

@media (hover: hover) {
  .toggle:not(:disabled):hover {
    filter: brightness(1.06);
  }
}
</style>
