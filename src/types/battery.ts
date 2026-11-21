/**
 * 电量基础类型与常量（与 `src-tauri/src/battery/mod.rs` 的 `BatteryState` 对齐）。
 *
 * 唯一真值在 Rust：三档阈值判定、迟滞、提醒队列都由后端决定；
 * 这里的类型只负责展示层的形状与出厂默认值。
 */

/** 电量三档，顺序与 V16 `theme.BANDS` 一致。 */
export const BATTERY_BANDS = ['good', 'mid', 'low'] as const

export type BatteryBand = (typeof BATTERY_BANDS)[number]

/** 六色强调色板，顺序与 V16 `theme.PALETTE` 一致（新设备按序轮转取色）。 */
export const COLOR_KEYS = ['blue', 'red', 'green', 'purple', 'amber', 'cyan'] as const

export type ColorKey = (typeof COLOR_KEYS)[number]

/** 读取状态：ok 在线 / sleep 休眠 / absent 未连接 / error 读取失败。 */
export type BatteryState = 'ok' | 'sleep' | 'absent' | 'error'

/** 全局三档配色（config.json 的 `band_colors`）。 */
export type BandColors = Record<BatteryBand, ColorKey>

/** 设备级三档覆盖：只存与全局不同的档位，`{}` = 全跟随全局。 */
export type DeviceColors = Partial<Record<BatteryBand, ColorKey>>

/** 出厂配色。任何「没配过」的档位都回落到这里。 */
export const FACTORY_BAND_COLORS: BandColors = {
  good: 'green',
  mid: 'blue',
  low: 'red',
}

/** 「电量充足」默认下限（V16 `store.DEFAULTS.good_level`）。 */
export const DEFAULT_GOOD_LEVEL = 80

/** 「电量不足」默认下限（V16 `store.DEFAULTS.warn_level`）。 */
export const DEFAULT_WARN_LEVEL = 20
