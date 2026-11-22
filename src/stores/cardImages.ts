/** 图片本体只驻留本窗口缓存，不进入 config.json。 */
import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as ipc from '../lib/ipc'
import type { DeviceConfig } from '../types/device'

export const useCardImagesStore = defineStore('cardImages', () => {
  const images = ref<Record<string, string>>({})
  const pendingCount = ref(0)
  const loading = computed(() => pendingCount.value > 0)
  const completed = new Set<string>()
  const versions = new Map<string, symbol>()
  const reads = new Map<string, Promise<void>>()
  let devicesById: Map<string, string> | null = null

  function invalidate(id: string): void {
    versions.set(id, Symbol())
    reads.delete(id)
    completed.delete(id)
  }

  /** 合法设备与当前页面需求分开；没有 card_img 标记的旧图片仍会按需探测。 */
  function syncDevices(devices: readonly DeviceConfig[]): void {
    const next = new Map(devices.map(d => [d.id, d.card_img ?? '']))
    const known = new Set([...versions.keys(), ...Object.keys(images.value)])
    for (const id of known) {
      if (!next.has(id)) {
        invalidate(id)
        versions.delete(id)
      }
    }
    if (devicesById) {
      for (const [id, marker] of next) {
        if (devicesById.has(id) && devicesById.get(id) !== marker) invalidate(id)
      }
    }
    devicesById = next
    images.value = Object.fromEntries(Object.entries(images.value).filter(([id]) => next.has(id)))
  }

  function readImage(id: string): Promise<void> {
    if (devicesById && !devicesById.has(id)) return Promise.resolve()
    if (completed.has(id)) return Promise.resolve()
    const pending = reads.get(id)
    if (pending) return pending
    const version = versions.get(id) ?? Symbol()
    versions.set(id, version)
    pendingCount.value += 1
    const promise = ipc.getDeviceCardImage(id).then(data => {
      if (versions.get(id) !== version) return
      completed.add(id) // 不存在也是完成态；失败则留给下次需求重试。
      const next = { ...images.value }
      if (data) next[id] = data
      else delete next[id]
      images.value = next
    }).catch(() => {
      // 临时读取失败保留刚保存的图片，并允许下次重试。
    }).finally(() => {
      pendingCount.value -= 1
      if (reads.get(id) === promise) reads.delete(id)
    })
    reads.set(id, promise)
    return promise
  }

  async function ensureImages(ids: readonly string[]): Promise<void> {
    await Promise.all([...new Set(ids)].map(readImage))
  }

  /** 兼容需要完整列表的调用；完成态不会重复读取。 */
  function loadAll(devices: readonly DeviceConfig[]): Promise<void> {
    syncDevices(devices)
    return ensureImages(devices.map(d => d.id))
  }

  function loadOne(id: string): Promise<void> {
    invalidate(id)
    return readImage(id)
  }

  function imageFor(id: string): string | null {
    return images.value[id] ?? null
  }

  async function save(id: string, dataUrl: string): Promise<void> {
    await ipc.setDeviceCardImage(id, dataUrl)
    invalidate(id)
    if (devicesById && !devicesById.has(id)) return
    completed.add(id)
    images.value = { ...images.value, [id]: dataUrl }
  }

  async function saveFromPath(id: string, path: string): Promise<void> {
    await ipc.setDeviceCardImageFromPath(id, path)
    await loadOne(id)
  }

  async function clear(id: string): Promise<void> {
    await ipc.clearDeviceCardImage(id)
    invalidate(id)
    completed.add(id)
    const next = { ...images.value }
    delete next[id]
    images.value = next
  }

  return { images, loading, syncDevices, ensureImages, invalidate, loadAll, loadOne, imageFor, save, saveFromPath, clear }
})
