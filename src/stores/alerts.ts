/**
 * 提醒 store：界面展示态 + 提示音文件 + 测试通知。
 *
 * 不要在这里复刻 Rust 的低电量迟滞/队列：`lowArmed` 只是 UI 提示位，
 * 真正的阈值、迟滞与提醒队列唯一真值在 Rust。`queue` 也只服务界面预览。
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import * as ipc from '../lib/ipc'
import type { AlertTestPayload } from '../lib/ipc'

/** 队列上限（与提醒窗展示口径一致）。 */
export const ALERT_QUEUE_LIMIT = 3

/** 界面预览的一条提醒。 */
interface AlertQueueItem {
  /** 设备名；空串 = 示例/通用。 */
  device: string
  battery: number | null
  /** 入队时间（epoch ms）。 */
  at: number
}

export const useAlertsStore = defineStore('alerts', () => {
  /** 仅 UI 提示位：Rust 认为低电量提醒已武装时置 true，不代表任何阈值逻辑。 */
  const lowArmed = ref(false)
  /** 界面预览队列（最多 3 条，同设备合并）。 */
  const queue = ref<AlertQueueItem[]>([])
  const soundFiles = ref<string[]>([])
  const loadingSounds = ref(false)
  const testing = ref(false)
  const error = ref<string | null>(null)

  /** 当前要展示的一条（队列尾）。 */
  const current = computed<AlertQueueItem | null>(
    () => queue.value[queue.value.length - 1] ?? null,
  )

  /** UI 侧设置提示位，不参与任何判定。 */
  function setLowArmed(value: boolean): void {
    lowArmed.value = value
  }

  /**
   * 把一条提醒放进界面预览队列：同设备合并、最多 3 条、新的在尾部。
   * 纯展示，不触发 Rust 弹窗（真实弹窗用 `testAlert`）。
   */
  function previewAlert(payload: AlertTestPayload = {}): AlertQueueItem {
    const item: AlertQueueItem = {
      device: payload.device ?? '',
      battery: payload.battery ?? null,
      at: Date.now(),
    }
    queue.value = [
      ...queue.value.filter((entry) => entry.device !== item.device),
      item,
    ].slice(-ALERT_QUEUE_LIMIT)
    return item
  }

  function clearQueue(): void {
    queue.value = []
  }

  /** 读提示音列表（`sound/*.wav`）。失败保留旧列表并写 `error`。 */
  async function loadSoundFiles(): Promise<void> {
    loadingSounds.value = true
    error.value = null
    try {
      const files = await ipc.getSoundFiles()
      soundFiles.value = Array.isArray(files) ? files : []
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    } finally {
      loadingSounds.value = false
    }
  }

  /** 播放一次示例提醒（走 Rust，不改电量状态）。 */
  async function testAlert(payload?: AlertTestPayload): Promise<void> {
    testing.value = true
    error.value = null
    try {
      await ipc.testAlert(payload)
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    } finally {
      testing.value = false
    }
  }

  /** 进入提醒弹窗定位模式（方向键移动、Enter 保存）。 */
  async function startAlertPosition(): Promise<void> {
    await ipc.startAlertPosition()
  }

  /** 保存当前提醒弹窗位置。 */
  async function saveAlertPosition(): Promise<void> {
    await ipc.saveAlertPosition()
  }

  /** 恢复提醒弹窗默认位置（工作区底部居中）。 */
  async function resetAlertPosition(): Promise<void> {
    await ipc.resetAlertPosition()
  }

  return {
    lowArmed,
    queue,
    current,
    soundFiles,
    loadingSounds,
    testing,
    error,
    setLowArmed,
    previewAlert,
    clearQueue,
    loadSoundFiles,
    testAlert,
    startAlertPosition,
    saveAlertPosition,
    resetAlertPosition,
  }
})
