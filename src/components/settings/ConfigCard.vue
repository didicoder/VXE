<script setup lang="ts">
/**
 * 配置目录卡（高 90、与设置卡同宽）：左侧「配置目录 19 bold + 路径 15 muted」，
 * 右侧「恢复默认设置 / 打开目录」两枚 108×34 胶囊。
 *
 * 只负责展示与发声，IPC 由设置页持有；路径过长按尾部截断（V16 同口径）。
 */

import { computed } from 'vue';
import PillButton from '../PillButton.vue';

const {
  path,
  busy = false,
} = defineProps<{
  path: string;
  busy?: boolean;
}>();

const emit = defineEmits<{
  reset: [];
  open: [];
}>();

/** 展示用路径：超过 56 字符保留尾部 52，与 V16「…+尾部」一致。 */
const displayPath = computed(() => {
  const value = path.trim();
  if (!value) return '正在读取配置路径…';
  return value.length <= 56 ? value : `…${value.slice(-52)}`;
});
</script>

<template>
  <section class="cfg glass-card" aria-label="配置目录">
    <div class="cfg__text">
      <span class="cfg__title">配置目录</span>
      <span class="cfg__path" :title="path">{{ displayPath }}</span>
    </div>
    <div class="cfg__actions">
      <PillButton
        class="cfg__button"
        size="sm"
        :disabled="busy"
        @click="emit('reset')"
      >
        恢复默认设置
      </PillButton>
      <PillButton
        class="cfg__button"
        size="sm"
        :disabled="busy"
        @click="emit('open')"
      >
        打开目录
      </PillButton>
    </div>
  </section>
</template>

<style scoped>
/* 配置目录卡：表面走 .glass-card */
.cfg {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  height: 90px;
  padding: 0 18px 0 24px;
}

.cfg__text {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.cfg__title {
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 18px;
}

.cfg__path {
  /* 14px 行盒装不下带下伸字母（j/g 等）的 13px 文本：
     上下各垫 2px 再用负 margin 抵消，裁剪盒放大到 18px，视觉位置不变 */
  overflow: hidden;
  padding: 2px 0;
  margin: -2px 0;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 400;
  line-height: 14px;
  white-space: nowrap;
}

.cfg__actions {
  display: flex;
  align-items: center;
  gap: 10px;
  flex: none;
}

.cfg__button {
  min-width: 108px;
}
</style>
