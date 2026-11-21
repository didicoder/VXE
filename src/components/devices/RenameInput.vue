<script setup lang="ts">
import { onMounted, ref } from 'vue';

const { value, width = 220 } = defineProps<{
  value: string;
  width?: number;
}>();

const emit = defineEmits<{
  commit: [text: string];
  cancel: [];
}>();

const input = ref<HTMLInputElement | null>(null);
/** 本地草稿：只取初始值，编辑期间不跟随外部刷新（否则轮询重渲染会把输入弹回）。 */
const draft = ref(value);

/** Enter / Esc / 失焦三条路只走第一条（失焦提交与 Esc 取消不会互相打架）。 */
let settled = false;

onMounted(() => {
  // 双保险聚焦（部分环境 onMounted 时尚未可聚焦）
  const focus = () => {
    const el = input.value;
    if (!el) return;
    el.focus();
    el.select();
  };
  focus();
  requestAnimationFrame(focus);
});

function commit(): void {
  if (settled) return;
  settled = true;
  emit('commit', draft.value ?? '');
}

function cancel(): void {
  if (settled) return;
  settled = true;
  emit('cancel');
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Enter') {
    event.preventDefault();
    event.stopPropagation();
    commit();
    return;
  }
  if (event.key === 'Escape') {
    // Esc 只收起输入框，不冒泡到窗口（V16 `_open_entry` 返回 "break"）
    event.preventDefault();
    event.stopPropagation();
    cancel();
  }
}
</script>

<template>
  <input
    ref="input"
    v-model="draft"
    class="rename"
    type="text"
    :style="{ width: `${width}px` }"
    spellcheck="false"
    autocomplete="off"
    autofocus
    @click.stop
    @mousedown.stop
    @keydown="onKeydown"
    @blur="commit"
  />
</template>

<style scoped>
.rename {
  height: 42px;
  padding: 0 7px;
  border: 1.5px solid var(--accent);
  border-radius: var(--r-ctrl);
  outline: none;
  background: var(--card-btn);
  box-shadow: var(--focus-ring);
  color: var(--txt);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  user-select: text;
}
</style>
