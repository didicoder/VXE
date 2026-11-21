/**
 * 卡片图片预处理（选图 / 拖图共用）。
 *
 * 两级策略，保证「选了就一定能换上」：
 * 1. 正常路径：等比缩到最长边 ≤ 1024，再编码（PNG 保透明，过大改 JPEG）——
 *    卡片上最大也就 400 px 宽，原图进内存纯属浪费；
 * 2. 兜底路径：缩图/编码任一步失败（老 WebView、异常格式…），退回把原图直接转
 *    data URL；只要不超过后端 8 MB 上限就照用，不再报错。
 *
 * 失败不吞掉：两条路都走不通时抛出明确文案，由卡片设置抽屉显示。
 */

/** 卡片显示宽度最大约 400 px，2× 屏留余量。 */
const MAX_EDGE = 1024;
/** 缩图后 PNG 的预算；超过就改用 JPEG。 */
const PNG_BUDGET = 5 * 1024 * 1024;
/** 后端 `set_device_card_image` 的上限（8 MB），兜底路径也按它走。 */
export const MAX_DATA_URL_BYTES = 8 * 1024 * 1024;
/** 原始文件上限：超过这个尺寸连解码都不做，直接提示。 */
const MAX_SOURCE_BYTES = 32 * 1024 * 1024;
/** 解码/绘制的兜底超时：卡住时退回原图直转，不让按钮一直「正在读取…」。 */
const DECODE_TIMEOUT_MS = 6_000;

const IMAGE_EXT = /\.(png|jpe?g|webp|gif|bmp|svg)$/i;

/** MIME 可能为空或 octet-stream（拖拽/个别系统），这时才按扩展名认。 */
export function isImageFile(file: { type: string; name: string }): boolean {
  const type = file.type.trim().toLowerCase();
  if (type.startsWith('image/')) return true;
  if (type === '' || type === 'application/octet-stream') return IMAGE_EXT.test(file.name);
  return false;
}

function withTimeout<T>(promise: Promise<T>, ms: number, message: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error(message)), ms);
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

async function loadBitmap(
  file: File,
  timeoutMs: number,
): Promise<ImageBitmap | HTMLImageElement> {
  if (typeof createImageBitmap === 'function') {
    try {
      return await withTimeout(createImageBitmap(file), timeoutMs, '解码图片超时');
    } catch {
      // SVG 等格式 createImageBitmap 可能拒绝，落到 <img> 解码
    }
  }
  return withTimeout(
    new Promise<ImageBitmap | HTMLImageElement>((resolve, reject) => {
      const url = URL.createObjectURL(file);
      const image = new Image();
      image.onload = () => {
        URL.revokeObjectURL(url);
        resolve(image);
      };
      image.onerror = () => {
        URL.revokeObjectURL(url);
        reject(new Error('无法解码这张图片'));
      };
      image.src = url;
    }),
    timeoutMs,
    '解码图片超时',
  );
}

function readAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error('读取文件失败'));
    reader.readAsDataURL(file);
  });
}

/** 原图直接转 data URL；只在缩图链路失败时兜底。 */
async function fallbackRaw(file: File, timeoutMs: number): Promise<string> {
  const dataUrl = await withTimeout(readAsDataUrl(file), timeoutMs, '读取文件超时');
  if (dataUrl.length > MAX_DATA_URL_BYTES) {
    throw new Error('图片过大且无法压缩，请换一张小一点的（约 6 MB 以内）');
  }
  return dataUrl;
}

/** 等比缩图 + 编码；失败返回 null，由调用方兜底。 */
async function shrink(file: File, timeoutMs: number): Promise<string | null> {
  try {
    const source = await loadBitmap(file, timeoutMs);
    const width = source.width;
    const height = source.height;
    if (!width || !height) return null;

    const ratio = Math.min(1, MAX_EDGE / Math.max(width, height));
    const targetWidth = Math.max(1, Math.round(width * ratio));
    const targetHeight = Math.max(1, Math.round(height * ratio));

    const canvas = document.createElement('canvas');
    canvas.width = targetWidth;
    canvas.height = targetHeight;
    const context = canvas.getContext('2d');
    if (!context) return null;
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = 'high';
    context.drawImage(source, 0, 0, targetWidth, targetHeight);
    if ('close' in source) source.close();

    const png = canvas.toDataURL('image/png');
    return png.length <= PNG_BUDGET ? png : canvas.toDataURL('image/jpeg', 0.92);
  } catch {
    return null;
  }
}

/** `prepareCardImage` 的可选项；测试里可把超时调短。 */
interface PrepareCardImageOptions {
  timeoutMs?: number;
}

/** 文件 → 可直接落盘的 `data:image/...;base64,...`（优先缩图，失败回退原图）。 */
export async function prepareCardImage(
  file: File,
  options: PrepareCardImageOptions = {},
): Promise<string> {
  if (!isImageFile(file)) {
    throw new Error('只支持图片文件');
  }
  if (file.size > MAX_SOURCE_BYTES) {
    throw new Error('图片过大，请换一张不超过 32 MB 的');
  }

  const timeoutMs = options.timeoutMs ?? DECODE_TIMEOUT_MS;
  const shrunk = await shrink(file, timeoutMs);
  if (shrunk) return shrunk;
  return fallbackRaw(file, timeoutMs);
}
