/**
 * 主题 class：`theme-light` / `theme-dark`。
 *
 * 返回 computed 供模板用；同时在 `document.documentElement` 上同步 class
 * （挂载时写一次，主题变化时更新，卸载时两个都摘掉）。
 */

import { computed, onMounted, onUnmounted, watch, type ComputedRef } from 'vue'
import { useUiStore } from '../stores/ui'

export type ThemeClassName = 'theme-dark' | 'theme-light'

const THEME_CLASSES: ThemeClassName[] = ['theme-dark', 'theme-light']

function rootElement(): HTMLElement | null {
  return typeof document === 'undefined' ? null : document.documentElement
}

function applyThemeClass(className: ThemeClassName): void {
  const root = rootElement()
  if (!root) return
  root.classList.remove(...THEME_CLASSES)
  root.classList.add(className)
}

function clearThemeClass(): void {
  rootElement()?.classList.remove(...THEME_CLASSES)
}

/** 当前主题 class，并把它同步到 `<html>`。 */
export function useThemeClass(): ComputedRef<ThemeClassName> {
  const ui = useUiStore()
  const themeClass = computed<ThemeClassName>(() =>
    ui.theme === 'light' ? 'theme-light' : 'theme-dark',
  )

  onMounted(() => applyThemeClass(themeClass.value))
  watch(themeClass, (value) => applyThemeClass(value))
  onUnmounted(clearThemeClass)

  return themeClass
}
