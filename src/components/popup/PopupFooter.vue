<script setup lang="ts">
/**
 * 预览小窗底栏：添加设备 / 打开面板（V16 `popup.py` 两键）。
 *
 * 几何按 V16：FOOT_H=44、FOOT_GAP=10、底部留白 16（由窗口根承载）。
 */
import LineIcon from '../icons/LineIcon.vue';

const emit = defineEmits<{
  add: [];
  open: [];
}>();
</script>

<template>
  <footer class="popup-footer">
    <button type="button" class="popup-footer__btn" @click="emit('add')">
      <LineIcon name="plus" :size="9.75" :stroke="1.275" />
      <span>添加设备</span>
    </button>
    <button
      type="button"
      class="popup-footer__btn popup-footer__btn--primary"
      @click="emit('open')"
    >
      打开面板
    </button>
  </footer>
</template>

<style scoped>
.popup-footer {
  display: flex;
  flex: none;
  gap: 7.5px;
  height: 33px;
  margin-top: 7.5px;
  padding: 0 18px;
}

.popup-footer__btn {
  display: inline-flex;
  flex: 1;
  align-items: center;
  justify-content: center;
  gap: 4.5px;
  min-width: 0;
  height: 33px;
  padding: 0 7.5px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 11.25px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    transform var(--speed) var(--ease),
    filter var(--speed) var(--ease),
    opacity var(--speed) var(--ease);
}

.popup-footer__btn--primary {
  border-color: var(--cta-line);
  background: var(--cta-bg);
  box-shadow: var(--cta-shadow);
  color: var(--cta-fg);
}

.popup-footer__btn:focus-visible {
  outline: 1.5px solid var(--accent);
  outline-offset: -1.125px;
}

.popup-footer__btn:disabled {
  cursor: default;
  opacity: 0.45;
}

@media (hover: hover) {
  .popup-footer__btn:not(:disabled):hover {
    transform: translateY(-1px);
    background: var(--ctrl-bg-hover);
    border-color: var(--ctrl-line-hover);
    box-shadow: var(--ctrl-shadow-hover);
  }

  .popup-footer__btn--primary:not(:disabled):hover {
    border-color: var(--cta-line);
    background: var(--cta-bg);
    box-shadow: var(--cta-shadow-hover);
    filter: none;
  }
}
</style>
