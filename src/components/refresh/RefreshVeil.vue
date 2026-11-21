<script setup lang="ts">
/**
 * 刷新遮罩：圆形从页头刷新按钮扩散，露出当前页的骨架加载态。
 *
 * 只在手动刷新时出现；数据回填后由 MainWindow 先挂 `leaving` 淡出再卸载。
 * 位置/背景尺寸由父级给，圆心的 CSS 变量也由父级写。
 */
import { computed } from 'vue';
import {
  devicesRowHeight,
  overviewFitLayout,
} from '../../lib/photoLayout';
import type { UiPage } from '../../stores/ui';

const {
  page,
  leaving = false,
  count = 4,
  photo = false,
  cols = 3,
} = defineProps<{
  page: UiPage;
  leaving?: boolean;
  /** 总览卡片或设备行数；设置页固定三张宽卡。 */
  count?: number;
  /** 实物卡片 UI：骨架用竖版海报卡，而不是横条卡。 */
  photo?: boolean;
  cols?: 2 | 3;
}>();

const SKELETON_COUNT = computed(() => Math.max(0, Math.min(5, Math.trunc(count))));
const SETTINGS_CARDS = [176, 218, 176];

/** 骨架卡与真实页面同高：总览按数量分屏，设备页按 2/3 列规则。 */
const photoGridStyle = computed(() => {
  if (!photo) return {};
  if (page === 'summary') {
    const fit = overviewFitLayout(Math.max(2, SKELETON_COUNT.value));
    return {
      width: `${fit.gridWidth.toFixed(2)}px`,
      height: `${fit.gridHeight.toFixed(2)}px`,
      '--sk-w': `${fit.cardWidth.toFixed(2)}px`,
      '--sk-h': `${fit.cardHeight.toFixed(2)}px`,
    };
  }
  if (page === 'devices') {
    return { '--sk-h': `${devicesRowHeight(cols)}px` };
  }
  return {};
});
</script>

<template>
  <div
    class="refresh-veil"
    :class="{ 'refresh-veil--leaving': leaving }"
    aria-hidden="true"
  >
    <div class="refresh-skeleton">
      <template v-if="page === 'summary'">
        <div class="sk-hero sk-surface">
          <span class="sk-ring"></span>
          <div class="sk-hero__value">
            <span class="sk-block sk-num"></span>
            <span class="sk-block sk-line sk-line--md"></span>
          </div>
          <div class="sk-hero__side">
            <span class="sk-block sk-line sk-line--sm"></span>
            <span class="sk-block sk-line sk-line--xs"></span>
          </div>
        </div>
        <span class="sk-block sk-head"></span>
        <div v-if="SKELETON_COUNT > 0 || page === 'summary'" class="sk-grid" :class="{ 'sk-grid--photo': photo, 'sk-grid--fit': photo && page === 'summary', 'sk-grid--3': photo && cols === 3 && page !== 'summary' }" :style="photoGridStyle">
          <div
            v-for="n in (page === 'summary' ? Math.max(2, SKELETON_COUNT) : SKELETON_COUNT)"
            :key="n"
            class="sk-card sk-surface"
            :class="{ 'sk-card--photo': photo }"
          >
            <template v-if="photo">
              <span class="sk-block sk-line sk-line--md"></span>
              <span class="sk-block sk-line sk-line--sm"></span>
              <span class="sk-block sk-image"></span>
              <span class="sk-block sk-line sk-line--xs"></span>
            </template>
            <template v-else>
              <span class="sk-dot sk-dot--lg"></span>
              <div class="sk-card__text">
                <span class="sk-block sk-line sk-line--md"></span>
                <span class="sk-block sk-line sk-line--xs"></span>
              </div>
            </template>
          </div>
        </div>
      </template>

      <template v-else-if="page === 'devices'">
        <div v-if="photo" class="sk-grid sk-grid--photo sk-grid--devices" :class="{ 'sk-grid--3': cols === 3 }" :style="photoGridStyle">
          <div
            v-for="n in Math.max(1, SKELETON_COUNT)"
            :key="n"
            class="sk-card sk-card--photo sk-surface"
          >
            <span class="sk-block sk-line sk-line--md"></span>
            <span class="sk-block sk-line sk-line--sm"></span>
            <span class="sk-block sk-image"></span>
            <span class="sk-block sk-line sk-line--xs"></span>
          </div>
        </div>
        <div v-else class="sk-rows">
          <div v-for="n in Math.max(1, SKELETON_COUNT)" :key="n" class="sk-row sk-surface">
            <span class="sk-dot sk-dot--lg"></span>
            <div class="sk-row__text">
              <span class="sk-block sk-line sk-line--md"></span>
              <span class="sk-block sk-line sk-line--xs"></span>
            </div>
            <span class="sk-chip"></span>
          </div>
        </div>
      </template>

      <template v-else>
        <div class="sk-stack">
          <div
            v-for="(height, index) in SETTINGS_CARDS"
            :key="index"
            class="sk-wide sk-surface"
            :style="{ height: `${height}px` }"
          >
            <span class="sk-block sk-line sk-line--head"></span>
            <span class="sk-block sk-line sk-line--wide"></span>
            <span class="sk-block sk-line sk-line--wide"></span>
            <span class="sk-block sk-line sk-line--wide"></span>
          </div>
        </div>
      </template>

    </div>
  </div>
</template>

<style scoped>
/* 遮罩从页头下缘开始：圆形圆心在按钮中心（y 为负），扫入内容区。
 * 全透明：刷新期间页面内容由 MainWindow 隐藏，骨架卡直接落在窗口壁纸（页面背景）上，
 * 不做任何填充/底色，玻璃骨架保持玻璃效果。 */
.refresh-veil {
  position: absolute;
  inset: 88px 0 0;
  z-index: 20;
  overflow: hidden;
  background: transparent;
  clip-path: circle(0px at var(--refresh-x, 50%) var(--refresh-y, 50%));
  animation: refresh-sweep 600ms var(--ease-out) both;
}

.refresh-veil--leaving {
  opacity: 0;
  transition: opacity 220ms ease;
}

@keyframes refresh-sweep {
  from {
    clip-path: circle(0px at var(--refresh-x, 50%) var(--refresh-y, 50%));
  }

  to {
    clip-path: circle(var(--refresh-r, 150vmax) at var(--refresh-x, 50%) var(--refresh-y, 50%));
  }
}

.refresh-skeleton {
  position: absolute;
  top: 8px;
  left: 20px;
  z-index: 1;
  width: 816px;
  height: 624px;
}

/* 侧栏收起时内容区 padding 变成 82，骨架跟着页面重新居中 */
.refresh-veil--min .refresh-skeleton {
  left: 82px;
}

/* 骨架卡：玻璃面（半透明渐变）+ inset 发丝线（底边闭合）；磨砂垫底盖住旧内容 */
.sk-surface {
  position: relative;
  border: 0;
  border-radius: var(--r-card);
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  box-shadow: inset 0 0 0 1px var(--card-line);
  overflow: hidden;
}

.sk-surface::after {
  position: absolute;
  inset: 0;
  background: linear-gradient(
    105deg,
    transparent 32%,
    color-mix(in srgb, var(--txt) 6%, transparent) 48%,
    transparent 64%
  );
  background-size: 220% 100%;
  content: "";
  pointer-events: none;
  animation: refresh-sheen 1.35s ease-in-out infinite;
}

@keyframes refresh-sheen {
  from {
    background-position: 110% 0;
  }

  to {
    background-position: -110% 0;
  }
}

.sk-block {
  display: block;
  flex: none;
  border-radius: var(--r-ctrl);
  background: var(--card);
}

.sk-line {
  height: 14px;
  background: var(--track);
}

.sk-line--xs {
  width: 96px;
  height: 11px;
}

.sk-line--sm {
  width: 148px;
}

.sk-line--md {
  width: 196px;
}

.sk-line--wide {
  width: 78%;
}

.sk-line--head {
  width: 132px;
  height: 20px;
}

/* ---------- 总览 ---------- */
.sk-hero {
  display: flex;
  align-items: center;
  gap: 28px;
  height: 116px;
  padding: 0 24px;
}

.sk-ring,
.sk-dot,
.sk-chip,
.sk-num,
.sk-head {
  flex: none;
}

.sk-ring {
  flex: none;
  width: 93px;
  height: 93px;
  border-radius: 50%;
  background: var(--track);
  mask: radial-gradient(circle, transparent 53%, #000 55%);
}

.sk-hero__value {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.sk-num {
  width: 96px;
  height: 36px;
  border-radius: var(--r-tile);
  background: var(--track);
}

.sk-hero__side {
  display: flex;
  flex-direction: column;
  gap: 18px;
  margin-left: auto;
}

.sk-head {
  width: 132px;
  height: 14px;
  margin-top: 14px;
  background: var(--track);
}

.sk-grid {
  display: grid;
  grid-template-columns: repeat(2, 401px);
  gap: 14px;
  margin-top: 12px;
}

.sk-card {
  display: flex;
  align-items: center;
  gap: 18px;
  height: 132px;
  padding: 0 26px;
}

.sk-card__text {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.sk-dot {
  width: 30px;
  height: 30px;
  border-radius: 50%;
  background: var(--track);
}

.sk-dot--lg {
  width: 48px;
  height: 48px;
}

/* ---------- 实物卡片骨架 ---------- */
.sk-grid--photo {
  grid-template-columns: repeat(2, minmax(0, 1fr));
  grid-auto-rows: var(--sk-h, 424px);
}

.sk-grid--photo.sk-grid--3 {
  grid-template-columns: repeat(3, minmax(0, 1fr));
}

.sk-grid--devices {
  margin-top: 20px;
}

/* 总览骨架跟随自适应排版：固定卡片尺寸在区域中心换行 */
.sk-grid--fit {
  display: flex;
  flex-wrap: wrap;
  align-content: center;
  justify-content: center;
  gap: 14px;
  margin: 12px auto 0;
}

.sk-grid--fit .sk-card {
  flex: none;
  width: var(--sk-w);
  height: var(--sk-h);
}

.sk-card--photo {
  flex-direction: column;
  align-items: flex-start;
  gap: 12px;
  height: 100%;
  padding: 26px 24px;
}

.sk-grid--devices .sk-card--photo {
  height: 100%;
}

.sk-image {
  flex: 1;
  width: 100%;
  min-height: 120px;
  border-radius: var(--r-tile);
  background: var(--track);
}

/* ---------- 设备 ---------- */
.sk-rows {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 12px;
}

.sk-row {
  display: flex;
  align-items: center;
  gap: 24px;
  height: 100px;
  padding: 0 26px;
}

.sk-row__text {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.sk-chip {
  width: 86px;
  height: 30px;
  margin-left: auto;
  border-radius: var(--r-chip);
  background: var(--track);
}

/* ---------- 设置 ---------- */
.sk-stack {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.sk-wide {
  display: flex;
  flex-direction: column;
  gap: 14px;
  padding: 22px 24px;
}

@media (prefers-reduced-motion: reduce) {
  .refresh-veil {
    clip-path: none;
    animation: none;
  }

  .refresh-veil--leaving {
    transition: none;
  }

  .sk-surface::after {
    animation: none;
  }
}
</style>
