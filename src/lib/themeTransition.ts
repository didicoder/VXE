/**
 * 主题切换的圆形揭开动画（CIRCLE）。
 *
 * 参考 `baiwumm/theme-switch-animation` 的 CIRCLE 形态：以触发按钮为圆心，
 * 新主题从 0 半径扩到覆盖整个视口。这里用 View Transitions 截取新旧两帧，
 * 只对一帧裁一个圆，另一帧保持不动，避免逐元素改 transition。
 *
 * `reverse` 对齐参考库 `reverse: 'auto'` 的收起形态：旧主题的圆面缩回触发点，
 * 新主题从四周显出（「切亮收起」）；缺省正向扩散（「切暗扩散」）。
 *
 * 不支持 View Transitions（或开启减少动态效果）时直接切主题，不阻断功能。
 */

export interface CircleOrigin {
  x: number;
  y: number;
}

interface CircleTransitionOptions {
  /** 反向收起：旧主题缩回圆心；缺省为正向扩散。 */
  reverse?: boolean;
}

/** 与新帧 clip-path 动画时长一致（`styles/base.css`），对齐参考项目的 750 ms。 */

function prefersReducedMotion(): boolean {
  return (
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
}

/** 圆心到视口四角的最远距离，加 2px 防止末帧露边。 */
export function circleRadius(origin: CircleOrigin, width: number, height: number): number {
  const dx = Math.max(Math.abs(origin.x), Math.abs(width - origin.x));
  const dy = Math.max(Math.abs(origin.y), Math.abs(height - origin.y));
  return Math.ceil(Math.hypot(dx, dy)) + 2;
}

/**
 * 以 `origin` 为圆心切换主题。
 *
 * `update` 可以返回 Promise：View Transitions 会等它完成后再截新帧，
 * 因此主题落盘（IPC）与 class 更新都在动画开始前完成。
 */
export function startCircleThemeTransition(
  origin: CircleOrigin,
  update: () => void | Promise<void>,
  options: CircleTransitionOptions = {},
): void {
  const root = document.documentElement;
  const width = typeof window === 'undefined' ? 0 : window.innerWidth;
  const height = typeof window === 'undefined' ? 0 : window.innerHeight;

  root.style.setProperty('--theme-circle-x', `${origin.x}px`);
  root.style.setProperty('--theme-circle-y', `${origin.y}px`);
  root.style.setProperty('--theme-circle-r', `${circleRadius(origin, width, height)}px`);

  if (typeof document.startViewTransition !== 'function' || prefersReducedMotion()) {
    void update();
    return;
  }

  root.classList.add('is-theme-switching');
  root.classList.toggle('is-theme-switching--collapse', options.reverse === true);
  const transition = document.startViewTransition(update);
  void transition.finished.finally(() => {
    root.classList.remove('is-theme-switching', 'is-theme-switching--collapse');
  });
}
