/**
 * 实物卡片的每台设备自选图片（文件存 `card-images/{id}.dat`）。
 *
 * 图片本体不进 config.json：卡片组件先读这张表，没有自选图就画电量光环。
 * `set()` 同时把 `card_img: 'custom'` 写进设备配置，作为“已有自选图”的标记。
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as ipc from '../lib/ipc'
import type { DeviceConfig } from '../types/device'

export const useCardImagesStore = defineStore('cardImages', () => {
  const images = ref<Record<string, string>>({})
  const loading = ref(false)

  /**
   * 为整份设备列表补齐自选图；读失败的设备按「没有自选图」处理。
   *
   * 合并而不是整体替换：刚保存的图会先写进内存、再由 `updateDevice` 触发重读，
   * 如果这中间任何一次读取失败，整体替换会把用户刚换上的图抹掉（表现为「换图没反应」）。
   * 真正删除只走 `clear()`。
   */
  async function loadAll(devices: readonly DeviceConfig[]): Promise<void> {
    loading.value = true
    const entries = await Promise.all(
      devices.map(async (device) => {
        try {
          const data = await ipc.getDeviceCardImage(device.id)
          return data ? ([device.id, data] as const) : null
        } catch {
          return null
        }
      }),
    )
    const next: Record<string, string> = { ...images.value }
    for (const entry of entries) {
      if (entry) next[entry[0]] = entry[1]
    }
    images.value = next
    loading.value = false
  }

  /** 单台设备重读：来自其它窗口的换图/清图事件。 */
  async function loadOne(id: string): Promise<void> {
    try {
      const data = await ipc.getDeviceCardImage(id)
      const next = { ...images.value }
      if (data) next[id] = data
      else delete next[id]
      images.value = next
    } catch {
      // 读失败保留当前内存图，等下一次 loadAll 补齐
    }
  }

  function imageFor(id: string): string | null {
    return images.value[id] ?? null
  }

  async function save(id: string, dataUrl: string): Promise<void> {
    await ipc.setDeviceCardImage(id, dataUrl)
    images.value = { ...images.value, [id]: dataUrl }
  }

  /**
   * 从磁盘路径装入（前端不碰 File API / canvas，Rust 侧转 data URL）。
   * 成功后回读一次，让内存缓存与磁盘一致。
   */
  async function saveFromPath(id: string, path: string): Promise<void> {
    await ipc.setDeviceCardImageFromPath(id, path)
    try {
      const data = await ipc.getDeviceCardImage(id)
      if (data) images.value = { ...images.value, [id]: data }
    } catch {
      // 回读失败不致命：下次 loadAll 会从磁盘补上
    }
  }

  async function clear(id: string): Promise<void> {
    await ipc.clearDeviceCardImage(id)
    const next = { ...images.value }
    delete next[id]
    images.value = next
  }

  return { images, loading, loadAll, loadOne, imageFor, save, saveFromPath, clear }
})