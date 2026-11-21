/**
 * 列表排序：把「可见子集的新顺序」写回完整列表。
 *
 * 设备页/总览页会把 `absent` 的设备过滤掉再渲染，所以拖拽拿到的是**可见项的下标**，
 * 不能直接拿去 `moveLocal(from, to)` 操作完整列表（有隐藏设备时两个下标体系会错位）。
 * 这里按 id 把可见项排好序，再填回它们在完整列表里原来的槽位，隐藏项原地不动。
 */

interface Identified {
  id: string
}

export function reorderSubset<T extends Identified>(
  list: readonly T[],
  order: readonly string[],
): T[] {
  const wanted = new Set(order)
  const byId = new Map(list.filter((item) => wanted.has(item.id)).map((item) => [item.id, item]))
  // 先滤掉列表里不存在的 id，避免一个坏 id 让后续槽位整体错位、出现重复项
  const resolved = order.filter((id) => byId.has(id))
  let cursor = 0
  return list.map((item) => {
    if (!wanted.has(item.id)) return item
    const next = byId.get(resolved[cursor])
    cursor += 1
    return next ?? item
  })
}
