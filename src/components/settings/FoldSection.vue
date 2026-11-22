<script setup lang="ts">
/**
 * 设置卡内的折叠区：点头部展开/收起设置行。
 * 展开图标为 useanimations menu-v4（汉堡 ↔ 关闭，Vue 适配见 MenuToggleIcon）。
 * `disabled` 时整组置灰且不可修改（提醒方式只选另一项时用）。
 * `size="row"` 与普通设置行同大（顶层收纳用）；默认 `sub` 小号（二级收纳用）。
 */
import { ref, watch } from 'vue';
import MenuToggleIcon from '../icons/MenuToggleIcon.vue';

const {
  title,
  expanded = false,
  disabled = false,
  size = 'sub',
} = defineProps<{
  title: string;
  expanded?: boolean;
  disabled?: boolean;
  size?: 'sub' | 'row';
}>();

const emit = defineEmits<{
  toggle: [];
}>();

const mountedOnce = ref(expanded);
watch(() => expanded, (value) => {
  if (value) mountedOnce.value = true;
});

function toggle(): void {
  if (!mountedOnce.value) mountedOnce.value = true;
  emit('toggle');
}
</script>

<template>
  <div class="fold" :class="[`fold--${size}`, { 'fold--disabled': disabled }]">
    <button
      type="button"
      class="fold__head"
      :aria-expanded="expanded"
      :disabled="disabled"
      @click="toggle"
    >
      <MenuToggleIcon :size="size === 'row' ? 20 : 18" :active="expanded" />
      <span class="fold__title">{{ title }}</span>
      <span class="fold__hint">{{ expanded ? '收起' : '展开设置' }}</span>
    </button>
    <div v-if="mountedOnce" v-show="expanded" class="fold__body" :class="`fold__body--${size}`">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.fold {
  border-top: 1px solid var(--card-line);
  margin-top: 2px;
  padding-top: 2px;
}

.fold--disabled {
  opacity: 0.45;
  pointer-events: none;
}

.fold__head {
  display: flex;
  gap: 10px;
  align-items: center;
  width: 100%;
  height: 40px;
  padding: 0 12px 0 4px;
  border: 0;
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  text-align: left;
  cursor: pointer;
}

/* 顶层收纳：与 SettingRow 同高同字（标签 18 bold + 右侧 13 提示） */
.fold--row .fold__head {
  height: 64px;
  font-size: var(--fs-label);
}

.fold__hint {
  margin-left: auto;
  color: var(--ink-3);
  font-size: 12.5px;
  font-weight: 400;
}

.fold--row .fold__hint {
  font-size: 13px;
  line-height: 14px;
}

.fold__body {
  padding-top: 4px;
}

/* 展开内容与折叠标题同层级：只按标题图标宽度右移，不再降一级字号 */
.fold__body--row {
  padding-left: 34px;
}

.fold__body--sub {
  padding-left: 32px;
}
</style>
