export type RightAngle = 90 | 180 | 270;

export function rightAngle(deg: number): RightAngle | 0 {
  const rounded = Math.round(deg / 90) * 90;
  if (Math.abs(deg - rounded) > 0.01) return 0;
  const angle = ((rounded % 360) + 360) % 360;
  return angle === 90 || angle === 180 || angle === 270 ? angle : 0;
}

interface RotatedImage {
  url: string;
  pixels: number;
}

export interface RotationLease {
  ready: Promise<string>;
  release(): void;
}

function rotate(src: string, angle: RightAngle, signal: AbortSignal): Promise<RotatedImage> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    let canvas: HTMLCanvasElement | null = null;
    const clearCanvas = () => {
      if (canvas) canvas.width = canvas.height = 0;
    };
    const cleanup = () => {
      image.onload = null;
      image.onerror = null;
      signal.removeEventListener('abort', abort);
    };
    const abort = () => {
      cleanup();
      clearCanvas();
      image.src = '';
      reject(new Error('旋转请求已释放'));
    };
    signal.addEventListener('abort', abort, { once: true });
    image.onerror = () => {
      cleanup();
      reject(new Error('旋转图片加载失败'));
    };
    image.onload = () => {
      image.onload = null;
      image.onerror = null;
      const sw = image.naturalWidth;
      const sh = image.naturalHeight;
      canvas = document.createElement('canvas');
      try {
        if (!sw || !sh) throw new Error('旋转图片尺寸无效');
        const quarter = angle === 90 || angle === 270;
        canvas.width = quarter ? sh : sw;
        canvas.height = quarter ? sw : sh;
        const ctx = canvas.getContext('2d');
        if (!ctx) throw new Error('无法创建旋转画布');
        ctx.imageSmoothingEnabled = true;
        ctx.imageSmoothingQuality = 'high';
        ctx.translate(canvas.width / 2, canvas.height / 2);
        ctx.rotate((angle * Math.PI) / 180);
        ctx.drawImage(image, -sw / 2, -sh / 2);
        canvas.toBlob((blob) => {
          cleanup();
          clearCanvas();
          if (signal.aborted || !blob) {
            reject(new Error('旋转请求已释放或编码失败'));
            return;
          }
          resolve({ url: URL.createObjectURL(blob), pixels: sw * sh });
        }, 'image/png');
      } catch (cause) {
        cleanup();
        clearCanvas();
        reject(cause);
      }
    };
    image.src = src;
  });
}

/** 原始字符串只作源身份，不再拼成含整张 base64 的派生键。闲置结果跨切页复用。 */
export function createRotatedCardImageCache(maxEntries = 24, maxPixels = 16 * 1024 * 1024) {
  interface Entry {
    source: string;
    angle: RightAngle;
    refs: number;
    controller: AbortController;
    ready: Promise<string>;
    result?: RotatedImage;
  }
  const sources = new Map<string, Map<RightAngle, Entry>>();
  const lru = new Set<Entry>();
  let pixels = 0;

  function remove(entry: Entry): void {
    lru.delete(entry);
    const angles = sources.get(entry.source);
    if (angles?.get(entry.angle) === entry) {
      angles.delete(entry.angle);
      if (angles.size === 0) sources.delete(entry.source);
    }
    entry.controller.abort();
    if (entry.result) {
      pixels -= entry.result.pixels;
      URL.revokeObjectURL(entry.result.url);
      entry.result = undefined;
    }
  }

  function trim(): void {
    for (const entry of [...lru]) {
      if (lru.size <= maxEntries && pixels <= maxPixels) break;
      // 在用 URL 必须保活；最后一个租约归还时再收敛到预算。
      if (entry.refs === 0) remove(entry);
    }
  }

  function acquire(source: string, angle: RightAngle): RotationLease {
    let angles = sources.get(source);
    if (!angles) {
      angles = new Map();
      sources.set(source, angles);
    }
    let entry = angles.get(angle);
    if (!entry || entry.controller.signal.aborted) {
      entry = {
        source, angle, refs: 0, controller: new AbortController(),
        ready: Promise.resolve(''),
      };
      const created = entry;
      angles.set(angle, created);
      lru.add(created);
      created.ready = rotate(source, angle, created.controller.signal).then((result) => {
        if (created.controller.signal.aborted || sources.get(source)?.get(angle) !== created) {
          URL.revokeObjectURL(result.url);
          throw new Error('旋转结果已过期');
        }
        created.result = result;
        pixels += result.pixels;
        trim();
        return result.url;
      }, (cause) => {
        remove(created);
        throw cause;
      });
    }
    entry.refs += 1;
    lru.delete(entry);
    lru.add(entry);
    trim();
    const leased = entry;
    let released = false;
    return {
      ready: leased.ready,
      release() {
        if (released) return;
        released = true;
        leased.refs -= 1;
        if (leased.refs === 0 && !leased.result) remove(leased);
        trim();
      },
    };
  }

  function clearIdle(): void {
    for (const entry of [...lru]) {
      if (entry.refs === 0) remove(entry);
    }
  }

  return { acquire, clearIdle };
}

export const rotatedCardImages = createRotatedCardImageCache();
if (typeof window !== 'undefined') {
  window.addEventListener('pagehide', rotatedCardImages.clearIdle);
}
