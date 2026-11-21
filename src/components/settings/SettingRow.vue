<script setup lang="ts">
/**
 * 设置页单行：左侧「标签 19 bold + 辅助 15 muted」，右侧控件插槽。
 *
 * 行高 64、左侧内缩 24（由父级卡片的水平内边距提供），控件右缘贴卡内缩 16。
 * 悬停整行弹预览的行用 `hover-start` / `hover-end` 通知父级（pointerenter 不冒泡，
 * 行内控件之间移动不会反复触发）。
 */

const {
  label,
  hint,
  hintTone = 'muted',
  size = 'row',
} = defineProps<{
  label: string;
  hint?: string;
  hintTone?: 'muted' | 'low';
  /** `row` = 卡片内第二层级；`sub` = 折叠区内的第三层级。 */
  size?: 'row' | 'sub';
}>();

const emit = defineEmits<{
  'hover-start': [];
  'hover-end': [];
}>();
</script>

<template>
  <div
    class="row"
    :class="`row--${size}`"
    @pointerenter="emit('hover-start')"
    @pointerleave="emit('hover-end')"
  >
    <div class="row__text">
      <span class="row__label">{{ label }}</span>
      <span
        v-if="hint"
        class="row__hint"
        :class="{ 'row__hint--low': hintTone === 'low' }"
      >{{ hint }}</span>
    </div>
    <div class="row__control">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  height: 64px;
}

/* 第三层级：与 sub 折叠区标题同字号，行高收紧 */
.row--sub {
  height: 48px;
  gap: 12px;
}

.row--sub .row__text {
  gap: 6px;
}

.row--sub .row__label {
  font-size: 15px;
  line-height: 15px;
}

.row--sub .row__hint {
  font-size: 12.5px;
  line-height: 13px;
}

.row__text {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
}

.row__label {
  color: var(--ink);
  font-family: var(--font-body);
  font-size: var(--fs-label);
  font-weight: 700;
  line-height: var(--fs-label);
  white-space: nowrap;
}

.row__hint {
  overflow: hidden;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 400;
  line-height: 14px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.row__hint--low {
  color: var(--low);
}

.row__control {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 10px;
  flex: none;
}
</style>
