<script setup lang="ts">
/**
 * 全局快捷键行（E1）：启用开关 + 录制/输入框 + 应用。
 *
 * 录制时在 window 捕获阶段监听 keydown，把修饰键与主键拼成 Tauri 加速器串
 * （`ctrl+alt+d`）。冲突/格式错误由父级在 `error` 里回传，直接替换辅助文字行内展示。
 */

import { onBeforeUnmount, ref } from 'vue';
import SettingRow from './SettingRow.vue';
import PillButton from '../PillButton.vue';
import ToggleSwitch from '../ToggleSwitch.vue';

const {
  enabled,
  modelValue,
  error = null,
  applying = false,
} = defineProps<{
  enabled: boolean;
  modelValue: string;
  error?: string | null;
  applying?: boolean;
}>();

const emit = defineEmits<{
  'update:enabled': [value: boolean];
  'update:modelValue': [value: string];
  apply: [];
}>();

const recording = ref(false);

const MODIFIERS = new Set(['Control', 'Alt', 'Shift', 'Meta']);

/** 特殊键 → 加速器写法；单字符键直接小写。 */
const NAMED_KEYS: Record<string, string> = {
  ' ': 'space',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  Enter: 'enter',
  Tab: 'tab',
  Home: 'home',
  End: 'end',
  PageUp: 'pageup',
  PageDown: 'pagedown',
  Insert: 'insert',
  Delete: 'delete',
};

function acceleratorKey(event: KeyboardEvent): string {
  if (/^F([1-9]|1[0-2])$/.test(event.key)) return event.key.toLowerCase();
  const named = NAMED_KEYS[event.key];
  if (named) return named;
  return event.key.length === 1 ? event.key.toLowerCase() : '';
}

function onRecordKey(event: KeyboardEvent): void {
  if (!recording.value) return;
  event.preventDefault();
  event.stopPropagation();
  if (event.key === 'Escape') {
    stopRecording();
    return;
  }
  if (MODIFIERS.has(event.key)) return;
  const main = acceleratorKey(event);
  if (!main) return;
  const parts: string[] = [];
  if (event.ctrlKey) parts.push('ctrl');
  if (event.altKey) parts.push('alt');
  if (event.shiftKey) parts.push('shift');
  if (event.metaKey) parts.push('super');
  parts.push(main);
  emit('update:modelValue', parts.join('+'));
  stopRecording();
}

function startRecording(): void {
  if (recording.value || !enabled) return;
  recording.value = true;
  window.addEventListener('keydown', onRecordKey, true);
}

function stopRecording(): void {
  if (!recording.value) return;
  recording.value = false;
  window.removeEventListener('keydown', onRecordKey, true);
}

function onInput(event: Event): void {
  emit('update:modelValue', (event.target as HTMLInputElement).value);
}

onBeforeUnmount(stopRecording);
</script>

<template>
  <SettingRow
    label="全局快捷键"
    :hint="error ?? '启用后按组合键直接唤起主窗'"
    :hint-tone="error ? 'low' : 'muted'"
  >
    <div class="hotkey">
      <ToggleSwitch
        :model-value="enabled"
        @update:model-value="emit('update:enabled', $event)"
      />
      <input
        class="hotkey__input"
        :class="{ 'hotkey__input--recording': recording }"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :value="modelValue"
        :disabled="!enabled"
        :placeholder="recording ? '按下组合键…' : 'ctrl+alt+d'"
        aria-label="快捷键组合"
        @input="onInput"
      >
      <button
        type="button"
        class="hotkey__record"
        :class="{ 'hotkey__record--on': recording }"
        :disabled="!enabled"
        @click="recording ? stopRecording() : startRecording()"
      >
        {{ recording ? '停止' : '录制' }}
      </button>
      <PillButton
        size="sm"
        :disabled="!enabled || applying"
        @click="emit('apply')"
      >
        应用
      </PillButton>
    </div>
  </SettingRow>
</template>

<style scoped>
.hotkey {
  display: flex;
  align-items: center;
  gap: 10px;
}

.hotkey__input {
  width: 190px;
  height: 34px;
  padding: 0 12px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 600;
  line-height: 1;
  outline: none;
  transition:
    border-color var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.hotkey__input::placeholder {
  color: var(--ink-4);
}

.hotkey__input:focus {
  border-color: var(--focus);
  box-shadow: var(--focus-shadow);
}

.hotkey__input--recording {
  border-color: var(--accent);
  box-shadow: var(--focus-shadow);
  color: var(--accent);
}

.hotkey__input:disabled {
  cursor: default;
  opacity: 0.45;
}

/* 与 PillButton sm 同规格（108×34 胶囊），保持组内按钮一致 */
.hotkey__record {
  min-width: 108px;
  height: 34px;
  padding: 0 16px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    transform var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

.hotkey__record--on {
  border-color: var(--accent);
  background: var(--accent-soft);
  color: var(--accent);
}

@media (hover: hover) {
  .hotkey__record:not(:disabled):hover {
    background: var(--ctrl-bg-hover);
    transform: translateY(-1px);
  }
}

.hotkey__record:disabled {
  cursor: default;
  opacity: 0.45;
}
</style>
