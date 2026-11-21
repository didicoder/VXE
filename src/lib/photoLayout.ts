/**
 * 实物卡片栅格几何：设备页固定行高、总览页「一屏放下全部卡片」的行高。
 *
 * 页面设计尺寸 816×632；数字全部来自 `DESIGN.md` 的布局常量，
 * 只做纯计算，组件里别再各自写 401/514/424 这类魔法数。
 */

/** 设备页卡片区可用高度：页高 632 − 底部 inset 8 − 栅格 padding-top 20。 */
export const DEVICES_AVAILABLE_H = 604
export const PHOTO_GAP = 14

/**
 * 设备页行高：2 列一行铺满整页，3 列两行铺满整页。
 * 多出来的卡片继续按同一行高往下排，由 `.dev-scroll` 滚动。
 */
export function devicesRowHeight(cols: number): number {
  const rows = cols === 3 ? 2 : 1
  return (DEVICES_AVAILABLE_H - PHOTO_GAP * (rows - 1)) / rows
}

/** 卡片图片的目标尺寸：按“卡片长边占比”归一化，并保证旋转后不超出卡片。 */
interface CardImageBox {
  width: number
  height: number
}

/**
 * 以图片「旋转后的外接框」计算卡片内尺寸：
 * - 先把旋转后的图片按 contain 完整放进卡片（100%）；
 * - 再整体乘以 ratioPct%（1–100，默认 80）。
 *
 * 这样图片长边会自动对应卡片上同方向的那条边（竖向图对应卡片高、横向图对应卡片宽），
 * 旋转 90° 后对应关系随之互换；因为比例 ≤100%，结果永远不会超出卡片。
 */
export function cardImageBox(
  naturalW: number,
  naturalH: number,
  cardW: number,
  cardH: number,
  rotDeg: number,
  ratioPct: number,
): CardImageBox | null {
  if (!(naturalW > 0 && naturalH > 0 && cardW > 0 && cardH > 0)) return null
  const rad = (rotDeg * Math.PI) / 180
  const cos = Math.abs(Math.cos(rad))
  const sin = Math.abs(Math.sin(rad))
  const rotatedW = naturalW * cos + naturalH * sin
  const rotatedH = naturalW * sin + naturalH * cos
  if (!(rotatedW > 0 && rotatedH > 0)) return null

  const ratio = Math.min(100, Math.max(1, ratioPct)) / 100
  const contain = Math.min(cardW / rotatedW, cardH / rotatedH)
  const scale = contain * ratio

  return {
    width: naturalW * scale,
    height: naturalH * scale,
  }
}
/** 总览可用区域与卡片目标宽高比（竖版海报卡）。 */
export const OVERVIEW_WIDTH = 816
export const OVERVIEW_HEIGHT = 424
export const OVERVIEW_CARD_ASPECT = 2 / 3

export interface OverviewFit {
  cols: number
  rows: number
  cardWidth: number
  cardHeight: number
  gridWidth: number
  gridHeight: number
}

/**
 * 总览自适应排版：所有卡片塞进一屏（不滚动），设备少时放大、多时缩小；
 * 枚举列数，取能让单卡最大的方案，返回整组尺寸供页面居中。
 */
export function overviewFitLayout(
  count: number,
  width: number = OVERVIEW_WIDTH,
  height: number = OVERVIEW_HEIGHT,
  gap: number = PHOTO_GAP,
  aspect: number = OVERVIEW_CARD_ASPECT,
): OverviewFit {
  const items = Math.max(1, Math.floor(Number.isFinite(count) ? count : 1))
  let best: { cols: number; rows: number; cardWidth: number; cardHeight: number } | null = null
  for (let cols = 1; cols <= items; cols += 1) {
    const rows = Math.ceil(items / cols)
    const cellWidth = (width - gap * (cols - 1)) / cols
    const cellHeight = (height - gap * (rows - 1)) / rows
    if (!(cellWidth > 0) || !(cellHeight > 0)) continue
    const cardWidth = Math.min(cellWidth, cellHeight * aspect)
    if (
      !best ||
      cardWidth > best.cardWidth + 0.5 ||
      (Math.abs(cardWidth - best.cardWidth) <= 0.5 && rows < best.rows)
    ) {
      best = { cols, rows, cardWidth, cardHeight: cardWidth / aspect }
    }
  }
  const chosen = best ?? { cols: 1, rows: 1, cardWidth: width, cardHeight: height }
  return {
    ...chosen,
    gridWidth: chosen.cols * chosen.cardWidth + gap * (chosen.cols - 1),
    gridHeight: chosen.rows * chosen.cardHeight + gap * (chosen.rows - 1),
  }
}