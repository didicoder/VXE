import { nextTick, onUnmounted, ref } from 'vue';

/**
 * 预热的浮层只在 show/hide 时切换内容，挂载动画不会重播。
 * 这里用一次性 class 重播入场；离场先等 CSS 动画，再交给窗口 hide。
 */
export function useWindowMotion() {
  const entering = ref(false);
  const leaving = ref(false);
  let generation = 0;
  let activeLeave: {
    generation: number;
    timer: ReturnType<typeof setTimeout> | null;
    settle: (completed: boolean) => void;
  } | null = null;

  function settleLeave(
    operation: NonNullable<typeof activeLeave>,
    completed: boolean,
  ): void {
    if (activeLeave !== operation) return;
    activeLeave = null;
    if (operation.timer !== null) clearTimeout(operation.timer);
    operation.timer = null;
    operation.settle(completed);
  }

  function cancelLeave(): void {
    if (activeLeave) settleLeave(activeLeave, false);
  }

  async function enter(): Promise<void> {
    const current = ++generation;
    cancelLeave();
    leaving.value = false;
    entering.value = false;
    await nextTick();
    if (current === generation) entering.value = true;
  }

  function leave(after: () => void | Promise<void>, ms = 140): Promise<boolean> {
    const current = ++generation;
    cancelLeave();
    entering.value = false;
    leaving.value = true;

    return new Promise((resolve) => {
      const operation = {
        generation: current,
        timer: null as ReturnType<typeof setTimeout> | null,
        settle: resolve,
      };
      activeLeave = operation;
      operation.timer = setTimeout(() => {
        operation.timer = null;
        if (activeLeave !== operation || operation.generation !== generation) {
          settleLeave(operation, false);
          return;
        }
        void (async () => {
          try {
            await after();
          } catch {
            if (activeLeave === operation && operation.generation === generation) {
              leaving.value = false;
            }
            settleLeave(operation, false);
            return;
          }
          if (activeLeave === operation && operation.generation === generation) {
            leaving.value = false;
            settleLeave(operation, true);
          } else {
            settleLeave(operation, false);
          }
        })();
      }, ms);
    });
  }

  onUnmounted(() => {
    generation += 1;
    cancelLeave();
  });

  return { entering, leaving, enter, leave };
}
