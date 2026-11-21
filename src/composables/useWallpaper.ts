/**
 * 自定义窗口壁纸：给每个 `.glass-window` 注入一个壁纸层。
 *
 * 整张图片按 "cover × 缩放" 排版，平移在整张图片内部进行；
 * 只有图片边缘真正进入窗口时才露出主题白/黑填充（--wallpaper-fill）。
 * 预览草稿与已保存值都从 `get_background_state` 拉取，
 * `background-changed` / `background-preview` 事件让各窗口重新排版。
 */

import { onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as ipc from '../lib/ipc'
import { wallpaperGeometry } from '../lib/wallpaperLayout'
import wallpaperUrl from '../assets/wallpaper.jpg'

interface ImageSize {
  width: number
  height: number
}

const sizeCache = new Map<string, Promise<ImageSize>>()
let current: ipc.BackgroundState | null = null
let observer: ResizeObserver | null = null

function imageSize(src: string): Promise<ImageSize> {
  const cached = sizeCache.get(src)
  if (cached) return cached
  const pending = new Promise<ImageSize>((resolve, reject) => {
    const image = new Image()
    image.onload = () => resolve({ width: image.naturalWidth, height: image.naturalHeight })
    image.onerror = () => reject(new Error('壁纸图片加载失败'))
    image.src = src
  })
  sizeCache.set(src, pending)
  pending.catch(() => sizeCache.delete(src))
  return pending
}

function ensureLayer(host: HTMLElement): HTMLImageElement {
  const existing = host.querySelector<HTMLImageElement>(
    ':scope > .wallpaper-layer > .wallpaper-layer__image',
  )
  if (existing) return existing
  const layer = document.createElement('div')
  layer.className = 'wallpaper-layer'
  const image = document.createElement('img')
  image.className = 'wallpaper-layer__image'
  image.alt = ''
  image.draggable = false
  layer.appendChild(image)
  host.prepend(layer)
  return image
}

function clampOffset(value: number): number {
  return Math.min(50, Math.max(-50, value))
}

async function layout(host: HTMLElement): Promise<void> {
  const image = ensureLayer(host)
  const src = current?.image ?? wallpaperUrl
  if (image.getAttribute('src') !== src) image.src = src
  const boxWidth = host.clientWidth
  const boxHeight = host.clientHeight
  if (!boxWidth || !boxHeight) return
  let size: ImageSize
  try {
    size = await imageSize(src)
  } catch {
    return
  }
  const ratio = Math.min(3, Math.max(1, current?.ratio ?? 1))
  const dx = clampOffset(current?.dx ?? 0)
  const dy = clampOffset(current?.dy ?? 0)
  const geometry = wallpaperGeometry(
    boxWidth,
    boxHeight,
    size.width,
    size.height,
    ratio,
    dx,
    dy,
  )
  image.style.width = `${geometry.width}px`
  image.style.height = `${geometry.height}px`
  image.style.left = `${geometry.left}px`
  image.style.top = `${geometry.top}px`
}

function apply(state: ipc.BackgroundState | null): void {
  current = state
  const root = document.documentElement
  root.classList.toggle('has-custom-wallpaper', !!state?.image)
  root.classList.toggle('no-wallpaper-blur', (state?.blur ?? 12) <= 0)
  root.style.setProperty('--bg-blur', `${state?.blur ?? 12}px`)
  root.style.setProperty('--bg-dim', String(state?.dim ?? 0))
  const hosts = Array.from(document.querySelectorAll<HTMLElement>('.glass-window'))
  if (!hosts.length) return
  root.classList.add('has-wallpaper-layer')
  for (const host of hosts) void layout(host)
}

async function sync(): Promise<void> {
  try {
    apply(await ipc.getBackgroundState())
  } catch {
    apply(null)
  }
}

/** 挂载时注入壁纸层，并监听保存/预览/尺寸变化。 */
export function useWallpaper(): void {
  let unlisten: UnlistenFn[] = []

  onMounted(async () => {
    void sync()
    observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const host = entry.target
        if (host instanceof HTMLElement) void layout(host)
      }
    })
    for (const host of Array.from(document.querySelectorAll<HTMLElement>('.glass-window'))) {
      observer.observe(host)
    }
    try {
      unlisten = [
        await listen(ipc.BACKGROUND_CHANGED_EVENT, () => {
          void sync()
        }),
        await listen(ipc.BACKGROUND_PREVIEW_EVENT, () => {
          void sync()
        }),
      ]
    } catch {
      // 非 Tauri 环境没有事件总线
    }
  })

  onUnmounted(() => {
    observer?.disconnect()
    observer = null
    for (const off of unlisten) off()
    unlisten = []
  })
}