/**
 * 自定义窗口壁纸：给每个 `.glass-window` 注入一个壁纸层。
 *
 * 整张图片按 "cover × 缩放" 排版，平移在整张图片内部进行；
 * 只有图片边缘真正进入窗口时才露出主题白/黑填充（--wallpaper-fill）。
 * 预览草稿与已保存值都从 `get_background_state` 拉取，
 * `background-changed` / `background-preview` 事件让各窗口重新排版。
 */

import { getCurrentInstance, onMounted, onUnmounted } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as ipc from '../lib/ipc'
import { wallpaperGeometry } from '../lib/wallpaperLayout'
import { trackWindowInitialization } from '../lib/windowLifecycle'
import wallpaperUrl from '../assets/wallpaper.jpg'

interface ImageSize {
  width: number
  height: number
}

const sizeCache = new Map<string, Promise<ImageSize>>()
const layoutGenerations = new WeakMap<HTMLElement, number>()
const hostsByOwner = new Map<object, Set<HTMLElement>>()
let current: ipc.BackgroundState | null = null
let observer: ResizeObserver | null = null
let pipelinePromise: Promise<void> | null = null
let pipelineGeneration = 0
let unlisten: UnlistenFn[] = []
let loadGeneration = 0

function imageSize(src: string): Promise<ImageSize> {
  const cached = sizeCache.get(src)
  if (cached) {
    sizeCache.delete(src)
    sizeCache.set(src, cached)
    return cached
  }
  const pending = new Promise<ImageSize>((resolve, reject) => {
    const image = new Image()
    image.onload = () => resolve({ width: image.naturalWidth, height: image.naturalHeight })
    image.onerror = () => reject(new Error('壁纸图片加载失败'))
    image.src = src
  })
  sizeCache.set(src, pending)
  if (sizeCache.size > 2) sizeCache.delete(sizeCache.keys().next().value!)
  pending.catch(() => {
    if (sizeCache.get(src) === pending) sizeCache.delete(src)
  })
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

function hostsForOwner(owner: object): HTMLElement[] {
  const instance = owner as { subTree?: { el?: unknown } }
  const root = instance.subTree?.el
  if (!(root instanceof HTMLElement)) return []
  const hosts: HTMLElement[] = []
  if (root.matches('.glass-window')) hosts.push(root)
  hosts.push(...Array.from(root.querySelectorAll<HTMLElement>('.glass-window')))
  return hosts
}

function isRegisteredHost(host: HTMLElement): boolean {
  for (const hosts of hostsByOwner.values()) {
    if (hosts.has(host)) return true
  }
  return false
}

function activeHosts(): HTMLElement[] {
  const hosts = new Set<HTMLElement>()
  for (const ownerHosts of hostsByOwner.values()) {
    for (const host of ownerHosts) hosts.add(host)
  }
  return Array.from(hosts)
}

function isPipelineActive(generation: number): boolean {
  return generation === pipelineGeneration && hostsByOwner.size > 0
}

async function layout(host: HTMLElement, generation: number): Promise<void> {
  const layoutGeneration = (layoutGenerations.get(host) ?? 0) + 1
  layoutGenerations.set(host, layoutGeneration)
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
  if (
    !isPipelineActive(generation)
    || layoutGenerations.get(host) !== layoutGeneration
    || !isRegisteredHost(host)
  ) return
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

function apply(state: ipc.BackgroundState | null, generation: number): void {
  if (!isPipelineActive(generation)) return
  current = state
  const root = document.documentElement
  root.classList.toggle('has-custom-wallpaper', !!state?.image)
  root.classList.toggle('no-wallpaper-blur', (state?.blur ?? 12) <= 0)
  root.style.setProperty('--bg-blur', `${state?.blur ?? 12}px`)
  root.style.setProperty('--bg-dim', String(state?.dim ?? 0))
  const hosts = activeHosts()
  if (!hosts.length) return
  root.classList.add('has-wallpaper-layer')
  for (const host of hosts) void layout(host, generation)
}

async function refresh(generation: number): Promise<void> {
  const request = ++loadGeneration
  let state: ipc.BackgroundState | null
  try {
    state = await ipc.getBackgroundState()
  } catch {
    state = null
  }
  if (!isPipelineActive(generation) || request !== loadGeneration) return
  apply(state, generation)
}

async function startPipeline(generation: number): Promise<void> {
  observer = new ResizeObserver((entries) => {
    for (const entry of entries) {
      const host = entry.target
      if (host instanceof HTMLElement && isPipelineActive(generation) && isRegisteredHost(host)) {
        void layout(host, generation)
      }
    }
  })
  for (const host of activeHosts()) observer.observe(host)

  const listeners: UnlistenFn[] = []
  try {
    listeners.push(await listen(ipc.BACKGROUND_CHANGED_EVENT, () => {
      void refresh(generation)
    }))
    if (!isPipelineActive(generation)) {
      for (const off of listeners) void off()
      return
    }
    listeners.push(await listen(ipc.BACKGROUND_PREVIEW_EVENT, () => {
      void refresh(generation)
    }))
    if (isPipelineActive(generation)) {
      unlisten = listeners
    } else {
      for (const off of listeners) void off()
      return
    }
  } catch {
    for (const off of listeners) void off()
  }
  if (!isPipelineActive(generation)) return
  await refresh(generation)
}

function ensurePipeline(): Promise<void> {
  if (pipelinePromise) return pipelinePromise
  const generation = ++pipelineGeneration
  pipelinePromise = startPipeline(generation)
  return pipelinePromise
}

function stopPipeline(): void {
  ++pipelineGeneration
  ++loadGeneration
  observer?.disconnect()
  observer = null
  for (const off of unlisten) void off()
  unlisten = []
  pipelinePromise = null
}

/** 挂载时注入壁纸层，并监听保存/预览/尺寸变化。 */
export function useWallpaper(): void {
  const owner = getCurrentInstance()
  if (!owner) return

  onMounted(() => {
    const hosts = new Set(hostsForOwner(owner))
    hostsByOwner.set(owner, hosts)
    if (observer) {
      for (const host of hosts) observer.observe(host)
    }
    void trackWindowInitialization(ensurePipeline())
  })

  onUnmounted(() => {
    const hosts = hostsByOwner.get(owner)
    if (hosts) {
      for (const host of hosts) observer?.unobserve(host)
      hostsByOwner.delete(owner)
    }
    if (!hostsByOwner.size) stopPipeline()
  })
}
