/** 展示文本格式化（页头日期与百分比）。 */

const WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六'] as const

/** `M月D日 星期X`（V16 页头副标题格式）。 */
export function dateLine(date: Date = new Date()): string {
  return `${date.getMonth() + 1}月${date.getDate()}日 星期${WEEKDAYS[date.getDay()]}`
}

/** 百分数文本，如 `85%`；没读数时为 `--`（V16 「最低电量」同款）。 */
export function percent(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return '--'
  return `${Math.round(value)}%`
}
