<script setup lang="ts">
import { computed } from 'vue';

type DeviceState = 'ok' | 'sleep' | 'absent' | 'error';

const {
  state,
  charging = false,
  text,
} = defineProps<{
  state: DeviceState;
  charging?: boolean;
  text?: string;
}>();

const STATE_TEXT: Record<DeviceState, string> = {
  ok: '已连接',
  sleep: '休眠',
  absent: '未连接',
  error: '读取失败',
};

const STATE_COLOR: Record<DeviceState, string> = {
  ok: 'var(--ink-good)',
  sleep: 'var(--ink-sleep)',
  absent: 'var(--muted)',
  error: 'var(--ink-low)',
};

const isCharging = computed(
  () => charging && (state === 'ok' || state === 'sleep'),
);

const label = computed(() => {
  if (text != null) return text;
  return isCharging.value ? '充电中' : STATE_TEXT[state];
});

const chipColor = computed(() =>
  isCharging.value ? 'var(--ink-good)' : STATE_COLOR[state],
);
</script>

<template>
  <span class="state-chip" :style="{ color: chipColor }">{{ label }}</span>
</template>

<style scoped>
.state-chip {
  display: inline-block;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}
</style>
