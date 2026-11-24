/**
 * 配置形状：`AppSettings` 逐键对齐 V16 `store.DEFAULTS` / `config.json`，
 * 加上 E1/E3 的 `hotkey_enabled` / `hotkey`。
 *
 * `get_config()` 返回 `{ settings, devices }`；未列出的未知键由 Rust 原样保留，
 * 前端不关心，因此 `AppSettings` 只声明 UI 真正读取的键。
 */

import type { BandColors } from './battery'
import type { DeviceConfig } from './device'

/** 电量指示样式（V16 只有 `ring` / `dot` 精确匹配，其余都按 bar 渲染）。 */
export type IndicatorStyle = 'ring' | 'bar' | 'dot'

/** 生效中的明暗主题（`ui_theme`）。V16 大小写敏感，只有 `light` 算亮色。 */
export type UiTheme = 'dark' | 'light'

/** 主题来源（`theme_mode`）。 */
export type ThemeMode = 'manual' | 'system' | 'schedule'

/** 主窗界面风格（`ui_style`）：实物照片卡片 / 3.0 玻璃卡片。 */
export type UiStyle = 'photo' | 'classic'

/** 实物卡片每行张数（`photo_cols`）。 */
type PhotoColumns = 2 | 3

/** 设备显示档位（`display_*`）：`all` = 含未连接，`connected` = 仅已连接。 */
export type DeviceDisplay = 'all' | 'connected'

/** 低电量提醒方式（`alert_mode`）。 */
type AlertMode = 'popup' | 'tray' | 'both'

/** 托盘闪电图标的电量来源（`tray_level_mode`）。 */
export type TrayLevelMode = 'average' | 'device' | 'min'

/** 托盘图标样式（`tray_icon_style`）。 */
export type TrayIconStyle = 'bolt' | 'ring' | 'battery'

/** 提醒弹窗自定义位置：物理像素中心点，null = 默认底部居中。 */
type AlertPosition = [number, number] | null

/** 用户设置（`settings` 段）。 */
export interface AppSettings {
  autostart: boolean
  refresh_seconds: number
  hover_delay: number
  good_level: number
  warn_level: number
  band_colors: BandColors
  ui_base: number
  ui_scale: number
  /** 旧添加设备窗基准，仅兼容旧配置/迁移；当前添加窗固定设计尺寸 1×。 */
  add_base?: number
  popup_base: number
  popup_scale: number
  tip_base: number
  tip_scale: number
  split_scales: boolean
  indicator_style: IndicatorStyle
  /** 主窗界面风格：`photo` = 实物照片卡片，`classic` = 3.0 玻璃卡片。 */
  ui_style: UiStyle
  /** 实物卡片每行张数。 */
  photo_cols: PhotoColumns
  /** 实物卡片模式下的侧栏是否收起（只留 60px 图标轨道）。 */
  sidebar_collapsed: boolean
  /** 总览页设备显示：`all` = 含未连接，`connected` = 仅已连接。 */
  display_summary: DeviceDisplay
  /** 设备页设备显示。 */
  display_devices: DeviceDisplay
  /** 预览卡片（托盘小窗）设备显示。 */
  display_popup: DeviceDisplay
  /** 悬停卡片设备显示。 */
  display_tip: DeviceDisplay
  ui_theme: UiTheme
  theme_mode: ThemeMode
  theme_light_at: string
  theme_dark_at: string
  notify_low: boolean
  notify_level: number
  /** 低电量重复提醒间隔（分钟）；0 = 仅提醒一次。 */
  notify_interval_minutes: number
  alert_mode: AlertMode
  /** 托盘图标提醒的闪烁时长（秒）。 */
  tray_blink_seconds: number
  /** 托盘闪电图标的电量来源：设备平均值 / 自选设备 / 最低电量。 */
  tray_level_mode: TrayLevelMode
  /** 托盘图标样式：闪电 / 圆环 / 电池。 */
  tray_icon_style: TrayIconStyle
  /** 自选设备 id；空串 = 尚未选择。 */
  tray_level_device: string
  alert_base: number
  alert_scale: number
  alert_sound: boolean
  alert_sound_file: string
  alert_title: string
  alert_subtitle: string
  alert_pos: AlertPosition
  /** 背景图缩放（cover 之上，1–3）。 */
  bg_ratio: number
  /** 背景画面平移（−50–50 百分比，与缩放相互独立；露出的部分用主题白/黑填充）。 */
  bg_dx: number
  bg_dy: number
  /** 自定义壁纸磨砂强度（px，0–24）。 */
  bg_blur: number
  /** 自定义壁纸暗化（0–60 百分比）。 */
  bg_dim: number
  hotkey_enabled: boolean
  hotkey: string
  last_levels: Record<string, number>
}

/** `get_config()` 的返回结构。 */
export interface ConfigSnapshot {
  settings: AppSettings
  devices: DeviceConfig[]
}

/** 默认设置（出厂默认 = 用户当前配置快照，`hotkey*` 为 E1 新增项）。 */
const DEFAULT_SETTINGS: AppSettings = {
  autostart: false,
  refresh_seconds: 60,
  hover_delay: 0.3,
  good_level: 80,
  warn_level: 20,
  band_colors: { good: 'green', mid: 'blue', low: 'red' },
  ui_base: 1.0,
  ui_scale: 1.0,
  add_base: 1.0,
  popup_base: 1.0,
  popup_scale: 1.0,
  tip_base: 1.0,
  tip_scale: 1.0,
  split_scales: true,
  indicator_style: 'ring',
  ui_style: 'photo',
  photo_cols: 3,
  sidebar_collapsed: false,
  display_summary: 'connected',
  display_devices: 'connected',
  display_popup: 'all',
  display_tip: 'all',
  ui_theme: 'light',
  theme_mode: 'manual',
  theme_light_at: '07:00',
  theme_dark_at: '19:00',
  notify_low: true,
  notify_level: 20,
  notify_interval_minutes: 0,
  alert_mode: 'tray',
  tray_blink_seconds: 10,
  tray_level_mode: 'average',
  tray_icon_style: 'bolt',
  tray_level_device: '',
  alert_base: 1.0,
  alert_scale: 1.0,
  alert_sound: true,
  alert_sound_file: 'notify_plat.wav',
  alert_title: '',
  alert_subtitle: '',
  alert_pos: null,
  bg_ratio: 1.0,
  bg_dx: 0,
  bg_dy: 0,
  bg_blur: 12,
  bg_dim: 0,
  hotkey_enabled: false,
  hotkey: 'ctrl+alt+d',
  last_levels: {},
}

/** 取一份可安全修改的默认设置（嵌套对象已复制）。 */
export function defaultSettings(): AppSettings {
  return {
    ...DEFAULT_SETTINGS,
    band_colors: { ...DEFAULT_SETTINGS.band_colors },
    last_levels: {},
    alert_pos: null,
  }
}

/** 出厂设备列表为空：设备一律由用户自己添加。 */
export function defaultDevices(): DeviceConfig[] {
  return []
}
