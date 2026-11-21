/**
 * 各窗口设计尺寸即 1×，CSS 当前只乘用户 scale（保留 V16 `store` 的夹取语义）。
 * base 仅用于旧配置兼容/迁移，不参与当前窗口尺寸。
 *
 * 主窗只吃 `ui_scale`；预览/悬停/提醒各有独立倍率。
 * 返回值是纯数字，写 CSS 变量时再乘设计像素。
 */

import { computed, type ComputedRef } from 'vue'
import { clamp } from '../lib/geometry'
import { useConfigStore } from '../stores/config'
import type { AppSettings } from '../types/config'

/** 各窗口类型。 */
type WindowScaleKind = 'ui' | 'popup' | 'tip' | 'alert' | 'add' | 'inspector' | 'bg'

/** 脏值兜底（设计尺寸即 1×）。 */
const FALLBACK = {
  scale: 1.0,
}

function finite(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback
}

/**
 * 无响应性的纯计算，便于单测与 store 复用。
 *
 * 设计尺寸就是 1×；只有主窗/预览/悬停/提醒有用户缩放。
 * 添加窗、卡片设置、背景编辑器固定 1×；旧 base 字段只用于兼容/迁移。
 */
export function windowScale(settings: AppSettings, kind: WindowScaleKind): number {
  switch (kind) {
    case 'ui': {
      const user = finite(settings.ui_scale, FALLBACK.scale)
      return user >= 0.5 && user <= 2.0 ? user : FALLBACK.scale
    }
    case 'popup':
      return clamp(finite(settings.popup_scale, FALLBACK.scale), 0.5, 2.0)
    case 'tip':
      return clamp(finite(settings.tip_scale, FALLBACK.scale), 0.5, 2.0)
    case 'alert':
      return clamp(finite(settings.alert_scale, FALLBACK.scale), 0.5, 2.0)
    case 'add':
    case 'inspector':
    case 'bg':
      return 1
  }
}

/** 响应式版本：配置变化时自动重算。 */
export function useWindowScale(kind: WindowScaleKind): ComputedRef<number> {
  const config = useConfigStore()
  return computed(() => windowScale(config.settings, kind))
}
