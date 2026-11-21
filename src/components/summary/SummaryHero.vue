<script setup lang="ts">
import { computed } from 'vue';
import HeroRing from './HeroRing.vue';
import LineIcon from '../icons/LineIcon.vue';
import { bandOf, levelColor } from '../../lib/bands';
import { colorHex } from '../../lib/colors';
import { percent } from '../../lib/format';
import type { ColorKey } from '../../types/battery';

const props = defineProps<{
  total: number;
  online: number;
  avg: number;
  min: number | null;
  good: number;
  warn: number;
}>();

const emit = defineEmits<{ add: [] }>();

function bandToken(key: ColorKey): string {
  if (key === 'green') return 'var(--good)';
  if (key === 'blue') return 'var(--mid)';
  if (key === 'red') return 'var(--low)';
  return colorHex(key);
}

function levelToken(level: number | null): string {
  if (level == null) return 'var(--gray)';
  const key = levelColor(level, props.good, props.warn);
  return key ? bandToken(key) : 'var(--gray)';
}

const heroColor = computed(() =>
  props.total > 0 ? levelToken(props.avg) : 'var(--faint)',
);
const heroTone = computed(() =>
  props.total > 0 ? bandOf(props.avg, props.good, props.warn) : 'none',
);
const onlineDot = computed(() => (props.online > 0 ? 'var(--good)' : 'var(--gray)'));
const lowDot = computed(() => levelToken(props.min));
const lowText = computed(() => (props.min == null ? '--' : percent(props.min)));
</script>

<template>
  <div class="hero glass-card">
    <HeroRing
      class="hero__ring"
      :level="avg"
      :size="93"
      :thickness="7"
      :color="heroColor"
      :tone="heroTone"
    />

    <div class="hero__value">
      <span class="hero__num">{{ avg }}</span>
      <span class="hero__sign" :style="{ color: heroColor }">%</span>
    </div>

    <div class="hero__online">{{ online }} / {{ total }} 台设备在线</div>

    <div class="hero__row hero__row--top">
      <span class="hero__dot" :style="{ background: onlineDot }"></span>
      <span class="hero__label">在线设备</span>
      <span class="hero__stat">{{ online }} 台</span>
    </div>
    <div class="hero__row hero__row--bottom">
      <span class="hero__dot" :style="{ background: lowDot }"></span>
      <span class="hero__label">最低电量</span>
      <span class="hero__stat">{{ lowText }}</span>
    </div>

    <button type="button" class="hero__add" @click="emit('add')">
      <LineIcon name="plus" :size="18" :stroke="2" />
      <span>添加设备</span>
    </button>
  </div>
</template>

<style scoped>
/* Hero：高 116（原 194 的 60%）；环 rr=43 / rw=7（外径 93），圆心居中。表面走 .glass-card */
.hero {
  position: absolute;
  top: 8px;
  left: 0;
  width: 816px;
  height: 116px;
  overflow: hidden;
}

.hero__ring {
  position: absolute;
  top: 11.5px;
  left: 18px;
}

.hero__value {
  position: absolute;
  top: 44px;
  left: 124px;
  display: flex;
  align-items: baseline;
  gap: 3px;
  line-height: 1;
  transform: translateY(-50%);
}

.hero__num {
  font-family: var(--font-num);
  font-size: 30px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.03em;
  color: var(--ink);
}

.hero__sign {
  font-family: var(--font-num);
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
  transform: translateY(-2px);
}

.hero__online {
  position: absolute;
  top: 82px;
  left: 124px;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  transform: translateY(-50%);
}

.hero__row {
  position: absolute;
  left: 288px;
  display: flex;
  align-items: center;
  line-height: 1;
  white-space: nowrap;
  transform: translateY(-50%);
}

.hero__row--top {
  top: 44px;
}

.hero__row--bottom {
  top: 82px;
}

.hero__dot {
  flex: none;
  width: 6px;
  height: 6px;
  margin-right: 7px;
  border-radius: 50%;
}

.hero__label {
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
}

.hero__stat {
  margin-left: 8px;
  font-size: 16px;
  font-weight: 700;
}

.hero__add {
  position: absolute;
  top: 36px;
  left: 642px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 12px;
  width: 152px;
  height: 44px;
  padding: 0 0 2px;
  border: 1px solid var(--cta-line);
  border-radius: var(--r-pill);
  background: var(--cta-bg);
  box-shadow: var(--cta-shadow);
  color: var(--cta-fg);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    transform var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.hero__add:hover {
  transform: translateY(-1px);
  box-shadow: var(--cta-shadow-hover);
}

.hero__add:active {
  transform: translateY(0);
}
</style>