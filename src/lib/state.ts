/**
 * 设备展示文案与统计（V16 `widgets.state_text` / `widgets.level_of` /
 * `widgets.stats` 的纯函数移植）。
 *
 * 这里只把 Rust 给的状态翻译成文本，不重新判定低电量：通知阈值与迟滞
 * 的唯一真值在 Rust。
 */

import type { BatteryState } from '../types/battery'
import type { DeviceStatus } from '../types/device'
import { online } from './bands'

/** V16 `widgets.STATE_TEXT`：ok 也是“已连接”，不是空串。 */
const STATE_TEXT: Record<BatteryState, string> = {
  ok: '已连接',
  sleep: '休眠',
  absent: '未连接',
  error: '读取失败',
}

/** 未知 state（脏数据）兜底文案。 */
const UNKNOWN_TEXT = '--'

/** 充电中优先；其余按 V16 `widgets.state_text` 表。 */
export function stateText(state: BatteryState, charging = false): string {
  if (charging && (state === 'ok' || state === 'sleep')) return '充电中'
  return STATE_TEXT[state] ?? UNKNOWN_TEXT
}

/**
 * 对外展示的电量数字（不含 `%`）：大号数字用这个，百分号单独排版。
 * 没读数时返回 `--`。
 */
export function levelText(level: number | null | undefined): string {
  if (level == null || !Number.isFinite(level)) return '--'
  return String(Math.round(level))
}

/** 一台设备要展示的电量与来源。 */
interface DisplayLevel {
  level: number | null
  /** true = 本次实时读数；false = 休眠/掉线时的上次读数。 */
  live: boolean
}

/**
 * 展示用电量：实时优先，休眠/掉线退回上次读数（V16 `widgets.level_of`）。
 */
export function levelOf(
  device: Pick<DeviceStatus, 'state' | 'level' | 'last'>,
): DisplayLevel {
  if (device.state === 'ok' && device.level != null && Number.isFinite(device.level)) {
    return { level: device.level, live: true }
  }
  if (device.last != null && Number.isFinite(device.last)) {
    return { level: device.last, live: false }
  }
  return { level: null, live: false }
}

/** 总览 Hero 用的统计。 */
interface DeviceStats {
  total: number
  online: number
  /** 参与统计的最低电量；没读数时为 null。 */
  min: number | null
  /** 平均电量（四舍五入到整数）；没读数时为 0（V16 同语义）。 */
  avg: number
}

/**
 * Python `round()` 的 half-even 语义：恰好 .5 时取偶数。
 * V16 用 `int(round(sum / len))`，80.5 得 80 而不是 81。
 */
function roundHalfEven(value: number): number {
  const floor = Math.floor(value)
  const diff = value - floor
  if (diff < 0.5) return floor
  if (diff > 0.5) return floor + 1
  return floor % 2 === 0 ? floor : floor + 1
}

/** 在线数 / 总数 / 最低 / 平均（V16 `widgets.stats`）。 */
export function stats(devices: readonly DeviceStatus[]): DeviceStats {
  const levels = devices
    .map((device) => levelOf(device).level)
    .filter((level): level is number => level != null)
  const sum = levels.reduce((total, level) => total + level, 0)
  return {
    total: devices.length,
    online: devices.filter((device) => online(device)).length,
    min: levels.length > 0 ? Math.min(...levels) : null,
    avg: levels.length > 0 ? roundHalfEven(sum / levels.length) : 0,
  }
}
