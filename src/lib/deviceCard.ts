/**
 * 实物照片卡片的展示值（`card-preview/device-card-integration.html` 的数据结构）。
 *
 * 卡片文字全部可被 `config.devices[].card_*` 覆盖；缺省值从设备本身派生：
 * 品牌取显示名首个词、名称去掉品牌前缀、竖排字按设备类型。图片只来自用户
 * 自选图（单独存文件）；没有自选图时卡片画电量光环（`BatteryOrb`），不预置任何实物图。
 */

import type { DeviceIconName } from '../components/icons/DeviceIcon.vue'
import type { DeviceDisplay } from '../types/config'
import type {
  CardFields,
  DeviceConfig,
  DeviceStatus,
} from '../types/device'
import { bandOf } from './bands'
import { levelOf } from './state'

const WM_BY_ICON: Record<DeviceIconName, string> = {
  mouse: 'MOUSE',
  keyboard: 'KEYBOARD',
  headset: 'HEADSET',
  earbuds: 'EARBUDS',
  gamepad: 'GAMEPAD',
  phone: 'PHONE',
}

const DEFAULT_FIELDS: CardFields = { link: true, ident: true, power: true }

/** 设备图标：显式选择优先，否则按蓝牙音频回退耳机、其余回退鼠标。 */
export function resolveCardIcon(
  device: Pick<DeviceConfig, 'icon' | 'profile' | 'link'>,
): DeviceIconName {
  if (device.icon) return device.icon
  const profile = (device.profile ?? '').toLowerCase()
  if (
    profile.startsWith('bt-audio') ||
    (device.link ?? '').includes('音频') ||
    (device.link ?? '').includes('耳机')
  ) {
    return 'headset'
  }
  return 'mouse'
}

/** 品牌大字：取显示名首个词（"示例 K1 键盘" → "示例"）。 */
export function brandOf(label: string): string {
  const first = (label ?? '').trim().split(/\s+/)[0] ?? ''
  return first.length > 0 ? first : 'VXE'
}

/** 第二行：去掉与品牌重复的前缀（"示例 M1 鼠标" → "M1 鼠标"）。 */
export function nameOf(label: string, brand: string): string {
  const trimmed = (label ?? '').trim()
  if (!trimmed) return '—'
  if (brand && trimmed.toLowerCase().startsWith(brand.toLowerCase())) {
    const rest = trimmed.slice(brand.length).replace(/^[\s·\-—]+/, '')
    if (rest) return rest
  }
  return trimmed
}

/** 卡片图片只来自用户自选图；没有自选图时由卡片画电量光环。 */
interface CardView {
  icon: DeviceIconName
  brand: string
  name: string
  wm: string
  /** 图片变换：长边占卡片长边的百分比 + 水平/垂直偏移百分比 + 旋转角度（度）。 */
  ratio: number
  dx: number
  dy: number
  rot: number
  shadow: boolean
  glow: boolean
  fields: CardFields
}

/** 合并单卡配置与派生默认值。 */
export function cardView(device: DeviceConfig): CardView {
  const icon = resolveCardIcon(device)
  const brand = device.card_brand?.trim() || brandOf(device.label)
  const fields = { ...DEFAULT_FIELDS, ...(device.card_fields ?? {}) }
  return {
    icon,
    brand,
    name: device.card_name?.trim() || nameOf(device.label, brand),
    wm: device.card_wm?.trim() || WM_BY_ICON[icon],
    ratio: Number.isFinite(device.card_ratio) ? (device.card_ratio as number) : 80,
    dx: Number.isFinite(device.card_dx) ? (device.card_dx as number) : 0,
    dy: Number.isFinite(device.card_dy) ? (device.card_dy as number) : 0,
    rot: Number.isFinite(device.card_rot) ? (device.card_rot as number) : 0,
    shadow: device.card_shadow ?? true,
    glow: device.card_glow ?? true,
    fields,
  }
}

/** 卡片上显示的连接方式。 */
export function cardLink(device: DeviceConfig, status: DeviceStatus | null): string {
  if (device.link) return device.link
  return status?.link || '2.4G'
}

/** 卡片上显示的供电状态。 */
export function cardPower(status: DeviceStatus | null): string {
  if (!status) return '电池供电'
  return status.charging && (status.state === 'ok' || status.state === 'sleep')
    ? '充电中'
    : '电池供电'
}

/** 卡片右下角的状态胶囊：`live` 绿字，`warn` 告警色。 */
export interface CardChip {
  tone: 'live' | 'warn'
  text: string
}

/**
 * 照片卡状态文案：`state` 优先，历史读数只决定数字，不把休眠/失败说成实时。
 * `charging` 只在 ok/sleep 时算数，与浮层 `StateChip` 同一规则。
 */
export function cardChip(
  status: DeviceStatus | null,
  level: number | null,
  low: boolean,
): CardChip {
  const state = status?.state
  const charging =
    status?.charging === true && (state === 'ok' || state === 'sleep')
  if (charging) return { tone: 'live', text: '充电中' }
  if (state === 'sleep') return { tone: 'warn', text: '休眠' }
  if (state === 'absent') return { tone: 'warn', text: '未连接' }
  if (state === 'error') return { tone: 'warn', text: '读取失败' }
  if (level == null) return { tone: 'warn', text: '未读数' }
  if (low) return { tone: 'warn', text: '低电量' }
  return { tone: 'live', text: '实时' }
}

/** 低电量标记：与现有三档配色同一判据（低于 warn 档）。 */
export function cardLow(status: DeviceStatus | null, good: number, warn: number): boolean {
  if (!status) return false
  const level = levelOf(status).level
  if (level == null) return false
  return bandOf(level, good, warn) === 'low'
}

/**
 * 设备当前是否接在本机上（卡片是否该出现在列表里）。
 *
 * 只有 `absent`（后端明确报「没读到这个设备」）才当作不存在：Xbox 手柄拔掉、
 * 接收器拔掉、蓝牙耳机没连，这些设备留在 `config.devices` 里但已经没有对应的
 * 硬件。`sleep` 是设备在、只是休眠，`error` 是设备在但协议读不出电量，两者都保留。
 * 还没有状态（首帧 / 未轮询）时按在线处理，避免整页先空一下。
 */
export function isDevicePresent(status: DeviceStatus | null | undefined): boolean {
  return status?.state !== 'absent'
}

/**
 * 按「设备显示」设置过滤状态列表（`display_*` 两档）：
 * `all` = 含未连接，`connected` = 只留真正接在本机上的（`isDevicePresent`）。
 */
export function filterStatusesByDisplay(
  statuses: DeviceStatus[],
  mode: DeviceDisplay,
): DeviceStatus[] {
  return mode === 'all' ? statuses : statuses.filter(isDevicePresent)
}

/** 同上，按设备列表过滤：`statusById` 查不到状态时按在线处理（与 `isDevicePresent` 一致）。 */
export function filterDevicesByDisplay(
  devices: DeviceConfig[],
  statusById: ReadonlyMap<string, DeviceStatus>,
  mode: DeviceDisplay,
): DeviceConfig[] {
  return mode === 'all'
    ? devices
    : devices.filter((device) => isDevicePresent(statusById.get(device.id)))
}