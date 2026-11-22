/**
 * IPC 唯一出口：全前端只有本文件可以 `import { invoke }`。
 *
 * 命令名与返回结构是 P6 冻结契约（见 `V16/docs/V17-REWRITE-MAP.md` §5）：
 * 前端只调用、不改签名；确实要改时必须回 Rust 改命令并同步这里。
 */

import { invoke } from '@tauri-apps/api/core'
import type { AppSettings, ConfigSnapshot } from '../types/config'
import type { DeviceConfig, DeviceStatus } from '../types/device'
import type { DetectResult, DiscoveredDevice, ProfileChoice } from '../types/discover'
import type { AlertPayload } from '../types/alert'

/** 预览/定位窗口的展示方式：`show` 展示后自动收回，`hold` 常驻，`hide` 立即隐藏。 */
export type PreviewMode = 'show' | 'hold' | 'hide'

/** 全局刷新：菜单/预览卡请求 Rust 采集；主窗只根据开始/结束事件播放动效。 */
export const REFRESH_REQUEST_EVENT = 'refresh-request'
export const REFRESH_STARTED_EVENT = 'refresh-started'
export const REFRESH_DONE_EVENT = 'refresh-done'

/** 当前调用窗口已完成初始化及事件订阅，Rust 此后才显示并派发首次打开事件。 */
export function windowReady(): Promise<void> {
  return invoke<void>('window_ready')
}

/** 当前托盘浮层复用窗口显示的逻辑内容。 */
export function getTrayOverlayMode(): Promise<'popup' | 'tooltip'> {
  return invoke<'popup' | 'tooltip'>('get_tray_overlay_mode')
}

/** 自定义壁纸变更后广播；各窗口收到后重新拉取并换上。 */
export const BACKGROUND_CHANGED_EVENT = 'background-changed'

/** 卡片图片落盘/清除后广播设备 id；主窗与设置窗据此重读图片。 */
export const CARD_IMAGE_CHANGED_EVENT = 'card-image-changed'

/** 测试通知的示例数据；不传 = 用内置示例。 */
export interface AlertTestPayload {
  device?: string
  battery?: number
}

/** 整份配置：设置 + 设备列表。 */
export function getConfig(): Promise<ConfigSnapshot> {
  return invoke<ConfigSnapshot>('get_config')
}

/** 写单个设置键（Rust 负责归一化与落盘）。 */
export function setSetting(key: string, value: unknown): Promise<void> {
  return invoke<void>('set_setting', { key, value })
}

/** 批量写设置（设置页一次提交）。 */
export function setSettings(patch: Partial<AppSettings>): Promise<void> {
  return invoke<void>('set_settings', { patch })
}

/** 当前设备状态（读缓存，不触发采集）。 */
export function getDevices(): Promise<DeviceStatus[]> {
  return invoke<DeviceStatus[]>('get_devices')
}

/** 立即采集一次，返回新状态。 */
export function refreshNow(): Promise<DeviceStatus[]> {
  return invoke<DeviceStatus[]>('refresh_now')
}

/** 追加一台设备（Rust 归一化并落盘，返回补齐后的设备）。 */
export function addDevice(device: Partial<DeviceConfig>): Promise<DeviceConfig> {
  return invoke<DeviceConfig>('add_device', { device })
}

/** 修改某台设备的字段。 */
export function updateDevice(id: string, fields: Partial<DeviceConfig>): Promise<void> {
  return invoke<void>('update_device', { id, fields })
}

/** 按 id 顺序重排设备（拖拽排序）。 */
export function reorderDevices(ids: string[]): Promise<void> {
  return invoke<void>('reorder_devices', { ids })
}

/** 删除某台设备（同时清掉 last_levels）。 */
export function removeDevice(id: string): Promise<void> {
  return invoke<void>('remove_device', { id })
}

/** 扫描 HID / 蓝牙音频候选设备。 */
export function scanDevices(): Promise<DiscoveredDevice[]> {
  return invoke<DiscoveredDevice[]>('scan_devices')
}

/** 可用协议列表（含一个空的“不指定”）。 */
export function getProfileChoices(): Promise<ProfileChoice[]> {
  return invoke<ProfileChoice[]>('get_profile_choices')
}

/** 用指定协议试读一台候选设备。 */
export function detectDevice(
  device: Partial<DeviceConfig>,
  profileId = '',
): Promise<DetectResult> {
  return invoke<DetectResult>('detect_device', { device, profileId })
}

/** 保存扫描到的设备（Rust 归一化并落盘）。 */
export function addDiscoveredDevice(device: DiscoveredDevice): Promise<DeviceConfig> {
  return invoke<DeviceConfig>('add_discovered_device', { device })
}

/** 可用提示音文件名列表（`sound/*.wav`）。 */
export function getSoundFiles(): Promise<string[]> {
  return invoke<string[]>('get_sound_files')
}

/** 播放一次示例提醒（不改电量状态）。 */
export function testAlert(payload?: AlertTestPayload): Promise<void> {
  return invoke<void>('test_alert', { payload: payload ?? null })
}

/** 试一次托盘红色闪电闪烁。 */
export function testTrayBlink(): Promise<void> {
  return invoke<void>('test_tray_blink')
}

/** 懒建提醒窗挂载后拉取当前提醒，避免错过 show 时的事件。 */
export function getCurrentAlert(): Promise<AlertPayload | null> {
  return invoke<AlertPayload | null>('get_current_alert')
}

/** 打开配置目录。 */
export function openConfigDir(): Promise<void> {
  return invoke<void>('open_config_dir')
}

export const BACKGROUND_PREVIEW_EVENT = 'background-preview'

/** 背景可调参数 + 当前图片（与 Rust `BackgroundState` 对齐）。 */
export interface BackgroundState {
  image: string | null
  ratio: number
  dx: number
  dy: number
  blur: number
  dim: number
}

/** 当前生效背景：预览草稿优先，否则是已保存值。 */
export function getBackgroundState(): Promise<BackgroundState> {
  return invoke<BackgroundState>('get_background_state')
}

/** 预览：只进内存草稿并广播，不落盘。 */
export function previewBackground(background: BackgroundState): Promise<void> {
  return invoke<void>('preview_background', { background })
}

/** 保存背景图片与参数。 */
export function commitBackground(background: BackgroundState): Promise<void> {
  return invoke<void>('commit_background', { background })
}

/** 丢弃预览草稿，回到已保存效果。 */
export function cancelBackgroundPreview(): Promise<void> {
  return invoke<void>('cancel_background_preview')
}

export function showBackgroundEditor(): Promise<void> {
  return invoke<void>('show_background_editor')
}

export function hideBackgroundEditor(): Promise<void> {
  return invoke<void>('hide_background_editor')
}

/** 保存某台设备的实物卡片图片（`data:image/...` 整段 data URL）。 */
export function setDeviceCardImage(id: string, dataUrl: string): Promise<void> {
  return invoke<void>('set_device_card_image', { id, data: dataUrl })
}

/**
 * 从磁盘路径装入卡片图片（绕开 WebView 的 File API / canvas）。
 * 原图 ≤ 5 MB；更大请走 `setDeviceCardImage` + 前端缩图。
 */
export function setDeviceCardImageFromPath(id: string, path: string): Promise<void> {
  return invoke<void>('set_device_card_image_from_path', { id, path })
}

/**
 * 弹系统「打开文件」对话框选图，返回完整路径；用户取消返回 null。
 * WebView 的 `<input type="file">` 在无边框窗口里可能把对话框开到主窗后面，所以走 Win32。
 */
export function pickCardImageFile(): Promise<string | null> {
  return invoke<string | null>('pick_card_image_file')
}

/** 读取某台设备的卡片图片；没配过返回 null。 */
export function getDeviceCardImage(id: string): Promise<string | null> {
  return invoke<string | null>('get_device_card_image', { id })
}

/** 清除某台设备的卡片图片（回到电量光环）。 */
export function clearDeviceCardImage(id: string): Promise<void> {
  return invoke<void>('clear_device_card_image', { id })
}

// ---------- 智能探测（B） ----------

/** 未命中协议的厂商 HID 接口。 */
export interface UnknownHid {
  id: string
  vid: string
  pid: string
  usage_page: number
  usage: number
  product: string
}

/** 智能探测结果；`ok` 时协议 JSON 已落盘到 device_profiles/。 */
interface ProbeResult {
  ok: boolean
  template_id: string
  level: number | null
  charging: boolean
  profile_id: string
  text: string
}

export function listUnknownHid(): Promise<UnknownHid[]> {
  return invoke<UnknownHid[]>('list_unknown_hid')
}

export function probeUnknownHid(unknown: UnknownHid): Promise<ProbeResult> {
  return invoke<ProbeResult>('probe_unknown_hid', { unknown })
}

/** 退出应用。 */
export function quitApp(): Promise<void> {
  return invoke<void>('quit_app')
}

/** 预览小窗：show = 展示后等待点击卡外收起，hold = 常驻，hide = 立即隐藏。 */
export function previewPopup(mode: PreviewMode = 'show'): Promise<void> {
  return invoke<void>('preview_popup', { mode })
}

/** 预览悬停卡。 */
export function previewTooltip(mode: PreviewMode = 'show'): Promise<void> {
  return invoke<void>('preview_tooltip', { mode })
}

/** 预览提醒弹窗（静态展开帧）。 */
export function previewAlert(mode: PreviewMode = 'show'): Promise<void> {
  return invoke<void>('preview_alert', { mode })
}

/** 试听提示音（不弹提醒窗）。 */
export function previewSound(file: string): Promise<void> {
  return invoke<void>('preview_sound', { file })
}

/** 打开主窗并切到设置页（托盘菜单/快捷键入口）。 */
export function showSettings(): Promise<void> {
  return invoke<void>('show_settings')
}

/** 打开主窗并切到指定页（预览小窗/托盘菜单）。 */
export function showMain(page: 'summary' | 'devices' | 'settings' = 'summary'): Promise<void> {
  return invoke<void>('show_main', { page })
}

/** 在挂载首个页面前读取原生目标页，不创建或显示窗口。 */
export function getMainPage(): Promise<'summary' | 'devices' | 'settings'> {
  return invoke('get_main_page')
}

/** 仅记忆主窗当前页，供窗口销毁后恢复；不创建或显示窗口。 */
export function rememberMainPage(page: 'summary' | 'devices' | 'settings'): Promise<void> {
  return invoke<void>('remember_main_page', { page })
}

/** 打开添加设备窗口。 */
export function showAdd(): Promise<void> {
  return invoke<void>('show_add')
}

/** 打开卡片设置窗口并指定设备。 */
export function showCardInspector(deviceId: string): Promise<void> {
  return invoke<void>('show_card_inspector', { deviceId })
}

/** 收起卡片设置窗口。 */
export function hideCardInspector(): Promise<void> {
  return invoke<void>('hide_card_inspector')
}

/** 设置窗挂载/重开时兜底读取当前目标设备。 */
export function getCardInspectorTarget(): Promise<string | null> {
  return invoke<string | null>('get_card_inspector_target')
}

/** 卡片设置窗的实时预览：只更新主窗内存，不落盘；空 patch 清除。 */
export function previewCard(id: string, patch: Partial<DeviceConfig>): Promise<void> {
  return invoke<void>('preview_card', { id, patch })
}

/** 当前 config.json 的绝对路径（设置页展示）。 */
export function getConfigPath(): Promise<string> {
  return invoke<string>('get_config_path')
}

/** 恢复默认设置（保留设备列表）。 */
export function resetSettings(): Promise<void> {
  return invoke<void>('reset_settings')
}

/** 进入提醒弹窗定位模式。 */
export function startAlertPosition(): Promise<void> {
  return invoke<void>('start_alert_position')
}

/** 保存当前提醒弹窗位置。 */
export function saveAlertPosition(): Promise<void> {
  return invoke<void>('save_alert_position')
}

/** 恢复提醒弹窗默认位置。 */
export function resetAlertPosition(): Promise<void> {
  return invoke<void>('reset_alert_position')
}

/** 注册全局快捷键；返回是否成功（冲突时 false，旧键由 Rust 保留）。 */
export function registerHotkey(accelerator: string): Promise<boolean> {
  return invoke<boolean>('register_hotkey', { accelerator })
}

/** 注销全局快捷键。 */
export function unregisterHotkey(): Promise<void> {
  return invoke<void>('unregister_hotkey')
}

/** 把 IPC/未知异常转成可展示的一行文本。 */
export function errorMessage(error: unknown): string {
  if (error instanceof Error) return error.message
  if (typeof error === 'string') return error
  return String(error)
}
