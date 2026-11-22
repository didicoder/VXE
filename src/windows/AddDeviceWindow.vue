<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import ColorSwatches from '../components/add/ColorSwatches.vue';
import IconGrid from '../components/add/IconGrid.vue';
import ScanRow from '../components/add/ScanRow.vue';
import ProtocolSelect from '../components/add/ProtocolSelect.vue';
import LineIcon from '../components/icons/LineIcon.vue';
import PillButton from '../components/PillButton.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowMotion } from '../composables/useWindowMotion';
import { useWindowScale } from '../composables/useWindowScale';
import { isSameDevice } from '../lib/deviceIdentity';
import { clamp } from '../lib/geometry';
import * as ipc from '../lib/ipc';
import { initializeWindow } from '../lib/windowLifecycle';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';
import { useDiscoverStore } from '../stores/discover';
import { COLOR_KEYS, type ColorKey } from '../types/battery';
import { DEVICE_ICONS, type DeviceConfig, type DeviceIcon } from '../types/device';
import type { DiscoveredDevice, ProfileChoice } from '../types/discover';

defineProps<{ windowLabel?: string }>();

/** 添加窗设计尺寸 459×586.5（即 1×），每页 4 行、行高 54、行距 6。 */
const PER = 4;

const NO_PROFILE: ProfileChoice = { id: '', label: '不指定', vendor: '自动', keywords: '' };

const config = useConfigStore();
const discover = useDiscoverStore();
const deviceStore = useDevicesStore();
const themeClass = useThemeClass();
useWallpaper();
const { entering, leaving, enter, leave } = useWindowMotion();

/** 与 Rust `show_add` 一致：设计尺寸即 1×，当前不乘 scale；base 仅兼容旧配置/迁移。 */
const addScale = useWindowScale('add');

const selectedId = ref<string | null>(null);
/** 多选：勾中的设备一起添加；最后勾选的是下方表单编辑的设备。 */
const checkedIds = ref<string[]>([]);
const name = ref('');
const color = ref<ColorKey>('blue');
const icon = ref<DeviceIcon>('mouse');
const profileId = ref('');
const modelId = ref('');
const modelLink = ref('');
const protocolEditing = ref(false);
const probe = ref<{ ok: boolean; text: string } | null>(null);
const offset = ref(0);
const saving = ref(false);
const scanFailed = ref(false);
const saveError = ref<string | null>(null);

let unlisten: UnlistenFn | null = null;

const selected = computed(
  () => discover.devices.find((device) => device.id === selectedId.value) ?? null,
);

/** 当前扫描设备的型号候选（共用 VID:PID 时会有多个）。 */
const modelCandidates = computed(() => selected.value?.model_candidates ?? []);
const hasModelChoices = computed(() => modelCandidates.value.length > 0);
const selectedModel = computed(
  () => modelCandidates.value.find((choice) => choice.id === modelId.value) ?? null,
);
const modelNeedsPick = computed(() => hasModelChoices.value && !modelId.value);
const showModelPicker = computed(() => hasModelChoices.value && !protocolEditing.value);

const modelOptions = computed<ProfileChoice[]>(() => {
  const list: ProfileChoice[] = modelCandidates.value.map((choice) => ({
    id: choice.id,
    label: choice.label,
    vendor: choice.vendor || '其他',
    keywords: choice.keywords || '',
  }));
  if (modelId.value && !list.some((choice) => choice.id === modelId.value)) {
    list.unshift({
      id: modelId.value,
      label: selected.value?.model_label || modelId.value,
      vendor: '当前设备',
      keywords: '',
    });
  }
  // 「请选择型号」固定行：ProtocolSelect 把空 id 作为固定首行。
  list.unshift({ id: '', label: '请选择型号', vendor: '型号', keywords: '' });
  return list;
});

/** 勾选顺序保留：批量添加时按用户勾选先后入列。 */
const checkedDevices = computed(() =>
  checkedIds.value
    .map((id) => discover.devices.find((device) => device.id === id))
    .filter((device): device is DiscoveredDevice => device !== undefined),
);
const checkedSet = computed(() => new Set(checkedIds.value));
const bulkAdd = computed(() => checkedIds.value.length > 1);
const freshChecked = computed(() =>
  checkedDevices.value.filter((device) => !isAdded(device)),
);

/** 共用 VID:PID 且没选型号的待添加设备：不允许直接保存。 */
const unresolvedChecked = computed(() =>
  freshChecked.value.filter((device) => {
    if (device.id === selectedId.value) return modelNeedsPick.value;
    return device.model_candidates.length > 1 && !device.model;
  }),
);

/** 同一硬件已在配置里：列表标「已添加」，选中后保存 = 更新该设备。 */
function existingFor(device: DiscoveredDevice): DeviceConfig | null {
  return config.devices.find((item) => isSameDevice(item, device)) ?? null;
}

function isAdded(device: DiscoveredDevice): boolean {
  return existingFor(device) !== null;
}

/** 旧配置可能把「VID:PID 2.4G 键盘」这类中性标识存成名称。 */
function neutralLabel(label: string, device: DiscoveredDevice): boolean {
  const prefix = `${device.vid}:${device.pid}`.toLowerCase();
  const trimmed = label.trim().toLowerCase();
  if (prefix.length <= 3 || !trimmed.startsWith(prefix)) return false;
  const rest = trimmed.slice(prefix.length).trim();
  return (
    rest === '' ||
    ['2.4g', '蓝牙', '有线', 'usb', 'hid', '键盘', '鼠标', '耳机', '手柄'].some((word) =>
      rest.startsWith(word),
    )
  );
}

/** 已有自定义名优先；只有旧中性名才换成本次扫描识别出的真实型号。 */
function preferredName(existing: DeviceConfig | null, device: DiscoveredDevice): string {
  const current = existing?.label?.trim() ?? '';
  const scanned = device.label.trim();
  if (!current) return scanned;
  if (current && neutralLabel(current, device) && scanned && !neutralLabel(scanned, device)) {
    return scanned;
  }
  return current;
}

/** 旧中性名往往同时存着同 VID/PID 的占位协议；识别出具体型号后一并更新。 */
function preferredProfile(existing: DeviceConfig | null, device: DiscoveredDevice): string {
  const current = existing?.label?.trim() ?? '';
  if (current && neutralLabel(current, device) && device.profile) {
    return device.profile;
  }
  return existing?.profile || device.profile;
}

/** 扫描后顺手迁移旧版的中性名/占位协议，用户不必手动点“更新”。 */
async function adoptRecognizedLabels(): Promise<void> {
  for (const device of discover.devices) {
    const existing = existingFor(device);
    if (!existing) continue;
    const current = existing.label?.trim() ?? '';
    const scanned = device.label.trim();
    if (current && scanned && neutralLabel(current, device) && !neutralLabel(scanned, device)) {
      await config.updateDevice(existing.id, {
        label: scanned,
        profile: device.profile || existing.profile,
      });
    }
  }
}

const selectedExisting = computed(() =>
  selected.value ? existingFor(selected.value) : null,
);
const selectedAdded = computed(() => selectedExisting.value !== null);

/** 扫描中/扫描失败时先收起旧列表，和 V16 清空列表再扫的观感一致。 */
const visibleDevices = computed(() =>
  discover.scanning || scanFailed.value ? [] : discover.devices,
);

const rows = computed(() => visibleDevices.value.slice(offset.value, offset.value + PER));

const profileOptions = computed<ProfileChoice[]>(() => {
  const base = discover.profiles.length > 0 ? discover.profiles : [NO_PROFILE];
  if (profileId.value && !base.some((choice) => choice.id === profileId.value)) {
    return [
      {
        id: profileId.value,
        label: selected.value?.profile_label || profileId.value,
        vendor: '当前设备',
        keywords: '',
      },
      ...base,
    ];
  }
  return base;
});

const pageHint = computed(() => {
  const total = visibleDevices.value.length;
  const parts: string[] = [];
  if (total > PER) {
    const end = Math.min(total, offset.value + PER);
    parts.push(`滚轮翻页 · ${offset.value + 1}-${end} / ${total}`);
  }
  if (checkedIds.value.length > 0) parts.push(`已选 ${checkedIds.value.length} 台`);
  return parts.join(' · ');
});

interface EmptyState {
  tone: 'idle' | 'bad';
  title: string;
  detail: string;
}

const emptyState = computed<EmptyState>(() => {
  if (discover.scanning) {
    return {
      tone: 'idle',
      title: '正在扫描…',
      detail: '扫描本机 HID 与蓝牙设备，匹配电量协议；需要时可点「试读」验证。',
    };
  }
  if (scanFailed.value) {
    return {
      tone: 'bad',
      title: '扫描失败',
      detail: discover.error ?? '未知错误',
    };
  }
  return {
    tone: 'idle',
    title: '没有找到可读电量的设备',
    detail: '蓝牙设备要先配对并连上再扫描。',
  };
});

const probeText = computed(() => {
  if (discover.detecting) return '正在试读…';
  if (modelNeedsPick.value) return '请选择型号';
  if (probe.value) return probe.value.text;
  return selected.value ? '未试读' : '先选择设备';
});

const probeTone = computed<'idle' | 'ok' | 'bad'>(() => {
  if (discover.detecting) return 'idle';
  if (modelNeedsPick.value) return 'bad';
  if (!probe.value) return 'idle';
  return probe.value.ok ? 'ok' : 'bad';
});

function isColorKey(value: string): value is ColorKey {
  return (COLOR_KEYS as readonly string[]).includes(value);
}

function isDeviceIcon(value: string): value is DeviceIcon {
  return (DEVICE_ICONS as readonly string[]).includes(value);
}

/** 按序挑一个还没被占用的强调色（V16 `theme.pick_device_color`）。 */
function pickColor(): ColorKey {
  const used = new Set(config.devices.map((device) => device.color));
  const free = COLOR_KEYS.find((key) => !used.has(key));
  return free ?? COLOR_KEYS[used.size % COLOR_KEYS.length];
}

function hitText(device: DiscoveredDevice): string {
  if (device.level == null) return '读取成功';
  return `读取成功 · 电量 ${device.level}%${device.charging ? '（充电中）' : ''}`;
}

function resetForm(): void {
  name.value = '';
  profileId.value = '';
  modelId.value = '';
  modelLink.value = '';
  protocolEditing.value = false;
  icon.value = 'mouse';
  color.value = pickColor();
  probe.value = null;
}

function selectDevice(device: DiscoveredDevice): void {
  if (selectedId.value === device.id) return;
  selectedId.value = device.id;
  saveError.value = null;
  const existing = existingFor(device);
  name.value = preferredName(existing, device);
  color.value = isColorKey(existing?.color ?? device.color)
    ? ((existing?.color ?? device.color) as ColorKey)
    : pickColor();
  icon.value = isDeviceIcon(existing?.icon ?? device.icon)
    ? ((existing?.icon ?? device.icon) as DeviceIcon)
    : 'mouse';
  profileId.value = preferredProfile(existing, device);
  modelId.value = existing?.model || device.model || '';
  modelLink.value = existing?.link || device.link || '';
  protocolEditing.value = false;
  // 自动识别出型号时，旧复合协议名（如 "VXE V3 NK / R1 Nearlink 鼠标"）改成具体型号名。
  const model = modelCandidates.value.find((choice) => choice.id === modelId.value);
  if (model) {
    const current = name.value.trim();
    if (!current || neutralLabel(current, device) || current === device.profile_label) {
      name.value = model.label;
    }
  }
  // 扫描列表不再逐台读电量：只有真正读到过数值才显示“读取成功”。
  probe.value = device.level != null ? { ok: true, text: hitText(device) } : null;
}

/** 勾选一台设备（已勾选时不会重复），并把它设为表单编辑对象。 */
function checkDevice(device: DiscoveredDevice): void {
  if (!checkedIds.value.includes(device.id)) {
    checkedIds.value = [...checkedIds.value, device.id];
  }
  selectDevice(device);
}

/** 点行 = 勾选/取消勾选；取消当前行时把表单切到还勾着的最后一台。 */
function toggleDevice(device: DiscoveredDevice): void {
  if (checkedIds.value.includes(device.id)) {
    checkedIds.value = checkedIds.value.filter((id) => id !== device.id);
    if (selectedId.value === device.id) {
      const last = checkedIds.value[checkedIds.value.length - 1];
      const next = last
        ? (discover.devices.find((item) => item.id === last) ?? null)
        : null;
      if (next) selectDevice(next);
      else selectedId.value = null;
    }
    saveError.value = null;
    return;
  }
  checkDevice(device);
}

/** 把表单重置为本次扫描的识别结果（保存后即覆盖已有设备的名称 / 协议）。 */
function resetRecognized(): void {
  const device = selected.value;
  if (!device) return;
  name.value = device.label;
  profileId.value = device.profile;
  modelId.value = device.model;
  modelLink.value = device.link;
  protocolEditing.value = false;
  color.value = isColorKey(device.color) ? device.color : pickColor();
  icon.value = isDeviceIcon(device.icon) ? device.icon : 'mouse';
  probe.value = device.level != null ? { ok: true, text: hitText(device) } : null;
  saveError.value = null;
}

async function runScan(): Promise<void> {
  if (discover.scanning) return;
  scanFailed.value = false;
  saveError.value = null;
  selectedId.value = null;
  checkedIds.value = [];
  offset.value = 0;
  resetForm();
  await config.load();
  await discover.scan();
  await adoptRecognizedLabels();
  await loadUnknowns();
  if (discover.error !== null) {
    scanFailed.value = true;
    return;
  }
  const first = discover.devices.find((device) => !isAdded(device)) ?? discover.devices[0];
  if (first) checkDevice(first);
}

// ---------- B：智能探测未适配设备 ----------

const unknowns = ref<ipc.UnknownHid[]>([]);
const probing = ref(false);
const probeReport = ref('');

const unknownLine = computed(() => {
  if (probing.value) return '正在轮试协议模板…';
  if (probeReport.value) return probeReport.value;
  const names = unknowns.value.slice(0, 2).map((item) => `${item.vid}:${item.pid}`);
  const more = unknowns.value.length > 2 ? ' 等' : '';
  return `未适配设备 ${unknowns.value.length} 台（${names.join('、')}${more}）`;
});

async function loadUnknowns(): Promise<void> {
  try {
    unknowns.value = await ipc.listUnknownHid();
  } catch {
    unknowns.value = [];
  }
}

/** 逐台轮试协议模板；命中即生成协议 JSON 并直接入列。 */
async function runProbeAll(): Promise<void> {
  if (probing.value) return;
  probing.value = true;
  probeReport.value = '';
  let okCount = 0;
  let failCount = 0;
  for (const item of unknowns.value) {
    try {
      const result = await ipc.probeUnknownHid(item);
      if (result.ok) {
        okCount += 1;
        await discover.add({
          id: `auto${item.vid.toLowerCase()}${item.pid.toLowerCase()}`,
          label: `自动适配 ${item.vid}:${item.pid}`,
          vid: item.vid,
          pid: item.pid,
          wired_pid: '',
          link: '2.4G',
          bt_addr: '',
          icon: 'mouse',
          color: pickColor(),
          profile: result.profile_id,
          profile_label: result.text,
          model: '',
          model_label: '',
          model_candidates: [],
          instance: '',
          hit: true,
          level: result.level,
          charging: result.charging,
        });
      } else {
        failCount += 1;
      }
    } catch {
      failCount += 1;
    }
  }
  probeReport.value =
    okCount > 0
      ? `已适配 ${okCount} 台${failCount ? `，未识别 ${failCount} 台` : ''}`
      : '未识别出电量协议（可抓包后手动写协议）';
  await loadUnknowns();
  await config.load();
  probing.value = false;
}

function onWheel(event: WheelEvent): void {
  const total = discover.devices.length;
  if (total <= PER) return;
  const maxOffset = Math.max(0, total - PER);
  offset.value = clamp(offset.value + (event.deltaY > 0 ? 1 : -1), 0, maxOffset);
}

function onProfileChange(): void {
  probe.value = null;
  saveError.value = null;
}

/** 协议名里的链接后缀（"示例键盘 (2.4G)"）去掉再当名称。 */
function profileModelName(label: string): string {
  return label.replace(/[（(][^）)]*[）)]/g, '').trim();
}

/** 协议选择器选中（多级菜单）。 */
function onProfilePick(value: string): void {
  const device = selected.value;
  const before = name.value.trim();
  const prevChoice = profileOptions.value.find((choice) => choice.id === profileId.value);
  const prevAuto = prevChoice ? profileModelName(prevChoice.label) : '';
  profileId.value = value;
  // 名称还是中性名/空、或上一轮由协议自动带入时，跟随新协议更新；手写名称保持不动。
  // 已经选了型号时名称跟型号走，不因手动改协议而变化。
  if (device && value && !modelId.value) {
    const nextChoice = profileOptions.value.find((choice) => choice.id === value);
    const suggested = nextChoice ? profileModelName(nextChoice.label) : '';
    const auto = !before || neutralLabel(before, device) || (prevAuto !== '' && before === prevAuto);
    if (suggested && auto) name.value = suggested;
  }
  onProfileChange();
}

/** 型号下拉选中：带出名称/图标/连接方式/默认协议；空 id = 未选。 */
function onModelPick(value: string): void {
  const device = selected.value;
  const choice = modelCandidates.value.find((item) => item.id === value);
  const previous = selectedModel.value?.label ?? '';
  modelId.value = choice?.id ?? '';
  protocolEditing.value = false;
  probe.value = null;
  saveError.value = null;
  if (!choice || !device) return;
  const before = name.value.trim();
  const auto =
    !before ||
    neutralLabel(before, device) ||
    before === device.profile_label ||
    before === device.model_label ||
    before === previous;
  if (auto) name.value = choice.label;
  if (choice.icon && isDeviceIcon(choice.icon)) icon.value = choice.icon;
  modelLink.value = choice.link || modelLink.value || device.link;
  if (choice.profile) profileId.value = choice.profile;
}

function buildSpec(device: DiscoveredDevice): Partial<DeviceConfig> {
  return {
    id: device.id,
    key: device.id,
    label: name.value.trim() || device.label,
    vid: device.vid,
    pid: device.pid,
    wired_pid: device.wired_pid,
    profile: profileId.value,
    link: modelLink.value || device.link,
    bt_addr: device.bt_addr,
    color: color.value,
    icon: icon.value,
    model: modelId.value,
    instance: device.instance,
  };
}

async function runDetect(): Promise<void> {
  const device = selected.value;
  if (!device || discover.detecting) return;
  probe.value = null;
  const result = await discover.detect(buildSpec(device), profileId.value);
  probe.value = result ?? { ok: false, text: discover.error ?? '试读失败' };
}

/** 批量添加：除当前编辑的那台用表单值外，其余保留识别结果。 */
async function addMany(targets: DiscoveredDevice[]): Promise<void> {
  const fresh = targets.filter((device) => !existingFor(device));
  if (fresh.length === 0) {
    saveError.value = '选中的设备都已添加，无需重复添加';
    return;
  }
  if (unresolvedChecked.value.length > 0) {
    saveError.value = '请先为共用接收器的设备选择型号';
    return;
  }
  saving.value = true;
  saveError.value = null;
  let ok = 0;
  let lastError = '';
  for (const device of fresh) {
    const useForm = device.id === selectedId.value;
    const added = await discover.add({
      ...device,
      label: useForm ? name.value.trim() || device.label : device.label,
      color: useForm
        ? color.value
        : isColorKey(device.color)
          ? device.color
          : pickColor(),
      icon: useForm
        ? icon.value
        : isDeviceIcon(device.icon)
          ? device.icon
          : 'mouse',
      profile: useForm ? profileId.value : device.profile,
      model: useForm ? modelId.value : device.model,
      link: useForm ? (modelLink.value || device.link) : device.link,
      instance: device.instance,
    });
    if (added) ok += 1;
    else lastError = discover.error ?? '保存失败';
  }
  saving.value = false;
  if (ok === fresh.length) {
    checkedIds.value = [];
    await hideWindow();
    // 新设备加入后立刻采集一次，面板/预览不用等下一个轮询周期。
    void deviceStore.refreshNow();
    return;
  }
  saveError.value =
    ok > 0
      ? `已添加 ${ok} 台，${fresh.length - ok} 台失败：${lastError}`
      : lastError;
}

async function save(): Promise<void> {
  if (saving.value) return;
  if (bulkAdd.value) {
    await addMany(checkedDevices.value);
    return;
  }
  const device = checkedDevices.value[0] ?? selected.value;
  if (!device) return;
  if (modelNeedsPick.value) {
    saveError.value = '请先选择设备型号';
    return;
  }
  saving.value = true;
  saveError.value = null;
  const existing = existingFor(device);
  if (existing) {
    // 重新添加 = 更新已有设备：名称 / 型号 / 协议 / 颜色 / 图标都以本次表单为准。
    const ok = await config.updateDevice(existing.id, {
      label: name.value.trim() || device.label,
      profile: profileId.value,
      model: modelId.value,
      instance: device.instance,
      color: color.value,
      icon: icon.value,
      link: modelLink.value || device.link,
    });
    saving.value = false;
    if (ok) {
      await hideWindow();
      void deviceStore.refreshNow();
    } else {
      saveError.value = config.error ?? '更新失败';
    }
    return;
  }
  const added = await discover.add({
    ...device,
    label: name.value.trim() || device.label,
    color: color.value,
    icon: icon.value,
    profile: profileId.value,
    model: modelId.value,
    link: modelLink.value || device.link,
    instance: device.instance,
  });
  saving.value = false;
  if (added) {
    await hideWindow();
    // 新设备加入后立刻采集一次，面板/预览不用等下一个轮询周期。
    void deviceStore.refreshNow();
  } else {
    saveError.value = discover.error ?? '保存失败';
  }
}

const saveLabel = computed(() => {
  if (saving.value) return '保存中…';
  if (bulkAdd.value) {
    return freshChecked.value.length > 0
      ? `添加 ${freshChecked.value.length} 台`
      : '无可添加';
  }
  return selectedAdded.value ? '更新' : '保存';
});

const saveDisabled = computed(
  () =>
    saving.value ||
    !selected.value ||
    modelNeedsPick.value ||
    (bulkAdd.value &&
      (freshChecked.value.length === 0 || unresolvedChecked.value.length > 0)),
);

async function hideWindow(): Promise<void> {
  await leave(async () => {
    try {
      await getCurrentWindow().hide();
    } catch {
      /* 非 Tauri 环境（浏览器预览/单测）下忽略 */
    }
  });
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') void hideWindow();
}

function onAddOpen(): void {
  void enter();
  void runScan();
}

onMounted(() => initializeWindow(async () => {
  window.addEventListener('keydown', onKeydown);
  try {
    unlisten = await listen('add-open', onAddOpen);
  } catch {
    /* 浏览器预览/单测没有事件通道，忽略 */
  }
  await Promise.all([config.load(), discover.loadProfiles()]);
  // 首次扫描由 window_ready 之后的 add-open 触发，避免重复扫描。
}));

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown);
  unlisten?.();
});
</script>

<template>
  <section
    class="add-window glass-window"
    :class="[themeClass, { 'window-enter': entering, 'window-leave': leaving }]"
    :style="{ '--s': addScale }"
  >
    <header class="add-head" data-tauri-drag-region>
      <h1 class="head-title" data-tauri-drag-region>添加设备</h1>
      <p class="head-sub" data-tauri-drag-region>扫描本机 HID 与蓝牙设备，匹配电量协议（可手动试读）</p>
      <button type="button" class="close-btn" aria-label="关闭" @click="hideWindow">
        <LineIcon name="close" :size="15.3" :stroke="1.87" />
      </button>
    </header>

    <span class="section-label">本机设备</span>
    <button
      type="button"
      class="rescan-link"
      :disabled="discover.scanning"
      @click="runScan"
    >
      {{ discover.scanning ? '扫描中…' : '重新扫描' }}
    </button>

    <div class="device-list" @wheel.prevent="onWheel">
      <ScanRow
        v-for="device in rows"
        :key="device.id"
        :device="device"
        :active="device.id === selectedId"
        :added="isAdded(device)"
        :checked="checkedSet.has(device.id)"
        @select="toggleDevice(device)"
      />
      <div v-if="visibleDevices.length === 0" class="empty-card">
        <span class="empty-title" :class="{ 'empty-title--bad': emptyState.tone === 'bad' }">
          {{ emptyState.title }}
        </span>
        <p class="empty-detail">{{ emptyState.detail }}</p>
        <PillButton
          size="sm"
          variant="primary"
          icon="refresh"
          :disabled="discover.scanning"
          @click="runScan"
        >
          重新扫描
        </PillButton>
      </div>
    </div>

    <span v-if="unknowns.length || probeReport" class="unknown-bar">
      <span class="unknown-text">{{ unknownLine }}</span>
      <button
        v-if="unknowns.length"
        type="button"
        class="unknown-link"
        :disabled="probing"
        @click="runProbeAll"
      >
        {{ probing ? '探测中…' : '自动探测' }}
      </button>
    </span>
    <p v-else-if="pageHint" class="page-hint">{{ pageHint }}</p>

    <label
      class="field-label field-label--protocol"
      :for="showModelPicker ? 'model-select' : 'profile-select'"
    >
      {{ showModelPicker ? '设备型号' : '电量协议' }}
    </label>
    <button
      v-if="hasModelChoices"
      type="button"
      class="protocol-toggle"
      @click="protocolEditing = !protocolEditing"
    >
      {{ protocolEditing ? '返回型号' : '更改协议' }}
    </button>
    <span class="probe" :class="`probe--${probeTone}`">
      <span class="probe-dot" />
      <span class="probe-text">{{ probeText }}</span>
    </span>
    <div class="protocol-card">
      <ProtocolSelect
        v-if="showModelPicker"
        id="model-select"
        :model-value="modelId"
        :options="modelOptions"
        empty-label="请选择型号"
        expand-all
        @update:model-value="onModelPick"
      />
      <ProtocolSelect
        v-else
        id="profile-select"
        :model-value="profileId"
        :options="profileOptions"
        @update:model-value="onProfilePick"
      />
      <PillButton
        size="sm"
        variant="soft"
        :disabled="!selected || discover.detecting || modelNeedsPick"
        @click="runDetect"
      >
        {{ discover.detecting ? '试读中…' : '试读' }}
      </PillButton>
    </div>

    <label class="field-label field-label--name" for="device-name">名称</label>
    <button
      v-if="selectedAdded"
      type="button"
      class="reset-link"
      title="用本次扫描的名称、协议、颜色和图标覆盖当前表单"
      @click="resetRecognized"
    >
      重置为识别结果
    </button>
    <div class="name-box">
      <input
        id="device-name"
        v-model="name"
        class="name-input"
        type="text"
        spellcheck="false"
        autocomplete="off"
        :placeholder="selected?.label ?? '设备名称'"
      />
    </div>

    <div class="field-row field-row--color">
      <span class="field-label">强调色</span>
      <ColorSwatches v-model="color" />
    </div>

    <div class="field-row field-row--icon">
      <span class="field-label">图标</span>
      <IconGrid v-model="icon" />
    </div>

    <p v-if="saveError" class="save-error">{{ saveError }}</p>
    <p v-else-if="bulkAdd" class="save-hint">
      已选 {{ checkedIds.length }} 台 · 表单只编辑当前高亮设备
    </p>
    <p v-else-if="selectedAdded" class="save-hint">
      已在列表 · 保存将更新该设备
    </p>
    <footer class="footer">
      <PillButton class="footer-btn footer-btn--cancel" variant="dark" @click="hideWindow">
        取消
      </PillButton>
      <PillButton
        class="footer-btn footer-btn--save"
        :class="{ 'footer-btn--save-many': bulkAdd }"
        variant="primary"
        icon="check"
        :disabled="saveDisabled"
        @click="save"
      >
        {{ saveLabel }}
      </PillButton>
    </footer>
  </section>
</template>

<style scoped>
/* 添加设备窗设计尺寸 459×586.5（1×）：壁纸走 .glass-window */
.add-window {
  position: absolute;
  top: 0;
  left: 0;
  width: 459px;
  height: 586.5px;
  overflow: hidden;
  border-radius: 15.3px;
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
  color: var(--txt);
  transform: scale(var(--s));
  transform-origin: top left;
}

.add-head {
  position: absolute;
  top: 0;
  right: 0;
  left: 0;
  height: 59.5px;
}

.head-title {
  position: absolute;
  top: 17px;
  left: 22.1px;
  margin: 0;
  font-size: 17.85px;
  font-weight: 700;
  line-height: 23.8px;
}

.head-sub {
  position: absolute;
  top: 43.35px;
  left: 22.1px;
  margin: 0;
  color: var(--ink-2);
  font-size: 12.325px;
  font-weight: 400;
  line-height: 15.3px;
}

.close-btn {
  position: absolute;
  top: 17px;
  left: 419.9px;
  display: grid;
  place-items: center;
  width: 30.6px;
  height: 28.9px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--ink-2);
  cursor: pointer;
}

@media (hover: hover) {
  .close-btn:hover {
    background: var(--ctrl-bg-hover);
    color: var(--ink);
  }
}

.close-btn:focus-visible {
  outline: 1.4px solid var(--accent);
  outline-offset: -1.19px;
}

.section-label {
  position: absolute;
  top: 71.4px;
  left: 22.1px;
  height: 15.3px;
  color: var(--ink);
  font-size: 12.75px;
  font-weight: 700;
  line-height: 15.3px;
}

.rescan-link {
  position: absolute;
  top: 68px;
  right: 22.1px;
  height: 22.1px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-family: var(--font-body);
  font-size: 11.05px;
  font-weight: 700;
  line-height: 22.1px;
  cursor: pointer;
}

@media (hover: hover) {
  .rescan-link:not(:disabled):hover {
    color: var(--txt);
  }
}

.rescan-link:disabled {
  opacity: 0.5;
  cursor: default;
}

.rescan-link:focus-visible {
  outline: 1.4px solid var(--accent);
  outline-offset: 1.7px;
  border-radius: 3.4px;
}

.device-list {
  position: absolute;
  top: 91.8px;
  right: 22.1px;
  left: 22.1px;
  display: flex;
  flex-direction: column;
  gap: 5.1px;
  height: 198.9px;
}

.empty-card {
  display: flex;
  flex-direction: column;
  gap: 6.8px;
  align-items: center;
  justify-content: center;
  height: 198.9px;
  padding: 13.6px 15.3px;
  border: 1px solid var(--card-line);
  border-radius: var(--r-tile);
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  text-align: center;
}

.empty-title {
  font-size: 12.75px;
  font-weight: 700;
}

.empty-title--bad {
  color: var(--ink-low);
}

.empty-detail {
  margin: 0;
  color: var(--muted);
  font-size: 11.475px;
  font-weight: 400;
  line-height: 15.3px;
}

.page-hint {
  position: absolute;
  top: 294.1px;
  right: 0;
  left: 0;
  height: 13.6px;
  margin: 0;
  color: var(--ink-2);
  font-size: 11.9px;
  line-height: 13.6px;
  text-align: center;
}

/* 未适配设备条（B 智能探测入口）：与 page-hint 同一排 */
.unknown-bar {
  position: absolute;
  top: 292.4px;
  right: 22.1px;
  left: 22.1px;
  display: flex;
  gap: 10.2px;
  align-items: center;
  height: 17px;
}

.unknown-text {
  overflow: hidden;
  color: var(--ink-2);
  font-size: 11.9px;
  line-height: 17px;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.unknown-link {
  flex: none;
  margin-left: auto;
  height: 17px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-family: var(--font-body);
  font-size: 11.05px;
  font-weight: 700;
  line-height: 17px;
  cursor: pointer;
}

.unknown-link:disabled {
  opacity: 0.5;
  cursor: default;
}

.field-label {
  position: absolute;
  left: 22.1px;
  height: 15.3px;
  color: var(--ink);
  font-size: 12.75px;
  font-weight: 700;
  line-height: 15.3px;
}

.field-label--protocol {
  top: 308.55px;
}

.protocol-toggle {
  position: absolute;
  top: 308.55px;
  left: 95.2px;
  height: 15.3px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-family: var(--font-body);
  font-size: 11.05px;
  font-weight: 700;
  line-height: 15.3px;
  cursor: pointer;
}

@media (hover: hover) {
  .protocol-toggle:hover {
    color: var(--txt);
  }
}

.field-label--name {
  top: 376.55px;
}

/* 已添加设备：一键把表单恢复成本次扫描的识别结果 */
.reset-link {
  position: absolute;
  top: 373.15px;
  right: 22.1px;
  height: 18.7px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-family: var(--font-body);
  font-size: 11.05px;
  font-weight: 700;
  line-height: 18.7px;
  cursor: pointer;
}

@media (hover: hover) {
  .reset-link:hover {
    color: var(--txt);
  }
}

.probe {
  position: absolute;
  top: 308.55px;
  right: 22.1px;
  display: inline-flex;
  gap: 5.1px;
  align-items: center;
  max-width: 323px;
  height: 15.3px;
  color: var(--ink-2);
  font-size: 11.9px;
  font-weight: 700;
  line-height: 15.3px;
}

.probe-dot {
  flex: none;
  width: 5.95px;
  height: 5.95px;
  border-radius: 50%;
  background: currentColor;
}

.probe-text {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.probe--ok {
  color: var(--ink-good);
}

.probe--bad {
  color: var(--ink-low);
}

.protocol-card {
  position: absolute;
  top: 326.4px;
  right: 22.1px;
  left: 22.1px;
  z-index: 60;
  display: flex;
  gap: 6.8px;
  align-items: center;
  height: 39.1px;
  padding: 0 5.1px;
  border: 1px solid var(--card-line);
  border-radius: 10.2px;
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
}






.name-box {
  position: absolute;
  top: 396.1px;
  right: 22.1px;
  left: 22.1px;
  display: flex;
  align-items: center;
  height: 30.6px;
  padding: 0 8.5px;
  border: 1px solid var(--card-line);
  border-radius: 8.5px;
  background: var(--card-bg);
  -webkit-backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  backdrop-filter: blur(var(--card-blur)) saturate(var(--card-sat));
  -webkit-mask-image: linear-gradient(#000 0 0);
  mask-image: linear-gradient(#000 0 0);
  transition:
    border-color var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.name-box:focus-within {
  border-color: var(--accent);
  box-shadow: var(--focus-ring);
}

.name-input {
  width: 100%;
  padding: 0;
  border: 0;
  outline: none;
  background: transparent;
  color: var(--txt);
  font-family: var(--font-body);
  font-size: 13.6px;
}

.name-input::placeholder {
  color: var(--muted);
}

.field-row {
  position: absolute;
  right: 22.1px;
  left: 22.1px;
  display: flex;
  align-items: center;
}

.field-row--color {
  top: 433.5px;
}

.field-row--icon {
  top: 484.5px;
}

/* 标签列宽固定：色盘与图标行都从 x=108 起，两块左右对齐。 */
.field-row .field-label {
  position: static;
  flex: none;
  width: 69.7px;
}

.save-error,
.save-hint {
  position: absolute;
  top: 537.2px;
  left: 22.1px;
  display: flex;
  align-items: center;
  width: 166.6px;
  height: 34px;
  margin: 0;
  overflow: hidden;
  font-size: 11.9px;
  line-height: 14.45px;
}

.save-error {
  color: var(--ink-low);
}

.save-hint {
  color: var(--ink-2);
}

.footer {
  position: absolute;
  top: 537.2px;
  right: 22.1px;
  left: 22.1px;
  display: flex;
  gap: 10.2px;
  align-items: center;
  justify-content: flex-end;
  height: 34px;
}

.footer-btn--cancel {
  width: 96.9px;
}

.footer-btn--save {
  width: 98.6px;
}

/* 批量添加按钮字更长，右移出一点宽度（取消按钮保持原位） */
.footer-btn--save-many {
  width: 112.2px;
}

/* PillButton 是共享组件；这里仅覆盖添加窗当前 1× 设计尺寸，base 仅兼容旧配置/迁移。 */
.add-window :deep(.pill) {
  --pill-h: 34px;

  padding: 0 15.3px;
  font-size: 13.6px;
}

.add-window :deep(.pill--sm) {
  --pill-h: 28.9px;

  padding: 0 13.6px;
  font-size: 12.325px;
  min-width: 91.8px;
}
</style>
