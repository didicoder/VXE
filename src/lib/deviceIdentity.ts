import type { DiscoveredDevice } from '../types/discover';
import type { DeviceConfig } from '../types/device';

const norm = (value: string | null | undefined): string => (value ?? '').trim().toLowerCase();

/**
 * 是否同一台硬件：HID 看 vid + pid/wired_pid，蓝牙看地址。
 * 有硬件标识时不再用 id 兜底，避免不同设备恰好同 id 被误判。
 */
export function isSameDevice(existing: DeviceConfig, candidate: DiscoveredDevice): boolean {
  const addr = norm(candidate.bt_addr);
  if (addr) return norm(existing.bt_addr) === addr;

  const instance = norm(candidate.instance);
  const model = norm(candidate.model);
  const existingInstance = norm(existing.instance);
  const existingModel = norm(existing.model);
  if (instance && existingInstance) {
    if (instance !== existingInstance) return false;
    if (model && existingModel && model !== existingModel) return false;
    return true;
  }
  if (model && existingModel && model !== existingModel) return false;

  const vid = norm(candidate.vid);
  const pids = [norm(candidate.pid), norm(candidate.wired_pid)].filter(Boolean);
  if (vid && pids.length > 0) {
    if (norm(existing.vid) !== vid) return false;
    const existingPids = [norm(existing.pid), norm(existing.wired_pid)].filter(Boolean);
    return pids.some((pid) => existingPids.includes(pid));
  }

  const id = norm(candidate.id);
  return id.length > 0 && (norm(existing.id) === id || norm(existing.key) === id);
}


/** 卡面副标题的"代码"：蓝牙显地址、XInput 显槽位、其余 VID:PID。 */
export function deviceIdent(device: {
  vid?: string;
  pid?: string;
  bt_addr?: string;
}): string {
  const addr = (device.bt_addr ?? '').trim();
  if (addr) return addr.toUpperCase();
  const vid = (device.vid ?? '').trim();
  const pid = (device.pid ?? '').trim();
  if (vid.toUpperCase() === 'XINPUT') {
    return `XInput ${Number(pid) + 1} 号位`;
  }
  return `${vid}:${pid}`;
}
