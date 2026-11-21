/** 壁纸图层的像素几何：按 "cover × 缩放" 排版整张图片，再叠加与缩放无关的平移。 */
export interface WallpaperGeometry {
  width: number
  height: number
  left: number
  top: number
}

/**
 * `dx`/`dy` 是相对面板宽高的百分比（±50 = 半屏），与缩放比例相互独立；
 * 图片边缘真正进入面板内部时，露出的部分由主题白/黑填充。
 */
export function wallpaperGeometry(
  boxWidth: number,
  boxHeight: number,
  imageWidth: number,
  imageHeight: number,
  ratio: number,
  dx: number,
  dy: number,
): WallpaperGeometry {
  const cover = Math.max(boxWidth / imageWidth, boxHeight / imageHeight)
  const scale = cover * ratio
  const width = imageWidth * scale
  const height = imageHeight * scale
  const left = (boxWidth - width) / 2 + (dx / 100) * boxWidth
  const top = (boxHeight - height) / 2 + (dy / 100) * boxHeight
  return { width, height, left, top }
}