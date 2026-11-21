import { nextTick, onBeforeUnmount, type Ref } from 'vue';

interface DragReorderOptions {
  /** 列表容器：其后代中带 `data-drag-item` 的元素参与排序。 */
  container: Ref<HTMLElement | null>;
  /** 把手选择器，相对事件目标 `closest` 查找。 */
  handleSelector: string;
  /** 松手提交新顺序；下标是拖动开始时的可见下标。 */
  onCommit: (from: number, to: number) => void;
}

/** 位移小于此值（屏幕 px）视为点击把手而非拖动。 */
const CLICK_TOLERANCE = 3;

const SHIFT_EASE = 'transform 180ms cubic-bezier(0.2, 0.8, 0.2, 1)';
const SETTLE_EASE = 'transform 200ms cubic-bezier(0.2, 0.8, 0.2, 1)';

/**
 * 指针拖拽排序（自带拿起-拖动-让位-放下动效）。
 *
 * 主窗根节点带 `transform: scale(var(--s))`，第三方排序库的坐标换算在
 * 缩放容器下会失真（卡片不跟手），这里显式按“屏幕位移 ÷ 缩放系数”换算，
 * 保证卡片与指针 1:1 跟随。
 */
export function useDragReorder(options: DragReorderOptions) {

  let items: HTMLElement[] = [];
  let slots: DOMRect[] = [];
  let snapshots: { el: HTMLElement; rect: DOMRect }[] = [];
  let from = 0;
  let to = 0;
  let startX = 0;
  let startY = 0;
  let lastDx = 0;
  let lastDy = 0;
  let scale = 1;
  let active = false;

  /** 布局缩放系数：容器的视觉宽 / 布局宽。 */
  function measureScale(container: HTMLElement): number {
    const rect = container.getBoundingClientRect();
    return container.offsetWidth > 0 && rect.width > 0 ? rect.width / container.offsetWidth : 1;
  }

  /** from→to 之后，原下标 index 落到哪个槽位。 */
  function slotOf(index: number): number {
    if (index === from) return to;
    if (from < to) return index > from && index <= to ? index - 1 : index;
    return index >= to && index < from ? index + 1 : index;
  }

  /** 其他卡片让位：平移到目标槽位（带过渡）。 */
  function shiftOthers(): void {
    for (let i = 0; i < items.length; i += 1) {
      if (i === from) continue;
      const el = items[i];
      const target = slots[slotOf(i)];
      const origin = slots[i];
      const dx = (target.left - origin.left) / scale;
      const dy = (target.top - origin.top) / scale;
      el.classList.add('drag-shift');
      el.style.transform = dx === 0 && dy === 0 ? '' : `translate3d(${dx}px, ${dy}px, 0)`;
    }
  }

  /** 指针所在槽位 = 中心距指针最近的槽。 */
  function nearestSlot(x: number, y: number): number {
    let best = to;
    let bestDist = Infinity;
    for (let i = 0; i < slots.length; i += 1) {
      const rect = slots[i];
      const dx = rect.left + rect.width / 2 - x;
      const dy = rect.top + rect.height / 2 - y;
      const dist = dx * dx + dy * dy;
      if (dist < bestDist) {
        bestDist = dist;
        best = i;
      }
    }
    return best;
  }

  function cleanupStyles(): void {
    for (const el of items) {
      el.style.transform = '';
      el.style.transition = '';
      el.classList.remove('drag-shift', 'drag-lift');
    }
    document.body.style.userSelect = '';
    items = [];
    slots = [];
    snapshots = [];
    active = false;
  }

  function onPointerMove(event: PointerEvent): void {
    if (!active) return;
    event.preventDefault();
    lastDx = (event.clientX - startX) / scale;
    lastDy = (event.clientY - startY) / scale;

    const dragged = items[from];
    dragged.style.transform =
      `translate3d(${lastDx}px, ${lastDy}px, 0) scale(1.03) rotate(0.6deg)`;

    const next = nearestSlot(
      slots[from].left + slots[from].width / 2 + event.clientX - startX,
      slots[from].top + slots[from].height / 2 + event.clientY - startY,
    );
    if (next !== to) {
      to = next;
      shiftOthers();
    }
  }

  async function settleAndCommit(moved: boolean): Promise<void> {
    const dragged = items[from];

    if (!moved || to === from) {
      // 没换位：整组复原（拖拽卡弹回原槽，让位卡清掉偏移）
      for (const el of items) {
        el.style.transition = SHIFT_EASE;
      }
      dragged.style.transform = '';
      for (let i = 0; i < items.length; i += 1) {
        if (i === from) continue;
        items[i].style.transform = '';
      }
      window.setTimeout(cleanupStyles, 210);
      return;
    }

    // FLIP：记录旧视觉位置 → 提交顺序 → 反向补偿 → 过渡归位
    snapshots = items.map((el) => ({ el, rect: el.getBoundingClientRect() }));
    const fromIndex = from;
    const toIndex = to;
    options.onCommit(fromIndex, toIndex);
    await nextTick();

    // 先清掉拖拽期的内联位移再测新布局位，否则变换叠加测不准
    for (const { el } of snapshots) {
      el.style.transform = '';
    }
    for (const { el, rect: first } of snapshots) {
      const last = el.getBoundingClientRect();
      const dx = (first.left - last.left) / scale;
      const dy = (first.top - last.top) / scale;
      el.style.transition = 'none';
      el.style.transform = dx === 0 && dy === 0 ? '' : `translate3d(${dx}px, ${dy}px, 0)`;
    }
    // 强制回流后再放开过渡，归位动画才生效
    void document.body.offsetHeight;
    for (const { el } of snapshots) {
      el.style.transition = SETTLE_EASE;
      el.style.transform = '';
    }
    window.setTimeout(cleanupStyles, 220);
  }

  function onPointerUp(event: PointerEvent): void {
    if (!active) return;
    const moved =
      Math.abs(event.clientX - startX) > CLICK_TOLERANCE ||
      Math.abs(event.clientY - startY) > CLICK_TOLERANCE;
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
    // 放下前先把抬起的放大/旋转去掉，测量才准
    items[from].style.transform = `translate3d(${lastDx}px, ${lastDy}px, 0)`;
    void settleAndCommit(moved);
  }

  function onPointerDown(event: PointerEvent): void {
    if (event.button !== 0 || active) return;
    const container = options.container.value;
    if (!container) return;
    const target = event.target as Element | null;
    const handle = target?.closest(options.handleSelector);
    if (!handle || !container.contains(handle)) return;
    const item = handle.closest('[data-drag-item]') as HTMLElement | null;
    if (!item) return;

    const list = Array.from(container.querySelectorAll<HTMLElement>('[data-drag-item]'));
    const index = list.indexOf(item);
    if (index < 0 || list.length < 2) return;

    event.preventDefault();
    active = true;
    items = list;
    slots = list.map((el) => el.getBoundingClientRect());
    from = index;
    to = index;
    startX = event.clientX;
    startY = event.clientY;
    lastDx = 0;
    lastDy = 0;
    scale = measureScale(container);

    item.classList.add('drag-lift');
    item.style.transition = 'none';
    item.style.transform = 'translate3d(0, 0, 0) scale(1.03) rotate(0.6deg)';
    document.body.style.userSelect = 'none';

    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    window.addEventListener('pointercancel', onPointerUp);
  }

  onBeforeUnmount(() => {
    if (!active) return;
    window.removeEventListener('pointermove', onPointerMove);
    window.removeEventListener('pointerup', onPointerUp);
    window.removeEventListener('pointercancel', onPointerUp);
    cleanupStyles();
  });

  return { onPointerDown };
}
