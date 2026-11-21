/**
 * 滚动条「只在滑动时出现」：滚动期间给容器打 `data-scrolling`，停手约 0.9 秒后移除。
 *
 * 原生滚动条没法直接淡入淡出，所以只切换属性、由 CSS（`base.css` 的 `.fade-scroll`）
 * 决定拇指颜色：默认透明 = 看不见，`[data-scrolling]` 时才用 `--thumb`。
 * 一处 capture 监听覆盖所有窗口 / 所有滚动容器，页面组件不用各自写 JS。
 */

const HIDE_DELAY_MS = 900;

export function installScrollFade(delay: number = HIDE_DELAY_MS): () => void {
  const timers = new WeakMap<HTMLElement, ReturnType<typeof setTimeout>>();

  function onScroll(event: Event): void {
    const target = event.target;
    if (!(target instanceof HTMLElement)) return;
    target.dataset.scrolling = '';
    const pending = timers.get(target);
    if (pending !== undefined) clearTimeout(pending);
    timers.set(
      target,
      setTimeout(() => {
        delete target.dataset.scrolling;
        timers.delete(target);
      }, delay),
    );
  }

  document.addEventListener('scroll', onScroll, true);
  return () => document.removeEventListener('scroll', onScroll, true);
}
