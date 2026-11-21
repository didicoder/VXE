export interface DiscoveredDevice {
  id: string
  label: string
  vid: string
  pid: string
  wired_pid: string
  link: string
  bt_addr: string
  icon: string
  color: string
  profile: string
  profile_label: string
  /** 识别出的型号 id；空 = 共用 ID 需要在添加窗选择。 */
  model: string
  model_label: string
  model_candidates: ModelChoice[]
  /** 接收器/设备实例键。 */
  instance: string
  hit: boolean
  level: number | null
  charging: boolean
}

export interface ModelChoice {
  id: string
  label: string
  vendor?: string
  keywords?: string
  profile: string
  icon?: string
  link?: string
}

export interface ProfileChoice {
  id: string
  label: string
  /** 厂家分组（多级菜单用），可缺省。 */
  vendor?: string
  /** 搜索别名（型号/俗称），搜索命中用。 */
  keywords?: string
}

export interface DetectResult {
  ok: boolean
  text: string
}
