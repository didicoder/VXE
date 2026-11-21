/**
 * 配置 store：设置 + 设备配置。所有写操作都走 `lib/ipc`，前端不自己落盘。
 *
 * setup store 返回全部 state；组件解构必须用 `storeToRefs`（否则丢响应性）。
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as ipc from '../lib/ipc'
import {
  defaultDevices,
  defaultSettings,
  type AppSettings,
  type ConfigSnapshot,
} from '../types/config'
import type { DeviceConfig } from '../types/device'

/** 把后端/脏配置合并进默认值：缺失键补默认、嵌套对象补齐、alert_pos 归一化。 */
function mergeSettings(raw: Partial<AppSettings> | null | undefined): AppSettings {
  const base = defaultSettings()
  if (!raw) return base
  const merged: AppSettings = { ...base, ...raw }
  merged.band_colors = { ...base.band_colors, ...(raw.band_colors ?? {}) }
  merged.last_levels = { ...(raw.last_levels ?? {}) }
  merged.alert_pos = normalizeAlertPos(raw.alert_pos)
  merged.ui_scale = finiteNumber(raw.ui_scale, base.ui_scale)
  merged.popup_scale = finiteNumber(raw.popup_scale, base.popup_scale)
  merged.tip_scale = finiteNumber(raw.tip_scale, base.tip_scale)
  merged.alert_scale = finiteNumber(raw.alert_scale, base.alert_scale)
  merged.tray_level_mode =
    raw.tray_level_mode === 'device' || raw.tray_level_mode === 'min'
      ? raw.tray_level_mode
      : 'average'
  merged.tray_level_device =
    typeof raw.tray_level_device === 'string' ? raw.tray_level_device : ''
  return merged
}

/** 数字或 trim 后的十进制浮点字符串，与 Rust f64::parse 的输入格式保持一致。 */
function finiteNumber(value: unknown, fallback: number): number {
  if (typeof value === 'number') return Number.isFinite(value) ? value : fallback
  if (typeof value !== 'string') return fallback
  const trimmed = value.trim()
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/.test(trimmed)) return fallback
  const parsed = Number(trimmed)
  return Number.isFinite(parsed) ? parsed : fallback
}

/** `alert_pos` 只认两个有限数字，坐标取整（V16 `Store.alert_pos` 同语义）。 */
function normalizeAlertPos(value: unknown): [number, number] | null {
  if (!Array.isArray(value) || value.length !== 2) return null
  const [x, y] = value
  if (typeof x !== 'number' || typeof y !== 'number') return null
  if (!Number.isFinite(x) || !Number.isFinite(y)) return null
  return [Math.trunc(x), Math.trunc(y)]
}

function cloneDevice(device: DeviceConfig): DeviceConfig {
  return {
    ...device,
    colors: { ...(device.colors ?? {}) },
    card_fields: device.card_fields ? { ...device.card_fields } : undefined,
  }
}

export const useConfigStore = defineStore('config', () => {
  const settings = ref<AppSettings>(defaultSettings())
  const devices = ref<DeviceConfig[]>(defaultDevices())
  const loading = ref(false)
  const loaded = ref(false)
  const error = ref<string | null>(null)
  let unlisten: UnlistenFn | null = null

  /** 订阅 Rust 配置变更；主题/缩放/设备 CRUD 后所有窗口同步。 */
  async function startEvents(): Promise<void> {
    if (unlisten) return
    try {
      unlisten = await listen('config-changed', () => {
        void load()
      })
    } catch {
      // 浏览器预览/单测没有 Tauri 事件通道，忽略即可。
    }
  }

  function stopEvents(): void {
    unlisten?.()
    unlisten = null
  }

  function applySnapshot(snapshot: ConfigSnapshot | null | undefined): void {
    settings.value = mergeSettings(snapshot?.settings)
    devices.value = Array.isArray(snapshot?.devices)
      ? snapshot.devices.map(cloneDevice)
      : []
  }

  /** 本地合并一次写入结果（嵌套结构按默认值归一化）。 */
  function applyLocal(patch: Partial<AppSettings>): void {
    settings.value = mergeSettings({ ...settings.value, ...patch })
  }

  /**
   * 读整份配置。失败时保留当前值（首次失败即默认值），错误写进 `error`。
   */
  async function load(): Promise<void> {
    loading.value = true
    error.value = null
    try {
      applySnapshot(await ipc.getConfig())
      loaded.value = true
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    } finally {
      loading.value = false
    }
  }

  /** 写单个键；成功后本地同步（失败不回滚、由 `error` 暴露）。 */
  async function setSetting(key: string, value: unknown): Promise<void> {
    error.value = null
    try {
      await ipc.setSetting(key, value)
      const next = { ...settings.value } as Record<string, unknown>
      next[key] = value
      applyLocal(next as Partial<AppSettings>)
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    }
  }

  /** 批量写设置（设置页一次提交）。 */
  async function setSettings(patch: Partial<AppSettings>): Promise<void> {
    error.value = null
    try {
      await ipc.setSettings(patch)
      applyLocal(patch)
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    }
  }

  /** 追加设备；成功后重新读整份配置，失败返回 null。 */
  async function addDevice(device: Partial<DeviceConfig>): Promise<DeviceConfig | null> {
    error.value = null
    try {
      const added = await ipc.addDevice(device)
      await load()
      return added
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return null
    }
  }

  /** 改设备字段；成功后重新读整份配置。 */
  async function updateDevice(id: string, fields: Partial<DeviceConfig>): Promise<boolean> {
    error.value = null
    try {
      await ipc.updateDevice(id, fields)
      await load()
      return true
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return false
    }
  }

  /** 删除设备；成功后重新读整份配置。 */
  async function removeDevice(id: string): Promise<boolean> {
    error.value = null
    try {
      await ipc.removeDevice(id)
      await load()
      return true
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return false
    }
  }

  /** 拖拽中：本地即时移动一项（from→to），不落盘。 */
  function moveLocal(from: number, to: number): void {
    const list = devices.value
    if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return
    const [item] = list.splice(from, 1)
    if (item) list.splice(to, 0, item)
  }

  /** 拖拽排序提交：按 id 顺序持久化。 */
  async function reorderDevices(ids: string[]): Promise<boolean> {
    error.value = null
    try {
      await ipc.reorderDevices(ids)
      return true
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return false
    }
  }

  return {
    settings,
    devices,
    loading,
    loaded,
    error,
    load,
    setSetting,
    setSettings,
    addDevice,
    updateDevice,
    removeDevice,
    moveLocal,
    reorderDevices,
    startEvents,
    stopEvents,
  }
})
