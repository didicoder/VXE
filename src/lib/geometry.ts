/** 数值夹取到 `[min, max]`。 */
export function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value))
}