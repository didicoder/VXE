<script setup lang="ts">
import { computed } from 'vue';

const {
  level,
  width = 120,
  height = 4.5,
  color = 'var(--good)',
  track = 'var(--track)',
  radius = 2.25,
} = defineProps<{
  level: number | null;
  width?: number;
  height?: number;
  color?: string;
  track?: string;
  radius?: number;
}>();

const pct = computed(() => {
  if (level == null || !Number.isFinite(level)) return 0;
  return Math.min(100, Math.max(0, level));
});
</script>

<template>
  <div
    class="battery-bar"
    :style="{
      width: `${width}px`,
      height: `${height}px`,
      borderRadius: `${radius}px`,
      backgroundColor: track,
    }"
    aria-hidden="true"
  >
    <div
      class="battery-bar__fill"
      :style="{
        width: `${pct}%`,
        borderRadius: `${radius}px`,
        backgroundColor: color,
      }"
    />
  </div>
</template>

<style scoped>
.battery-bar {
  position: relative;
  flex: none;
  overflow: hidden;
}

.battery-bar__fill {
  height: 100%;
}
</style>
