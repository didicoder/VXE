<script setup lang="ts">
/**
 * 设置页正文（视口 y=96 起、内容宽 816，由 MainWindow 提供页面容器）。
 *
 * 5 组设置卡 + 配置目录卡 + E1 快捷键行；所有写操作走 `useConfigStore` / `lib/ipc`，
 * 前端不做二次归一化（数值夹取以 Rust 为准）。`ui_scale` 只改待保存值，
 * 点绿色「保存」胶囊才落盘；其余缩放即时落盘。
 */

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import SettingRow from './SettingRow.vue';
import ConfigCard from './ConfigCard.vue';
import FoldSection from './FoldSection.vue';
import HotkeyRow from './HotkeyRow.vue';
import SegmentedControl from '../SegmentedControl.vue';
import StepperControl from '../StepperControl.vue';
import ToggleSwitch from '../ToggleSwitch.vue';
import PillButton from '../PillButton.vue';
import LineIcon from '../icons/LineIcon.vue';
import TrayStyleIcon from '../icons/TrayStyleIcon.vue';
import { useAlertsStore } from '../../stores/alerts';
import { useConfigStore } from '../../stores/config';
import { useUiStore } from '../../stores/ui';
import * as ipc from '../../lib/ipc';
import type { ThemeMode, TrayIconStyle, TrayLevelMode, UiStyle } from '../../types/config';

const config = useConfigStore();
const alerts = useAlertsStore();
const ui = useUiStore();

const settings = computed(() => config.settings);

const THEME_OPTIONS: { value: string; label: string }[] = [
  { value: 'manual', label: '手动' },
  { value: 'system', label: '跟随系统' },
  { value: 'schedule', label: '定时' },
];

const UI_STYLE_OPTIONS: { value: string; label: string }[] = [
  { value: 'photo', label: '实物卡片' },
  { value: 'classic', label: '经典卡片' },
];

const DISPLAY_OPTIONS: { value: string; label: string }[] = [
  { value: 'all', label: '显示全部' },
  { value: 'connected', label: '显示已连接' },
];

function onUiStyle(value: string): void {
  write('ui_style', value as UiStyle);
}

/** 顶层折叠：显示设备 / 页面缩放（与普通设置行同大）。 */
const displayFold = ref(false);
const scaleFold = ref(false);

// ---------- 低电量提醒 ----------

const ALERT_MODE_OPTIONS: { value: string; label: string }[] = [
  { value: 'popup', label: '弹窗提醒' },
  { value: 'tray', label: '托盘闪烁' },
  { value: 'both', label: '两者同时' },
];

/** 二级折叠：弹窗提醒 / 托盘图标提醒。 */
const popupFold = ref(false);
const trayFold = ref(false);

/** 切换提醒方式时收回已展开的折叠组（含被置灰的那种）。 */
watch(
  () => settings.value.alert_mode,
  () => {
    popupFold.value = false;
    trayFold.value = false;
  },
);

function formatBlinkSeconds(value: number): string {
  return `${value} 秒`;
}

/** 试一次当前样式的托盘红色闪烁；图标与时长都由 Rust 处理。 */
async function onTestTrayBlink(): Promise<void> {
  await ipc.testTrayBlink();
}

// ---------- 托盘图标取值方式 ----------

const TRAY_LEVEL_OPTIONS: { value: TrayLevelMode; label: string; expandable?: boolean }[] = [
  { value: 'average', label: '设备平均值' },
  { value: 'device', label: '自选设备', expandable: true },
  { value: 'min', label: '最低电量' },
];

const TRAY_STYLE_OPTIONS: {
  value: TrayIconStyle;
  label: string;
  preview: typeof TrayStyleIcon;
  previewProps: { kind: 'bolt' | 'ring' | 'battery' };
}[] = [
  { value: 'bolt', label: '闪电', preview: TrayStyleIcon, previewProps: { kind: 'bolt' } },
  { value: 'ring', label: '圆环', preview: TrayStyleIcon, previewProps: { kind: 'ring' } },
  { value: 'battery', label: '电池', preview: TrayStyleIcon, previewProps: { kind: 'battery' } },
];

const deviceChoices = computed(() => config.devices);
const trayDeviceExpand = ref(false);

function onTrayLevelMode(value: string): void {
  const mode = value as TrayLevelMode;
  trayDeviceExpand.value = mode === 'device';
  const patch: { tray_level_mode: TrayLevelMode; tray_level_device?: string } = {
    tray_level_mode: mode,
  };
  if (mode === 'device' && !settings.value.tray_level_device) {
    const first = deviceChoices.value[0];
    if (first) patch.tray_level_device = first.id;
  }
  void config.setSettings(patch);
}

/** 点「自选设备」选项本体即展开设备胶囊；已是该选项时不会再发 update，靠 select 兜底。 */
function onTraySelect(value: string): void {
  if (value === 'device') trayDeviceExpand.value = true;
}

/** 收纳图标：切到自选设备并展开设备胶囊；已展开时再点收起。 */
function onTrayExpand(): void {
  const open = settings.value.tray_level_mode === 'device' && trayDeviceExpand.value;
  if (settings.value.tray_level_mode !== 'device') onTrayLevelMode('device');
  trayDeviceExpand.value = !open;
}

/** 自选设备被删除后自动落到第一台，避免托盘停留在旧选择。 */
watch(deviceChoices, (devices) => {
  if (settings.value.tray_level_mode !== 'device') return;
  if (!devices.length) return;
  if (devices.some((device) => device.id === settings.value.tray_level_device)) return;
  write('tray_level_device', devices[0].id);
});

// ---------- 写配置 ----------

function write(key: string, value: unknown): void {
  void config.setSetting(key, value);
}

function onThemeMode(value: string): void {
  write('theme_mode', value as ThemeMode);
}

// ---------- 自定义背景 ----------

function openBackgroundEditor(): void {
  void ipc.showBackgroundEditor();
}

// ---------- 数值格式 ----------

const formatSeconds = (value: number): string => `${value} 秒`;
const formatTenths = (value: number): string => `${value.toFixed(1)} 秒`;
const formatPercent = (value: number): string => `${value}%`;
const formatScale = (value: number): string => `${value.toFixed(2)}×`;
const formatNotifyInterval = (value: number): string =>
  value <= 0 ? '仅一次' : `${value} 分钟`;

// ---------- 界面缩放（待保存） ----------

const pendingScale = ref<number | null>(null);

const scaleValue = computed(() => pendingScale.value ?? settings.value.ui_scale);
const scaleDirty = computed(
  () => pendingScale.value !== null && pendingScale.value !== settings.value.ui_scale,
);

function onScaleInput(value: number): void {
  pendingScale.value = Math.round(value * 100) / 100;
}

async function saveScale(): Promise<void> {
  const value = pendingScale.value;
  if (value === null) return;
  await config.setSetting('ui_scale', value);
  pendingScale.value = null;
}

/** 点到「界面缩放」行以外的任何地方即丢弃待保存值。 */
function onWindowPointerDown(event: PointerEvent): void {
  if (pendingScale.value === null) return;
  const target = event.target;
  if (target instanceof Element && target.closest('[data-scale-pending]')) return;
  pendingScale.value = null;
}

// ---------- 缩放行即时值 ----------

function onPopupScale(value: number): void {
  write('popup_scale', value);
}

function onTipScale(value: number): void {
  write('tip_scale', value);
}

function onAlertScale(value: number): void {
  write('alert_scale', value);
}

/** 悬停整行预览：进入常驻、离开立即收起（hover 之间移动不重复触发）。 */
let popupPreviewOn = false;
let tipPreviewOn = false;
let alertPreviewOn = false;

function setPopupPreview(active: boolean): void {
  if (active === popupPreviewOn) return;
  popupPreviewOn = active;
  void ui.previewPopup(active ? 'hold' : 'hide');
}

function setTipPreview(active: boolean): void {
  if (active === tipPreviewOn) return;
  tipPreviewOn = active;
  void ui.previewTooltip(active ? 'hold' : 'hide');
}

function setAlertPreview(active: boolean): void {
  if (active === alertPreviewOn) return;
  alertPreviewOn = active;
  void ui.previewAlert(active ? 'hold' : 'hide');
}

// ---------- 时间文本 ----------

function normalizeClock(raw: string): string | null {
  const match = /^(\d{1,2})\s*[:：.]\s*(\d{1,2})$/.exec(raw.trim());
  if (!match) return null;
  const hour = Number(match[1]);
  const minute = Number(match[2]);
  if (!Number.isInteger(hour) || !Number.isInteger(minute)) return null;
  if (hour < 0 || hour > 23 || minute < 0 || minute > 59) return null;
  return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`;
}

const lightAt = ref(settings.value.theme_light_at);
const darkAt = ref(settings.value.theme_dark_at);

watch(() => settings.value.theme_light_at, (value) => { lightAt.value = value; });
watch(() => settings.value.theme_dark_at, (value) => { darkAt.value = value; });

function commitClock(key: 'theme_light_at' | 'theme_dark_at'): void {
  const source = key === 'theme_light_at' ? lightAt : darkAt;
  const next = normalizeClock(source.value);
  if (!next) {
    source.value = settings.value[key];
    return;
  }
  source.value = next;
  if (next !== settings.value[key]) write(key, next);
}

// ---------- 弹窗文案 ----------

const alertTitle = ref(settings.value.alert_title);
const alertSubtitle = ref(settings.value.alert_subtitle);

watch(() => settings.value.alert_title, (value) => { alertTitle.value = value; });
watch(() => settings.value.alert_subtitle, (value) => { alertSubtitle.value = value; });

function commitTitle(): void {
  if (alertTitle.value === settings.value.alert_title) return;
  write('alert_title', alertTitle.value);
}

function commitSubtitle(): void {
  if (alertSubtitle.value === settings.value.alert_subtitle) return;
  write('alert_subtitle', alertSubtitle.value);
}

// ---------- 提示音 ----------

const soundIndex = computed(() => {
  const index = alerts.soundFiles.indexOf(settings.value.alert_sound_file);
  return index < 0 ? 0 : index;
});

const soundLabel = computed(() => {
  const files = alerts.soundFiles;
  if (!files.length) return '系统提示音';
  return files[soundIndex.value] ?? files[0];
});

function formatSound(): string {
  return soundLabel.value;
}

/** ± / 滚轮在文件列表里循环（越过首尾回绕），切换后立即试听。 */
function stepSound(value: number): void {
  const files = alerts.soundFiles;
  if (!files.length) return;
  const count = files.length;
  const next = ((value % count) + count) % count;
  write('alert_sound_file', files[next]);
  void ipc.previewSound(files[next]);
}

// ---------- 提醒弹窗 ----------

const alertPosLabel = computed(() => (settings.value.alert_pos ? '已自定义' : '底部居中'));

function onTestAlert(): void {
  void alerts.testAlert({ device: '设备', battery: 20 });
}

function onStartAlertPos(): void {
  void alerts.startAlertPosition();
}

async function onResetAlertPos(): Promise<void> {
  try {
    await alerts.resetAlertPosition();
    await config.load();
  } catch {
    // 定位模式取消失败不阻断设置页
  }
}

// ---------- 配置目录 ----------

const configPath = ref('');
const resetting = ref(false);

async function onResetSettings(): Promise<void> {
  if (!window.confirm('恢复默认设置？设备列表会保留。')) return;
  resetting.value = true;
  try {
    await ipc.resetSettings();
    await config.load();
  } catch {
    // 失败沿用旧配置，错误由 store 暴露
  } finally {
    resetting.value = false;
  }
}

async function onOpenConfigDir(): Promise<void> {
  try {
    await ipc.openConfigDir();
  } catch {
    // 资源管理器起不来时静默
  }
}

// ---------- 快捷键（E1） ----------

const hotkeyDraft = ref(settings.value.hotkey);
const hotkeyError = ref<string | null>(null);
const hotkeyApplying = ref(false);

watch(() => settings.value.hotkey, (value) => { hotkeyDraft.value = value; });

async function applyHotkey(): Promise<void> {
  const accelerator = hotkeyDraft.value.trim();
  if (!accelerator) {
    hotkeyError.value = '请先录制或输入组合键';
    return;
  }
  hotkeyApplying.value = true;
  hotkeyError.value = null;
  const previous = settings.value.hotkey;
  try {
    const ok = await ipc.registerHotkey(accelerator);
    if (!ok) {
      hotkeyError.value = '该快捷键被占用或格式无效';
      if (settings.value.hotkey_enabled) await ipc.registerHotkey(previous);
      return;
    }
    if (accelerator !== previous) await config.setSetting('hotkey', accelerator);
    if (!settings.value.hotkey_enabled) await config.setSetting('hotkey_enabled', true);
  } catch (cause) {
    hotkeyError.value = ipc.errorMessage(cause);
  } finally {
    hotkeyApplying.value = false;
  }
}

async function onHotkeyToggle(enabled: boolean): Promise<void> {
  hotkeyError.value = null;
  if (!enabled) {
    await config.setSetting('hotkey_enabled', false);
    try {
      await ipc.unregisterHotkey();
    } catch (cause) {
      hotkeyError.value = ipc.errorMessage(cause);
    }
    return;
  }
  await config.setSetting('hotkey_enabled', true);
  const accelerator = hotkeyDraft.value.trim();
  try {
    const ok = accelerator ? await ipc.registerHotkey(accelerator) : true;
    if (!ok) {
      hotkeyError.value = '该快捷键被占用，请换一个';
      await config.setSetting('hotkey_enabled', false);
    } else if (accelerator && accelerator !== settings.value.hotkey) {
      await config.setSetting('hotkey', accelerator);
    }
  } catch (cause) {
    hotkeyError.value = ipc.errorMessage(cause);
  }
}

// ---------- 滚动（滚动条样式走全局 .fade-scroll，这里只收浮层预览） ----------

function onScroll(): void {
  if (popupPreviewOn) setPopupPreview(false);
  if (tipPreviewOn) setTipPreview(false);
  if (alertPreviewOn) setAlertPreview(false);
}

// ---------- 生命周期 ----------

onMounted(async () => {
  window.addEventListener('pointerdown', onWindowPointerDown, true);

  await alerts.loadSoundFiles();
  try {
    configPath.value = await ipc.getConfigPath();
  } catch {
    configPath.value = '';
  }
});

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onWindowPointerDown, true);
  setPopupPreview(false);
  setTipPreview(false);
  setAlertPreview(false);
});
</script>

<template>
  <section class="settings" aria-label="设置">
    <div class="settings__viewport fade-scroll" @scroll="onScroll">
      <div class="settings__content">
        <h2 class="settings__title">外观与尺寸</h2>
        <div class="settings__card glass-card">
          <SettingRow
            label="界面风格"
            hint="实物卡片：设备实物图海报卡；经典卡片：3.0 的玻璃卡片"
          >
            <SegmentedControl
              :model-value="settings.ui_style"
              :options="UI_STYLE_OPTIONS"
              @update:model-value="onUiStyle"
            />
          </SettingRow>

          <SettingRow label="颜色模式" hint="浅色 / 深色的切换方式：手动 / 跟随 Windows / 按时间">
            <SegmentedControl
              :model-value="settings.theme_mode"
              :options="THEME_OPTIONS"
              @update:model-value="onThemeMode"
            />
          </SettingRow>

          <SettingRow
            v-if="settings.theme_mode === 'schedule'"
            label="浅色时段开始"
            hint="每天此时刻起切换到浅色"
          >
            <input
              v-model="lightAt"
              class="field"
              type="text"
              inputmode="numeric"
              maxlength="5"
              aria-label="浅色时段开始时间"
              @change="commitClock('theme_light_at')"
              @keydown.enter="($event.target as HTMLInputElement).blur()"
            >
          </SettingRow>

          <SettingRow
            v-if="settings.theme_mode === 'schedule'"
            label="深色时段开始"
            hint="每天此时刻起切换到深色"
          >
            <input
              v-model="darkAt"
              class="field"
              type="text"
              inputmode="numeric"
              maxlength="5"
              aria-label="深色时段开始时间"
              @change="commitClock('theme_dark_at')"
              @keydown.enter="($event.target as HTMLInputElement).blur()"
            >
          </SettingRow>

          <FoldSection
            title="显示设备"
            size="row"
            :expanded="displayFold"
            @toggle="displayFold = !displayFold"
          >
            <SettingRow label="总览显示设备" hint="显示全部：含未连接设备；显示已连接：只列接在本机上的">
              <SegmentedControl
                :model-value="settings.display_summary"
                :options="DISPLAY_OPTIONS"
                @update:model-value="write('display_summary', $event)"
              />
            </SettingRow>

            <SettingRow label="设备页显示设备" hint="显示全部：含未连接设备；显示已连接：只列接在本机上的">
              <SegmentedControl
                :model-value="settings.display_devices"
                :options="DISPLAY_OPTIONS"
                @update:model-value="write('display_devices', $event)"
              />
            </SettingRow>

            <SettingRow label="预览卡片显示设备" hint="托盘左键小窗的设备列表，最多显示 3 台">
              <SegmentedControl
                :model-value="settings.display_popup"
                :options="DISPLAY_OPTIONS"
                @update:model-value="write('display_popup', $event)"
              />
            </SettingRow>

            <SettingRow label="悬停卡片显示设备" hint="托盘悬停卡的设备列表，最多显示 6 行">
              <SegmentedControl
                :model-value="settings.display_tip"
                :options="DISPLAY_OPTIONS"
                @update:model-value="write('display_tip', $event)"
              />
            </SettingRow>
          </FoldSection>

          <FoldSection
            title="页面缩放"
            size="row"
            :expanded="scaleFold"
            @toggle="scaleFold = !scaleFold"
          >
            <SettingRow label="主面板缩放" hint="1× 为 832×576；总览 / 设备 / 设置共用，点「保存」生效">
              <div class="scale" data-scale-pending>
                <button
                  v-if="scaleDirty"
                  type="button"
                  class="scale__save"
                  @click="saveScale"
                >
                  <LineIcon name="check" :size="13" :stroke="2.3" />
                  <span>保存</span>
                </button>
                <StepperControl
                  :model-value="scaleValue"
                  :min="0.5"
                  :max="2"
                  :step="0.05"
                  :format="formatScale"
                  @update:model-value="onScaleInput"
                />
              </div>
            </SettingRow>

            <SettingRow
              label="预览卡片缩放"
              hint="托盘左键弹出的电量卡片大小，鼠标停在这行即时预览"
              @hover-start="setPopupPreview(true)"
              @hover-end="setPopupPreview(false)"
            >
              <StepperControl
                :model-value="settings.popup_scale"
                :min="0.5"
                :max="2"
                :step="0.05"
                :format="formatScale"
                @update:model-value="onPopupScale"
              />
            </SettingRow>

            <SettingRow
              label="悬停卡片缩放"
              hint="托盘图标悬停显示的卡片大小，鼠标停在这行即时预览"
              @hover-start="setTipPreview(true)"
              @hover-end="setTipPreview(false)"
            >
              <StepperControl
                :model-value="settings.tip_scale"
                :min="0.5"
                :max="2"
                :step="0.05"
                :format="formatScale"
                @update:model-value="onTipScale"
              />
            </SettingRow>
          </FoldSection>

          <SettingRow
            label="界面背景"
            hint="各窗口共用；缩放、偏移、磨砂、暗化与旋转在弹窗里调整"
          >
            <PillButton size="sm" variant="soft" @click="openBackgroundEditor">
              自定义背景
            </PillButton>
          </SettingRow>
        </div>

        <h2 class="settings__title">
          刷新
          <span class="settings__title-hint">滚轮可调</span>
        </h2>
        <div class="settings__card glass-card">
          <SettingRow label="电量刷新间隔" hint="越短越及时，也更费电">
            <StepperControl
              :model-value="settings.refresh_seconds"
              :min="15"
              :max="600"
              :step="15"
              :format="formatSeconds"
              @update:model-value="write('refresh_seconds', $event)"
            />
          </SettingRow>
          <SettingRow label="悬停提示延迟" hint="鼠标停在托盘图标上多久弹出">
            <StepperControl
              :model-value="settings.hover_delay"
              :min="0"
              :max="2"
              :step="0.1"
              :format="formatTenths"
              @update:model-value="write('hover_delay', $event)"
            />
          </SettingRow>
        </div>

        <h2 class="settings__title">
          电量区间
          <span class="settings__title-hint">滚轮可调</span>
        </h2>
        <div class="settings__card glass-card">
          <SettingRow label="电量充足" hint="高于此值算电量充足">
            <StepperControl
              :model-value="settings.good_level"
              :min="Math.min(100, settings.warn_level + 5)"
              :max="100"
              :step="5"
              :format="formatPercent"
              @update:model-value="write('good_level', $event)"
            />
          </SettingRow>
          <SettingRow label="正常使用" hint="介于两值之间算正常使用" />
          <SettingRow label="电量不足" hint="低于此值算电量不足">
            <StepperControl
              :model-value="settings.warn_level"
              :min="0"
              :max="Math.max(0, settings.good_level - 5)"
              :step="5"
              :format="formatPercent"
              @update:model-value="write('warn_level', $event)"
            />
          </SettingRow>
        </div>

        <h2 class="settings__title">低电量提醒</h2>
        <div class="settings__card glass-card">
          <SettingRow label="低电量通知" hint="低于设定数值时按提醒方式通知；持续低电量按通知时间重复提醒">
            <ToggleSwitch
              :model-value="settings.notify_low"
              @update:model-value="write('notify_low', $event)"
            />
          </SettingRow>
          <SettingRow label="通知阈值" hint="电量等于或低于此值算低电量并触发提醒">
            <StepperControl
              :model-value="settings.notify_level"
              :min="5"
              :max="50"
              :step="5"
              :format="formatPercent"
              @update:model-value="write('notify_level', $event)"
            />
          </SettingRow>
          <SettingRow label="通知时间" hint="持续低电量时，每隔多久再提醒一次；0 = 仅提醒一次">
            <StepperControl
              :model-value="settings.notify_interval_minutes"
              :min="0"
              :max="120"
              :step="5"
              :format="formatNotifyInterval"
              @update:model-value="write('notify_interval_minutes', $event)"
            />
          </SettingRow>
          <SettingRow label="提醒方式" hint="弹窗提醒 / 托盘图标提醒，或两者同时提醒">
            <SegmentedControl
              :model-value="settings.alert_mode"
              :options="ALERT_MODE_OPTIONS"
              @update:model-value="write('alert_mode', $event)"
            />
          </SettingRow>

          <FoldSection
            title="弹窗提醒"
            :expanded="popupFold"
            :disabled="settings.alert_mode === 'tray'"
            @toggle="popupFold = !popupFold"
          >
            <SettingRow
              label="提醒弹窗缩放"
              hint="只影响低电量弹窗的大小，鼠标停在这行即时预览"
              size="sub"
              @hover-start="setAlertPreview(true)"
              @hover-end="setAlertPreview(false)"
            >
              <StepperControl
                :model-value="settings.alert_scale"
                :min="0.5"
                :max="2"
                :step="0.05"
                :format="formatScale"
                @update:model-value="onAlertScale"
              />
            </SettingRow>
            <SettingRow label="启用提示音" hint="内置提示音随软件打包；sound 文件夹里的同名 wav 优先" size="sub">
              <ToggleSwitch
                :model-value="settings.alert_sound"
                @update:model-value="write('alert_sound', $event)"
              />
            </SettingRow>
            <SettingRow label="提示音文件" hint="内置 3 个提示音；放同名 wav 到 sound 文件夹可自定义覆盖" size="sub">
              <div class="stepper-wide">
                <StepperControl
                  :model-value="soundIndex"
                  :step="1"
                  :format="formatSound"
                  @update:model-value="stepSound"
                />
              </div>
            </SettingRow>
            <SettingRow label="弹窗标题" hint="留空用默认；支持 {device} / {battery}" size="sub">
              <input
                v-model="alertTitle"
                class="field"
                type="text"
                placeholder="默认：电量不足"
                aria-label="弹窗标题"
                @change="commitTitle"
                @keydown.enter="($event.target as HTMLInputElement).blur()"
              >
            </SettingRow>
            <SettingRow label="弹窗副标题" hint="留空用默认模板" size="sub">
              <input
                v-model="alertSubtitle"
                class="field"
                type="text"
                placeholder="默认：{device} 剩余 {battery}%"
                aria-label="弹窗副标题"
                @change="commitSubtitle"
                @keydown.enter="($event.target as HTMLInputElement).blur()"
              >
            </SettingRow>
            <SettingRow label="测试弹窗" hint="立即弹一次示例，不改变电量状态" size="sub">
              <PillButton size="sm" :disabled="alerts.testing" @click="onTestAlert">
                测试
              </PillButton>
            </SettingRow>
            <SettingRow label="弹窗位置" hint="方向键移动，Enter 保存，Esc 取消" size="sub">
              <div class="pos">
                <span class="pos__value">{{ alertPosLabel }}</span>
                <button type="button" class="pos__button" @click="onResetAlertPos">
                  居中
                </button>
                <button type="button" class="pos__button" @click="onStartAlertPos">
                  定位
                </button>
              </div>
            </SettingRow>
          </FoldSection>

          <FoldSection
            title="托盘图标提醒"
            :expanded="trayFold"
            :disabled="settings.alert_mode === 'popup'"
            @toggle="trayFold = !trayFold"
          >
            <SettingRow label="闪烁时长" hint="低电量时托盘图标变红并闪烁的持续时间" size="sub">
              <StepperControl
                :model-value="settings.tray_blink_seconds"
                :min="5"
                :max="120"
                :step="5"
                :format="formatBlinkSeconds"
                @update:model-value="write('tray_blink_seconds', $event)"
              />
            </SettingRow>
            <SettingRow label="测试" hint="立即试一次托盘图标闪烁" size="sub">
              <PillButton size="sm" @click="onTestTrayBlink">
                测试
              </PillButton>
            </SettingRow>
          </FoldSection>

          <SettingRow label="托盘图标取值方式" hint="托盘图标显示哪一路电量">
            <SegmentedControl
              :model-value="settings.tray_level_mode"
              :options="TRAY_LEVEL_OPTIONS"
              :expanded="settings.tray_level_mode === 'device' && trayDeviceExpand ? 'device' : null"
              @update:model-value="onTrayLevelMode"
              @select="onTraySelect"
              @toggle-expand="onTrayExpand"
            />
          </SettingRow>
          <div
            v-if="settings.tray_level_mode === 'device' && trayDeviceExpand"
            class="tray-devices"
          >
            <button
              v-for="device in deviceChoices"
              :key="device.id"
              type="button"
              class="tray-devices__pill"
              :class="{ 'tray-devices__pill--active': device.id === settings.tray_level_device }"
              @click="write('tray_level_device', device.id)"
            >
              {{ device.label }}
            </button>
            <span v-if="!deviceChoices.length" class="tray-devices__empty">
              还没有添加设备
            </span>
          </div>
          <SettingRow label="托盘图标样式" hint="托盘图标外观：闪电 / 圆环 / 电池">
            <SegmentedControl
              :model-value="settings.tray_icon_style"
              :options="TRAY_STYLE_OPTIONS"
              @update:model-value="write('tray_icon_style', $event)"
            />
          </SettingRow>
        </div>

        <h2 class="settings__title">启动与配置</h2>
        <div class="settings__card glass-card">
          <SettingRow label="开机自动启动" hint="登录 Windows 后静默运行在托盘">
            <ToggleSwitch
              :model-value="settings.autostart"
              @update:model-value="write('autostart', $event)"
            />
          </SettingRow>
        </div>

        <ConfigCard
          class="settings__cfg"
          :path="configPath"
          :busy="resetting"
          @reset="onResetSettings"
          @open="onOpenConfigDir"
        />

        <h2 class="settings__title">快捷键</h2>
        <div class="settings__card glass-card">
          <HotkeyRow
            :enabled="settings.hotkey_enabled"
            :model-value="hotkeyDraft"
            :error="hotkeyError"
            :applying="hotkeyApplying"
            @update:enabled="onHotkeyToggle"
            @update:model-value="hotkeyDraft = $event"
            @apply="applyHotkey"
          />
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.settings {
  position: relative;
  width: 100%;
  max-width: 816px;
  height: 100%;
}

.settings__viewport {
  width: 100%;
  height: 100%;
  padding-bottom: 8px;
  overflow-x: hidden;
  overflow-y: auto;
  border-radius: var(--r-card);
  outline: none;
}

.settings__content {
  padding-top: 0;
}

.settings__title {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 22px 0 12px;
  padding-top: 3.5px;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: var(--fs-h2);
  font-weight: 700;
  line-height: var(--fs-h2);
  letter-spacing: -0.01em;
}

.settings__content > .settings__title:first-child {
  margin-top: 0;
}

.settings__title-hint {
  color: var(--ink-3);
  font-size: 13px;
  font-weight: 400;
  line-height: 14px;
}

/* 设置组卡：表面走 .glass-card */
.settings__card {
  padding: 18px 16px 18px 24px;
}

.settings__cfg {
  margin-top: 12px;
}

.settings :deep(.pill--sm) {
  width: 128px;
  min-width: 128px;
  height: 34px;
  font-size: 15px;
}

/* 文本框：364×44、r-ctrl、1px 描边、15 body、内缩 12 */
.field {
  width: 364px;
  height: 44px;
  padding: 0 12px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 400;
  line-height: 1;
  outline: none;
  transition:
    border-color var(--speed) var(--ease),
    box-shadow var(--speed) var(--ease);
}

.field::placeholder {
  color: var(--ink-4);
}

.field:focus {
  border-color: var(--focus);
  box-shadow: var(--focus-shadow);
}

/* 界面缩放：保存胶囊贴数值框左侧 */
.scale {
  display: flex;
  align-items: center;
  gap: 10px;
}

.scale__save {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 64px;
  height: 34px;
  padding: 0;
  border: 0;
  border-radius: var(--r-pill);
  background: var(--accent);
  box-shadow: inset 0 1px 0 rgb(255 255 255 / 16%);
  color: var(--accent-fg);
  font-family: var(--font-body);
  font-size: 14px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
  transition: filter var(--dur-fast) var(--ease-out);
}

@media (hover: hover) {
  .scale__save:hover {
    filter: brightness(1.06);
  }
}

/* 提示音文件名比 150 宽，略放宽步进器 */
.stepper-wide :deep(.stepper) {
  width: 210px;
}

/* 弹窗位置：值框 + 居中 + 定位 */
.pos {
  display: flex;
  align-items: center;
  gap: 10px;
}

.pos__value,
.pos__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 74px;
  height: 34px;
  padding: 0 18px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 15px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

.pos__button {
  width: 128px;
  min-width: 128px;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    transform var(--speed) var(--ease);
}

@media (hover: hover) {
  .pos__button:hover {
    background: var(--ctrl-bg-hover);
    transform: translateY(-1px);
  }
}

/* 自选设备胶囊：收纳图标展开后的设备选择区（二级行下方） */
.tray-devices {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  padding: 0 4px 14px;
}

.tray-devices__pill {
  height: 30px;
  padding: 0 14px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    color var(--speed) var(--ease);
}

.tray-devices__pill--active {
  border-color: var(--seg-on-line);
  background: var(--seg-on-bg);
  box-shadow: var(--seg-on-shadow);
}

@media (hover: hover) {
  .tray-devices__pill:not(.tray-devices__pill--active):hover {
    background: var(--ctrl-bg-hover);
    border-color: var(--ctrl-line-hover);
  }
}

.tray-devices__empty {
  color: var(--ink-3);
  font-size: 13px;
  line-height: 14px;
}
</style>
