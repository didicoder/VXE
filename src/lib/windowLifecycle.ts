import { nextTick } from 'vue'
import * as ipc from './ipc'

// 一个 WebView 只挂载一个窗口；壁纸和子组件的异步初始化也参加就绪握手。
const pending = new Set<Promise<unknown>>()

export function trackWindowInitialization<T>(task: Promise<T>): Promise<T> {
  pending.add(task)
  void task.then(() => pending.delete(task), () => pending.delete(task))
  return task
}

export async function initializeWindow(initialize: () => Promise<void>): Promise<void> {
  try {
    await initialize()
  } catch (cause) {
    console.error('窗口初始化失败', cause)
  } finally {
    const results = await Promise.allSettled([...pending])
    for (const result of results) {
      if (result.status === 'rejected') console.error('窗口资源初始化失败', result.reason)
    }
    await nextTick()
    try {
      await ipc.windowReady()
    } catch (cause) {
      console.error('窗口就绪通知失败', cause)
    }
  }
}
