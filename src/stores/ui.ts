/**
 * 界面 store：页面路由、主题、主窗可见性、主窗缩放（只读展示），
 * 以及应用级命令（预览/设置窗/配置目录/退出/快捷键）的透传。
 *
 * 组件不直接 `invoke`，窗口与生命周期动作统一从这里走。
 *
 * 主题的唯一真值是配置里的 `ui_theme`（Rust 负责 system/schedule 调度后写回）；
 * 这里只读它并给出 class 名。点标题栏太阳/月亮 = 手动切主题，同时退出自动模式
 * （V16 `app.toggle_theme` 同语义）。
 */

import { defineStore } from 'pinia'
import { computed, ref } from 'vue'
import { windowScale } from '../composables/useWindowScale'
import * as ipc from '../lib/ipc'
import type { PreviewMode } from '../lib/ipc'
import type { UiTheme } from '../types/config'
import { useConfigStore } from './config'

/** 主窗三页（侧栏导航）。 */
export type UiPage = 'summary' | 'devices' | 'settings'

export const useUiStore = defineStore('ui', () => {
  const page = ref<UiPage>('summary')
  const mainVisible = ref(false)

  const config = useConfigStore()

  /** 生效中的主题：只有 `ui_theme === 'light'` 算亮色（V16 大小写敏感）。 */
  const theme = computed<UiTheme>(() =>
    config.settings.ui_theme === 'light' ? 'light' : 'dark',
  )

  /** `<html>` 上挂的 class。 */
  const themeClass = computed(() => (theme.value === 'light' ? 'theme-light' : 'theme-dark'))

  /** 主窗设计尺寸即 1×，只乘 ui_scale；base 仅兼容旧配置/迁移（只读展示）。 */
  const scale = computed(() => windowScale(config.settings, 'ui'))

  /** 手动切主题：写 `ui_theme` 并把 `theme_mode` 拉回 manual。 */
  async function setTheme(next: UiTheme): Promise<void> {
    const value: UiTheme = next === 'light' ? 'light' : 'dark'
    if (value === theme.value && config.settings.theme_mode === 'manual') return
    await config.setSettings({ ui_theme: value, theme_mode: 'manual' })
  }

  /** 显示主窗并切页。 */
  function showMain(next: UiPage = 'summary'): void {
    page.value = next
    mainVisible.value = true
  }

  /** 隐藏主窗（窗口真正隐藏由 Rust 接管）。 */
  function closeMain(): void {
    mainVisible.value = false
  }

  // ---------- 应用级命令（Rust 侧窗口/生命周期）----------

  /** 预览小窗：show = 自动收回，hold = 常驻，hide = 立即隐藏。 */
  async function previewPopup(mode: PreviewMode = 'show'): Promise<void> {
    await ipc.previewPopup(mode)
  }

  /** 预览悬停卡。 */
  async function previewTooltip(mode: PreviewMode = 'show'): Promise<void> {
    await ipc.previewTooltip(mode)
  }

  /** 预览提醒弹窗（静态展开帧）。 */
  async function previewAlert(mode: PreviewMode = 'show'): Promise<void> {
    await ipc.previewAlert(mode)
  }

  /** 让 Rust 打开主窗并切到设置页（托盘/快捷键入口用）。 */
  async function showSettings(): Promise<void> {
    await ipc.showSettings()
  }

  /** 打开配置文件所在目录。 */
  async function openConfigDir(): Promise<void> {
    await ipc.openConfigDir()
  }

  /** 退出应用。 */
  async function quitApp(): Promise<void> {
    await ipc.quitApp()
  }

  /** 注册全局快捷键；冲突时返回 false 且旧键由 Rust 保留。 */
  async function registerHotkey(accelerator: string): Promise<boolean> {
    return ipc.registerHotkey(accelerator)
  }

  /** 注销全局快捷键。 */
  async function unregisterHotkey(): Promise<void> {
    await ipc.unregisterHotkey()
  }

  return {
    page,
    mainVisible,
    theme,
    themeClass,
    scale,
    setTheme,
    showMain,
    closeMain,
    previewPopup,
    previewTooltip,
    previewAlert,
    showSettings,
    openConfigDir,
    quitApp,
    registerHotkey,
    unregisterHotkey,
  }
})
