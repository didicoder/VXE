<script setup lang="ts">
/**
 * 托盘左键弹出的预览小窗：设计宽 345，设计尺寸即 1×，当前只乘 popup_scale。
 * base 仅用于旧配置兼容/迁移，不参与当前尺寸。
 *
 * 头部 49.5、Hero 102、分区 25.5、ROW=54/GAP=6（最多 3 台）、FOOT=33、BOTTOM=12。
 * 弹出后一直保持，直到点击卡片外（窗口失焦）；再次左键点托盘图标会重新刷新并保持。
 */
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { storeToRefs } from 'pinia';
import HeroRing from '../components/summary/HeroRing.vue';
import PopupFooter from '../components/popup/PopupFooter.vue';
import PopupHeader from '../components/popup/PopupHeader.vue';
import PopupRow from '../components/popup/PopupRow.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWindowMotion } from '../composables/useWindowMotion';
import { useWindowScale } from '../composables/useWindowScale';
import { bandOf } from '../lib/bands';
import { filterStatusesByDisplay } from '../lib/deviceCard';
import { percent } from '../lib/format';
import * as ipc from '../lib/ipc';
import { initializeWindow } from '../lib/windowLifecycle';
import { stats } from '../lib/state';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';
import type { BatteryBand } from '../types/battery';

const props = withDefaults(defineProps<{ windowLabel?: string; active?: boolean; managed?: boolean }>(), {
  active: true,
  managed: false,
});
const emitReady = defineEmits<{ ready: [] }>();

/** 小窗最多显示 3 台，其余折成「还有 N 台」。 */
const MAX_ROWS = 3;
/** 失焦后等一小段时间再收起，给「再点托盘」的 click 留出取消窗口。 */
const BLUR_HIDE_MS = 180;
/** 主窗刷新请求的兜底超时：失联时不让刷新按钮一直转。 */
const REFRESH_FALLBACK_MS = 10_000;

/** 整体电量环只看三档本体色（V16 `widgets.summary_hero` 用 `level_color`，不吃设备配色）。 */
const BAND_TOKEN: Record<BatteryBand, string> = {
  good: 'var(--good)',
  mid: 'var(--mid)',
  low: 'var(--low)',
};

const config = useConfigStore();
const devices = useDevicesStore();
const { settings } = storeToRefs(config);
const { statuses } = storeToRefs(devices);
const hold = ref(false);
const requested = ref(false);

const scale = useWindowScale('popup');
useThemeClass();
const { entering, leaving, enter, leave } = useWindowMotion();

const shown = computed(() =>
  filterStatusesByDisplay(statuses.value, settings.value.display_popup),
);
const visible = computed(() => shown.value.slice(0, MAX_ROWS));
const more = computed(() => Math.max(0, shown.value.length - visible.value.length));
const summary = computed(() => stats(statuses.value));

const heroColor = computed(() => {
  if (summary.value.total === 0) return 'var(--faint)';
  return BAND_TOKEN[bandOf(summary.value.avg, settings.value.good_level, settings.value.warn_level)];
});
const heroTone = computed(() =>
  summary.value.total > 0
    ? bandOf(summary.value.avg, settings.value.good_level, settings.value.warn_level)
    : 'none',
);
const onlineDot = computed(() => (summary.value.online > 0 ? 'var(--good)' : 'var(--gray)'));
const minDot = computed(() => {
  const min = summary.value.min;
  if (min == null) return 'var(--gray)';
  return BAND_TOKEN[bandOf(min, settings.value.good_level, settings.value.warn_level)];
});
const minText = computed(() => percent(summary.value.min));

/** 浏览器预览（无 Tauri 运行时）拿不到窗口句柄，降级为 null，不影响 Tauri 侧。 */
const popupWindow = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
})();
let open = false;
let blurTimer: ReturnType<typeof setTimeout> | null = null;
let blurGeneration = 0;

function cancelBlurHide(): void {
  blurGeneration += 1;
  if (blurTimer !== null) {
    clearTimeout(blurTimer);
    blurTimer = null;
  }
}

function hideWindow(): void {
  if (props.active === false) return;
  open = false;
  cancelBlurHide();
  void leave(async () => {
    if (props.active === false) return;
    try {
      await popupWindow?.hide();
    } catch {
      // 浏览器预览（无 Tauri 运行时）没有窗口 API，忽略。
    }
  });
}

/**
 * 点击卡片外（窗口失焦）后收起。延迟里若窗口重新拿到焦点
 * （例如再点托盘触发的 show + set_focus），就取消这次收起。
 */
function scheduleBlurHide(): void {
  cancelBlurHide();
  const generation = blurGeneration;
  blurTimer = setTimeout(async () => {
    blurTimer = null;
    let focused = false;
    try {
      focused = !!popupWindow && await popupWindow.isFocused();
    } catch {
      // 取不到焦点状态就按失焦处理
    }
    if (generation !== blurGeneration || props.active === false || hold.value || focused) return;
    hideWindow();
  }, BLUR_HIDE_MS);
}

let requestedTimer: ReturnType<typeof setTimeout> | null = null;

function clearRequested(): void {
  requested.value = false;
  if (requestedTimer !== null) {
    clearTimeout(requestedTimer);
    requestedTimer = null;
  }
}

/** 预览卡只请求 Rust 采集，主窗可见时由开始事件驱动圆形遮罩。 */
function requestRefresh(): void {
  clearRequested();
  requested.value = true;
  requestedTimer = setTimeout(clearRequested, REFRESH_FALLBACK_MS);
  void emit(ipc.REFRESH_REQUEST_EVENT).catch((cause) => {
    clearRequested();
    console.error('刷新请求失败', cause);
  });
}

function refresh(): void {
  requestRefresh();
}

function openAdd(): void {
  hideWindow();
  void ipc.showAdd();
}

function openPanel(): void {
  hideWindow();
  void ipc.showMain('summary');
}

function openDevices(): void {
  hideWindow();
  void ipc.showMain('devices');
}

let unlistenUpdated: UnlistenFn | null = null;
let unlistenOpen: UnlistenFn | null = null;
let unlistenClose: UnlistenFn | null = null;
let unlistenFocus: UnlistenFn | null = null;
let unlistenDone: UnlistenFn | null = null;

async function initialize(): Promise<void> {
  unlistenUpdated = await listen('devices-updated', clearRequested);
  unlistenDone = await listen(ipc.REFRESH_DONE_EVENT, () => {
    clearRequested();
  });
  unlistenFocus =
    (await popupWindow?.onFocusChanged(({ payload: focused }) => {
      if (!focused && open && !hold.value) scheduleBlurHide();
    })) ?? null;
  // 菜单/设置页在 Rust 侧直接 hide 时复位，避免下次弹出漏播入场。
  unlistenClose = await listen('popup-close', () => {
    if (props.active === false) return;
    open = false;
    cancelBlurHide();
  });
  unlistenOpen = await listen<boolean>('popup-open', (event) => {
    if (props.active === false) return;
    hold.value = event.payload === true;
    cancelBlurHide();
    if (!open) {
      open = true;
      void enter();
    }
    void devices.loadCached();
    // 设置页的悬停预览（hold）只为了看尺寸：不请求主窗整页刷新，
    // 否则刷新遮罩会盖住悬停行，把预览挤掉后又重新触发，形成开关循环。
    if (!hold.value) requestRefresh();
  });
  await config.load();
  await devices.loadCached();
}

onMounted(() => {
  if (props.managed) {
    void initialize().then(() => emitReady('ready'), () => emitReady('ready'));
    return;
  }
  void initializeWindow(initialize);
});

onUnmounted(() => {
  cancelBlurHide();
  clearRequested();
  unlistenUpdated?.();
  unlistenOpen?.();
  unlistenClose?.();
  unlistenFocus?.();
  unlistenDone?.();
});
</script>

<template>
  <section
    class="popup glass-window"
    :class="{ 'window-enter': entering, 'window-leave': leaving }"
    :style="{ '--s': scale }"
  >
    <PopupHeader
      :busy="requested"
      @refresh="refresh"
      @close="hideWindow"
    />

    <div class="hero">
      <div class="hero__ring">
        <HeroRing
          :level="summary.avg"
          :size="81.75"
          :thickness="6.3"
          :orb-size="37.5"
          :color="heroColor"
          :tone="heroTone"
        />
      </div>
      <p class="hero__value">
        <span class="hero__num">{{ summary.avg }}</span>
        <span class="hero__pct" :style="{ color: heroColor }">%</span>
      </p>
      <p class="hero__online">{{ summary.online }} / {{ summary.total }} 台设备在线</p>
      <ul class="hero__side">
        <li class="hero__stat">
          <i class="hero__dot" :style="{ background: onlineDot }" />
          <span class="hero__label">在线设备</span>
          <span class="hero__val">{{ summary.online }} 台</span>
        </li>
        <li class="hero__stat">
          <i class="hero__dot" :style="{ background: minDot }" />
          <span class="hero__label">最低电量</span>
          <span class="hero__val">{{ minText }}</span>
        </li>
      </ul>
    </div>

    <div class="section">
      <h2 class="section__title">设备</h2>
      <button type="button" class="section__add" @click="openAdd">+ 添加</button>
    </div>

    <div class="rows">
      <PopupRow
        v-for="device in visible"
        :key="device.id"
        :device="device"
        :good="settings.good_level"
        :warn="settings.warn_level"
        :band-colors="settings.band_colors"
        @select="openDevices"
      />
      <button v-if="!visible.length" type="button" class="empty" @click="openAdd">
        <span class="empty__title">还没有添加设备</span>
        <span class="empty__hint">点下面的「添加设备」开始</span>
      </button>
    </div>

    <button v-if="more > 0" type="button" class="more" @click="openPanel">
      还有 {{ more }} 台设备 · 打开面板查看
    </button>

    <PopupFooter
      @add="openAdd"
      @open="openPanel"
    />
  </section>
</template>

<style scoped>
/* 预览小窗设计宽 345（1×）：壁纸走 .glass-window，当前只乘 popup_scale */
.popup {
  width: 345px;
  height: auto;
  padding-bottom: 12px;
  transform: scale(var(--s));
  transform-origin: top left;
  border-radius: var(--r-panel);
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
  color: var(--txt);
  font-family: var(--font-body);
  overflow: hidden;
}

.hero {
  position: relative;
  height: 102px;
  margin: 0 18px;
}

.hero__ring {
  position: absolute;
  top: 19.5px;
  left: 13.5px;
  width: 81.75px;
  height: 81.75px;
}

.hero__value {
  position: absolute;
  top: 31.5px;
  left: 110.25px;
  display: flex;
  align-items: baseline;
  gap: 3px;
  margin: 0;
}

.hero__num {
  color: var(--ink);
  font-family: var(--font-num);
  font-size: 24px;
  font-weight: 700;
  letter-spacing: -0.03em;
  line-height: 24px;
}

.hero__pct {
  font-family: var(--font-num);
  font-size: 11.25px;
  font-weight: 700;
  line-height: 1;
}

.hero__online {
  position: absolute;
  top: 60px;
  left: 110.25px;
  margin: 0;
  color: var(--ink-2);
  font-size: 9.75px;
  font-weight: 400;
  line-height: 9.75px;
  white-space: nowrap;
}

.hero__side {
  position: absolute;
  top: 39px;
  left: 217.5px;
  display: grid;
  grid-template-columns: 5px 58px auto;
  column-gap: 4.5px;
  row-gap: 12px;
  align-items: center;
  margin: 0;
  padding: 0;
  list-style: none;
}

.hero__stat {
  display: contents;
}

.hero__dot {
  display: block;
  width: 3.75px;
  height: 3.75px;
  border-radius: 50%;
}

.hero__label,
.hero__val {
  color: var(--ink-2);
  font-size: 11.25px;
  line-height: 11.25px;
  white-space: nowrap;
}

.hero__label {
  font-weight: 400;
}

.hero__val {
  color: var(--ink);
  font-family: var(--font-num);
  font-size: 11.25px;
  font-weight: 700;
}

.section {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 25.5px;
  padding: 0 18px;
}

.section__title {
  margin: 0;
  color: var(--ink);
  font-size: 11.25px;
  font-weight: 700;
  line-height: 1;
}

.section__add {
  padding: 6px 0;
  border: 0;
  background: transparent;
  color: var(--accent);
  font-family: var(--font-body);
  font-size: 11.25px;
  font-weight: 700;
  line-height: 1;
  cursor: pointer;
}

.rows {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0 18px 6px;
}

.empty {
  display: flex;
  flex-direction: column;
  justify-content: center;
  gap: 7.5px;
  height: 72px;
  padding: 0 13.5px;
  border: 1px solid var(--card-line);
  border-radius: var(--r-tile);
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  text-align: left;
  cursor: pointer;
}

.empty__title {
  font-size: 11.25px;
  font-weight: 700;
  line-height: 1;
}

.empty__hint {
  color: var(--ink-3);
  font-size: 9.75px;
  font-weight: 400;
  line-height: 1;
}

.more {
  display: block;
  width: 100%;
  height: 15px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 9.75px;
  font-weight: 400;
  line-height: 1;
  cursor: pointer;
}

.section__add:focus-visible,
.empty:focus-visible,
.more:focus-visible {
  outline: 1.5px solid var(--accent);
  outline-offset: -1.125px;
}
</style>
