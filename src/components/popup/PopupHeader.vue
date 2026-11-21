<script setup lang="ts">
/**
 * 预览小窗头部：左「VXE」+ 刷新时间，右刷新/关闭。
 *
 * 当前设计尺寸即 1×：头部 49.5、标题 13.5 bold、副标 9.75、按钮 33×33；
 * 前端只乘用户 popup_scale，base 仅用于旧配置兼容/迁移。
 */
import LineIcon from '../icons/LineIcon.vue';
import RefreshIcon from '../icons/RefreshIcon.vue';

const { busy = false } = defineProps<{
  busy?: boolean;
}>();

const emit = defineEmits<{
  refresh: [];
  close: [];
}>();
</script>

<template>
  <header class="popup-header">
    <div class="popup-header__text">
      <h1 class="popup-header__title">VXE</h1>
      <p class="popup-header__meta">无线设备概览</p>
    </div>
    <div class="popup-header__actions">
      <button
        type="button"
        class="icon-btn"
        aria-label="立即刷新"
        title="立即刷新"
        @click="emit('refresh')"
      >
        <RefreshIcon :size="9.75" :stroke="1.125" :busy="busy" />
      </button>
      <button
        type="button"
        class="icon-btn"
        aria-label="关闭"
        title="关闭"
        @click="emit('close')"
      >
        <LineIcon name="close" :size="9" :stroke="1.125" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.popup-header {
  position: relative;
  flex: none;
  height: 49.5px;
  padding: 12px 18px 0;
}

.popup-header__title {
  margin: 0;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 13.5px;
  font-weight: 700;
  line-height: 13.5px;
  white-space: nowrap;
}

.popup-header__meta {
  margin: 6px 0 0;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 9.75px;
  font-weight: 400;
  line-height: 9.75px;
  white-space: nowrap;
}

.popup-header__actions {
  position: absolute;
  top: 8.25px;
  right: 18px;
  display: flex;
  gap: 3px;
}

.popup-header .icon-btn {
  width: 33px;
  height: 33px;
}
</style>
