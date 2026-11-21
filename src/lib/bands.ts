/**
 * 电量分档与配色（V16 `theme.band_of` / `theme.band_color` / `theme.level_color`
 * 与 `widgets.is_charging` / `widgets.online` 的纯函数移植）。
 *
 * 只算「显示成什么颜色」，不做低电量判定 —— 阈值与迟滞的唯一真值在 Rust。
 */

import {
  COLOR_KEYS,
  DEFAULT_GOOD_LEVEL,
  DEFAULT_WARN_LEVEL,
  FACTORY_BAND_COLORS,
  type BandColors,
  type BatteryBand,
  type ColorKey,
  type DeviceColors,
} from '../types/battery'
import type { DeviceStatus } from '../types/device'

/** 判定需要的最小字段（DeviceStatus 或配置里的设备都能传）。 */
type ChargeState = Pick<DeviceStatus, 'charging' | 'state'>

type OnlineState = Pick<DeviceStatus, 'state'>

function isColorKey(value: unknown): value is ColorKey {
  return typeof value === 'string' && (COLOR_KEYS as readonly string[]).includes(value)
}

/**
 * 电量落在哪一档。读不到电量按「正常使用」，别一上来就标红（V16 同语义）。
 */
export function bandOf(
  level: number | null | undefined,
  good: number = DEFAULT_GOOD_LEVEL,
  warn: number = DEFAULT_WARN_LEVEL,
): BatteryBand {
  if (level == null || !Number.isFinite(level)) return 'mid'
  if (level >= good) return 'good'
  return level >= warn ? 'mid' : 'low'
}

/**
 * 某一档的显示色：设备覆盖 → 全局 band_colors → 出厂色（绿 / 蓝 / 红）。
 * `colors` 只覆盖非空档位；非法色名按「没配过」处理。
 */
export function bandColor(
  band: BatteryBand,
  bandColors: BandColors = FACTORY_BAND_COLORS,
  deviceColors: DeviceColors = {},
): ColorKey {
  const own = deviceColors[band]
  if (isColorKey(own)) return own
  const global = bandColors?.[band]
  return isColorKey(global) ? global : FACTORY_BAND_COLORS[band]
}

/**
 * 电量数值直接对应的档位色；`null` 表示没有读数（调用方用 `--gray` / 图形轨道色）。
 */
export function levelColor(
  level: number | null | undefined,
  good: number = DEFAULT_GOOD_LEVEL,
  warn: number = DEFAULT_WARN_LEVEL,
  bandColors: BandColors = FACTORY_BAND_COLORS,
  deviceColors: DeviceColors = {},
): ColorKey | null {
  if (level == null || !Number.isFinite(level)) return null
  return bandColor(bandOf(level, good, warn), bandColors, deviceColors)
}

/**
 * 充电标志只在设备在线（ok / sleep）时算数；蓝牙音频没有这个标志，恒为 false。
 */
export function isCharging(status: ChargeState): boolean {
  return status.charging === true && (status.state === 'ok' || status.state === 'sleep')
}

/** 在线 = ok 或 sleep（休眠仍算在线，和 V16 `widgets.online` 一致）。 */
export function online(status: OnlineState): boolean {
  return status.state === 'ok' || status.state === 'sleep'
}
