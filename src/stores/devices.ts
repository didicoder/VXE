/**
 * 设备状态 store：只展示 Rust 给的结果，不在前端重算阈值/迟滞。
 *
 * setup store 返回全部 state；组件解构必须用 `storeToRefs`。
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import * as ipc from '../lib/ipc'
import type { DeviceStatus } from '../types/device'

/** 采集间隔兜底：设置页给的秒数异常时退回 60 秒。 */
const FALLBACK_INTERVAL_SECONDS = 60

export const useDevicesStore = defineStore('devices', () => {
  const statuses = ref<DeviceStatus[]>([])
  const refreshing = ref(false)
  const lastRefresh = ref<number | null>(null)
  const error = ref<string | null>(null)

  let timer: ReturnType<typeof setInterval> | null = null
  let unlisten: UnlistenFn | null = null

  /** 订阅 Rust 采集推送；所有窗口共享同一份状态，避免浮层各刷各的。 */
  async function startEvents(): Promise<void> {
    if (unlisten) return
    try {
      unlisten = await listen<DeviceStatus[]>('devices-updated', (event) => {
        statuses.value = Array.isArray(event.payload) ? event.payload : []
        lastRefresh.value = Date.now()
      })
    } catch {
      // 浏览器预览/单测没有 Tauri 事件通道，忽略即可。
    }
  }

  function stopEvents(): void {
    unlisten?.()
    unlisten = null
  }

  /** 停掉轮询（重复调用安全）。 */
  function stopPolling(): void {
    if (timer !== null) {
      clearInterval(timer)
      timer = null
    }
  }

  /** 立即刷新一次；正在刷新时忽略（避免采集重入）。 */
  async function refreshNow(): Promise<void> {
    if (refreshing.value) return
    refreshing.value = true
    error.value = null
    try {
      const next = await ipc.refreshNow()
      statuses.value = Array.isArray(next) ? next : []
      lastRefresh.value = Date.now()
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    } finally {
      refreshing.value = false
    }
  }

  /**
   * 读后端缓存（`get_devices`），不触发采集。
   * 预览/悬停窗首次打开时用，避免每个浮层都去读一次 HID。
   */
  async function loadCached(): Promise<void> {
    error.value = null
    try {
      const next = await ipc.getDevices()
      statuses.value = Array.isArray(next) ? next : []
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    }
  }

  /**
   * 启动轮询：立刻刷一次，之后每 `interval` 秒刷一次（秒，最小 1）。
   * 重复启动会先停掉旧定时器。返回停止函数。
   */
  function startPolling(interval: number): () => void {
    stopPolling()
    const seconds = Number.isFinite(interval) ? Math.max(1, interval) : FALLBACK_INTERVAL_SECONDS
    void refreshNow()
    timer = setInterval(() => {
      void refreshNow()
    }, seconds * 1000)
    return stopPolling
  }

  /** 拖拽中：本地即时移动一项（from→to），不落盘；下轮刷新按后端顺序对齐。 */
  function moveLocal(from: number, to: number): void {
    const list = statuses.value
    if (from === to || from < 0 || to < 0 || from >= list.length || to >= list.length) return
    const [item] = list.splice(from, 1)
    if (item) list.splice(to, 0, item)
  }

  return {
    statuses,
    refreshing,
    lastRefresh,
    error,
    refreshNow,
    moveLocal,
    loadCached,
    startPolling,
    stopPolling,
    startEvents,
    stopEvents,
  }
})
