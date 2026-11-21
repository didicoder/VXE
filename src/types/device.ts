/**
 * 设备形状：`config.json` 里的设备配置（`DeviceConfig`）与一次读取的状态
 * （`DeviceStatus`，逐字段对齐 `src-tauri/src/battery/mod.rs`）。
 */

import type { BatteryState, ColorKey, DeviceColors } from './battery'

/** 设备剪影（V16 `theme.DEVICE_ICONS`）。 */
export const DEVICE_ICONS = [
  'mouse',
  'keyboard',
  'headset',
  'earbuds',
  'gamepad',
  'phone',
] as const

export type DeviceIcon = (typeof DEVICE_ICONS)[number]

/** 图标字段的运行时形状：`''` = 没选过（按设备类型回退）。 */
export type DeviceIconKey = DeviceIcon | ''

/** 实物卡片底部三行字段的显示开关（窄卡自动隐藏）。 */
export interface CardFields {
  link: boolean
  ident: boolean
  power: boolean
}

/**
 * `config.json` 的 `devices[]` 项，字段与 V16 `store.normalize_device` 之后的形状一致。
 * `colors` 只记录用户改过的档位；`{}` 表示三档全跟随全局 `band_colors`。
 *
 * `card_*` 是 3.1 实物卡片的单卡设置（全部可选，缺省即用派生值）：
 * `card_img` 只记标记，图片本体存配置目录 `card-images/{id}.dat`。
 */
export interface DeviceConfig {
  id: string
  key: string
  label: string
  vid: string
  pid: string
  wired_pid: string
  profile: string
  link: string
  bt_addr: string
  color: ColorKey
  colors: DeviceColors
  icon: DeviceIconKey
  /** 型号目录 id；旧配置/未识别为空。 */
  model?: string
  /** 接收器/设备实例键；蓝牙、手柄和旧配置为空。 */
  instance?: string
  card_brand?: string
  card_name?: string
  card_wm?: string
  card_img?: string
  /** 旧版图片缩放；已停用，仅保留字段兼容旧配置。 */
  card_zoom?: number
  /** 图片长边占卡片长边的百分比（1–100，缺省 80）。 */
  card_ratio?: number
  card_dx?: number
  card_dy?: number
  /** 图片旋转角度（度，-180~180；缺省 0）。 */
  card_rot?: number
  card_shadow?: boolean
  card_glow?: boolean
  card_fields?: CardFields
}

/**
 * 一次读取的设备状态（`battery/mod.rs` 的 `DeviceStatus`）。
 * `level` 为 null 表示这次没读到；`last` 是上一次有效读数（休眠时展示用）。
 */
export interface DeviceStatus {
  id: string
  label: string
  vid: string
  pid: string
  state: BatteryState
  level: number | null
  charging: boolean
  link: string
  color: ColorKey
  colors: DeviceColors
  profile: string
  icon: DeviceIconKey
  last: number | null
}
