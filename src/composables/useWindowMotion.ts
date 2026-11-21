import { nextTick, onUnmounted, ref } from 'vue';

/**
 * 预热的浮层只在 show/hide 时切换内容，挂载动画不会重播。
 * 这里用一次性 class 重播入场；离场先等 CSS 动画，再交给窗口 hide。
 */
export function useWindowMotion() {
  const entering = ref(false);
  const leaving = ref(false);
  let timer: ReturnType<typeof setTimeout> | null = null;
  let generation = 0;

  async function enter(): Promise<void> {
    generation += 1;
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    leaving.value = false;
    entering.value = false;
    await nextTick();
    entering.value = true;
  }

  function leave(after: () => void | Promise<void>, ms = 140): Promise<void> {
    const current = ++generation;
    entering.value = false;
    leaving.value = true;
    if (timer !== null) clearTimeout(timer);
    return new Promise((resolve) => {
      timer = setTimeout(async () => {
        timer = null;
        if (current !== generation) {
          resolve();
          return;
        }
        try {
          await after();
        } finally {
          leaving.value = false;
          resolve();
        }
      }, ms);
    });
  }

  onUnmounted(() => {
    generation += 1;
    if (timer !== null) clearTimeout(timer);
    timer = null;
  });

  return { entering, leaving, enter, leave };
}
