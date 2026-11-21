<script setup lang="ts">
import BatteryOrb from '../BatteryOrb.vue';
import BatteryRing from '../BatteryRing.vue';
import type { BatteryBand } from '../../types/battery';

const {
  level,
  size = 155.6,
  thickness = 12,
  color = 'var(--good)',
  tone = 'none',
  label = '整体电量',
  orbSize = 50,
} = defineProps<{
  /** 展示电量（Hero 用平均电量）；null = 没有读数，只画轨道 */
  level: number | null;
  /** 环外径（= 2 × rr + rw，V16 Hero 为 71.8 × 2 + 12） */
  size?: number;
  thickness?: number;
  color?: string;
  /** 闪电组件档位：low 时核心呼吸闪烁。 */
  tone?: BatteryBand | 'none';
  label?: string;
  /** 环心闪电的直径；预览小窗等缩放场景由调用方传入。 */
  orbSize?: number;
}>();
</script>

<template>
  <div
    class="hero-ring"
    :style="{ width: `${size}px`, height: `${size}px` }"
    role="img"
    :aria-label="label"
  >
    <BatteryRing
      :level="level"
      :size="size"
      :thickness="thickness"
      :color="color"
    />
    <BatteryOrb
      class="hero-ring__orb"
      :tone="tone"
      :color="color"
      :style="{ '--orb-size': `${orbSize}px` }"
    />
  </div>
</template>

<style scoped>
.hero-ring {
  position: relative;
  flex: none;
}

.hero-ring__orb {
  position: absolute;
  top: 50%;
  left: 50%;
  /* 波纹最大 1.6×，50px 扩散后仍在 93px 环内。 */
  --orb-size: 50px;
  transform: translate(-50%, -50%);
}
</style>
