<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PhysicalPosition } from '@tauri-apps/api/dpi';
import { getCurrentAlert, saveAlertPosition } from '../lib/ipc';
import { clamp } from '../lib/geometry';
import { useWindowScale } from '../composables/useWindowScale';
import { useConfigStore } from '../stores/config';
import type { AlertPayload } from '../types/alert';

defineProps<{ windowLabel?: string }>();

const config = useConfigStore();
const scale = useWindowScale('alert');
const payload = ref<AlertPayload | null>(null);
const animationKey = ref(0);
const positionMode = ref(false);
const staticPreview = ref(false);
let unlisten: UnlistenFn | null = null;
let unlistenPreview: UnlistenFn | null = null;
let unlistenPosition: UnlistenFn | null = null;
let positionX = 0;
let positionY = 0;

const accentMap: Record<string, string> = {
  green: '#22c55e',
  blue: '#3b82f6',
  red: '#ef4444',
  purple: '#a06bff',
  amber: '#f5920b',
  cyan: '#22c7d6',
};

/** 提醒窗固定设计宽（= 最大胶囊 600 + 左右各 48 留白）。 */
const WINDOW_W = 696;
const BADGE = 64;
const TEXT_GAP = 14;
const RIGHT_PAD = 38;
const MIN_PILL = 250;
const MAX_PILL = 600;

const device = computed(() => payload.value?.device || '设备');
const battery = computed(() => clamp(payload.value?.battery ?? 20, 0, 100));
const accent = computed(() => accentMap[payload.value?.color ?? ''] ?? '#ef4444');

function format(template: string, fallback: string): string {
  return (template || fallback)
    .replace(/\{device\}/g, device.value)
    .replace(/\{battery\}/g, String(battery.value));
}

const title = computed(() => format(config.settings.alert_title, '电量不足'));
const subtitle = computed(() =>
  format(config.settings.alert_subtitle, '{device} 剩余 {battery}%，请及时充电'),
);

// 文字宽度用隐藏量尺实测（V16 用字体度量，字符数估算会让长文案越界/短文案留白）。
const measureTitle = ref<HTMLElement | null>(null);
const measureSub = ref<HTMLElement | null>(null);
const textWidth = ref(0);

function measureText(): void {
  const width = Math.max(
    measureTitle.value?.scrollWidth ?? 0,
    measureSub.value?.scrollWidth ?? 0,
  );
  textWidth.value = width;
}

watch([title, subtitle], () => {
  void nextTick(measureText);
}, { flush: 'post' });

const pillWidth = computed(() => {
  const text = Math.min(textWidth.value + 4, MAX_PILL - BADGE - TEXT_GAP - RIGHT_PAD);
  return clamp(BADGE + TEXT_GAP + text + RIGHT_PAD, MIN_PILL, MAX_PILL);
});

/** 徽标里的电量填充：电池内腔宽 11.2（24 viewBox）。 */
const batteryFill = computed(() => 11.2 * (battery.value / 100));

function show(next: AlertPayload | null | undefined): void {
  if (next) payload.value = next;
  positionMode.value = false;
  staticPreview.value = false;
  animationKey.value += 1;
  void nextTick(measureText);
}

function preview(next: AlertPayload | null | undefined): void {
  if (next) payload.value = next;
  positionMode.value = false;
  staticPreview.value = true;
  animationKey.value += 1;
  void nextTick(measureText);
}

async function enterPositionMode(): Promise<void> {
  positionMode.value = true;
  try {
    const window = getCurrentWindow();
    const position = await window.outerPosition();
    positionX = position.x;
    positionY = position.y;
    await window.setFocus();
  } catch {
    // Browser/dev fallback: keyboard mode still shows the hint.
  }
}

async function onKeydown(event: KeyboardEvent): Promise<void> {
  if (!positionMode.value) return;
  const step = event.shiftKey ? 10 : 1;
  let dx = 0;
  let dy = 0;
  if (event.key === 'ArrowLeft') dx = -step;
  else if (event.key === 'ArrowRight') dx = step;
  else if (event.key === 'ArrowUp') dy = -step;
  else if (event.key === 'ArrowDown') dy = step;
  else if (event.key === 'Enter') {
    event.preventDefault();
    await saveAlertPosition();
    positionMode.value = false;
    await getCurrentWindow().hide();
    return;
  } else if (event.key === 'Escape') {
    event.preventDefault();
    positionMode.value = false;
    await getCurrentWindow().hide();
    return;
  } else {
    return;
  }
  event.preventDefault();
  positionX += dx;
  positionY += dy;
  await getCurrentWindow().setPosition(new PhysicalPosition(positionX, positionY));
}

onMounted(async () => {
  await config.load();
  unlisten = await listen<AlertPayload>('alert-show', (event) => show(event.payload));
  unlistenPreview = await listen<AlertPayload | null>('alert-preview', (event) =>
    preview(event.payload),
  );
  unlistenPosition = await listen('alert-position-start', () => {
    staticPreview.value = false;
    void enterPositionMode();
  });
  window.addEventListener('keydown', onKeydown);
  const current = await getCurrentAlert();
  show(current ?? { device: '设备', battery: 20, color: 'red' });
  await nextTick();
  measureText();
});

onUnmounted(() => {
  unlisten?.();
  unlistenPreview?.();
  unlistenPosition?.();
  window.removeEventListener('keydown', onKeydown);
});
</script>

<template>
  <section
    class="alert-root"
    :class="{ 'position-mode': positionMode, 'static-preview': staticPreview }"
    :key="animationKey"
    :style="{ '--s': scale, '--win-w': `${WINDOW_W}px`, '--pill-width': `${pillWidth}px`, '--accent': accent }"
  >
    <div class="glow" />
    <span class="ring" />
    <div class="pill">
      <div class="shine-clip"><div class="shine" /></div>
      <div class="badge">
        <svg viewBox="0 0 24 24" width="30" height="30" fill="none" stroke="#fff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="2.4" y="6.8" width="14.4" height="10.4" rx="2.9" />
          <rect x="18" y="10" width="2.4" height="4" rx="1.2" fill="#fff" stroke="none" />
          <rect
            v-if="batteryFill > 0.2"
            x="4"
            y="8.4"
            :width="batteryFill"
            height="7.2"
            rx="1.4"
            fill="#fff"
            stroke="none"
          />
        </svg>
      </div>
      <div class="content">
        <div class="title">{{ title }}</div>
        <div class="subtitle">{{ subtitle }}</div>
      </div>
    </div>
    <div class="measure" aria-hidden="true">
      <span ref="measureTitle" class="measure__line measure__title">{{ title }}</span>
      <span ref="measureSub" class="measure__line measure__sub">{{ subtitle }}</span>
    </div>
  </section>
</template>

<style scoped>
.alert-root {
  position: relative;
  width: 696px;
  height: 160px;
  transform: scale(var(--s));
  transform-origin: top left;
  overflow: hidden;
  background: transparent;
}

/* 光晕/扩散环跟着徽标走：先居中，展开期间同步滑到胶囊最终左端。 */
.glow {
  position: absolute;
  left: calc((var(--win-w) - var(--pill-width)) / 2 - 40px);
  top: 8px;
  width: 144px;
  height: 144px;
  border-radius: 50%;
  background: radial-gradient(circle, color-mix(in srgb, var(--accent) 34%, transparent), transparent 68%);
  filter: blur(10px);
  animation:
    glow 700ms ease-out both,
    follow-glow 380ms cubic-bezier(0.33, 1, 0.68, 1) 180ms both;
}

.ring {
  position: absolute;
  z-index: 3;
  top: 48px;
  left: calc((var(--win-w) - var(--pill-width)) / 2);
  width: 64px;
  height: 64px;
  border: 2.5px solid color-mix(in srgb, var(--accent) 62%, transparent);
  border-radius: 50%;
  box-sizing: border-box;
  pointer-events: none;
  animation:
    ring 600ms cubic-bezier(0.2, 0.7, 0.3, 1) both,
    follow-pill 380ms cubic-bezier(0.33, 1, 0.68, 1) 180ms both;
}

.pill {
  position: absolute;
  z-index: 2;
  left: 0;
  right: 0;
  top: 48px;
  width: var(--pill-width);
  height: 64px;
  margin: 0 auto;
  border-radius: 32px;
  animation:
    pill-pop 320ms cubic-bezier(0.34, 1.56, 0.64, 1) both,
    pill-expand 380ms cubic-bezier(0.33, 1, 0.68, 1) 180ms both,
    pill-shrink 330ms cubic-bezier(0.55, 0, 1, 0.45) 5150ms forwards;
}

/* 卡片背景 180 ms 后才出现：起手只有红色圆标，300 ms 后胶囊再撑开。 */
.pill::before {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  background: #1e1e21;
  box-shadow:
    inset 0 0 0 1px #141417,
    0 7px 14px rgb(0 0 0 / 38%),
    0 14px 30px 8px rgb(0 0 0 / 26%);
  opacity: 0;
  animation: card-in 1ms linear 180ms forwards;
  content: "";
}

.badge {
  position: absolute;
  left: 0;
  top: 0;
  width: 64px;
  height: 64px;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 74%, #fff) 0%, var(--accent) 44%, color-mix(in srgb, var(--accent) 70%, #000) 100%);
  animation:
    badge-pop 320ms cubic-bezier(0.34, 1.56, 0.64, 1) both,
    badge-out 300ms cubic-bezier(0.55, 0, 1, 0.45) 5450ms forwards;
}

.content {
  position: absolute;
  left: 78px;
  top: 11px;
  right: 38px;
  overflow: hidden;
  padding: 2px 0;
  margin: -2px 0;
  animation:
    text-in 280ms ease-out 380ms both,
    text-out 180ms ease-in 5000ms forwards;
}

.title {
  overflow: hidden;
  padding: 2px 0;
  margin: -2px 0;
  color: #fff;
  font-size: 19px;
  font-weight: 700;
  line-height: 22px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.subtitle {
  overflow: hidden;
  margin-top: 3px;
  color: #c8c8ce;
  font-size: 14px;
  line-height: 18px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.shine-clip {
  position: absolute;
  inset: 0;
  border-radius: inherit;
  overflow: hidden;
  pointer-events: none;
}

.shine {
  position: absolute;
  top: 0;
  left: -56px;
  width: 56px;
  height: 64px;
  background: linear-gradient(90deg, transparent, rgb(255 255 255 / 16%), transparent);
  transform: skewX(-20deg);
  animation: shine 530ms ease-in-out 620ms both;
}

.measure {
  position: absolute;
  top: 0;
  left: 0;
  visibility: hidden;
  pointer-events: none;
  white-space: nowrap;
}

.measure__line {
  display: inline-block;
  font-family: var(--font-body);
}

.measure__title {
  font-size: 19px;
  font-weight: 700;
}

.measure__sub {
  font-size: 14px;
  font-weight: 400;
}

@keyframes card-in {
  to {
    opacity: 1;
  }
}

@keyframes follow-pill {
  0% {
    left: calc((var(--win-w) - 64px) / 2);
  }
  100% {
    left: calc((var(--win-w) - var(--pill-width)) / 2);
  }
}

@keyframes follow-glow {
  0% {
    left: calc((var(--win-w) - 64px) / 2 - 40px);
  }
  100% {
    left: calc((var(--win-w) - var(--pill-width)) / 2 - 40px);
  }
}

@keyframes glow {
  0% {
    opacity: 0;
    transform: scale(0.6);
  }
  40% {
    opacity: 0.9;
  }
  100% {
    opacity: 0;
    transform: scale(1.5);
  }
}

@keyframes ring {
  0% {
    opacity: 0.6;
    transform: scale(1);
  }
  100% {
    opacity: 0;
    transform: scale(1.94);
  }
}

@keyframes pill-pop {
  0% {
    transform: scale(0.2);
    opacity: 0;
  }
  70% {
    transform: scale(1.04);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}

@keyframes pill-expand {
  0% {
    width: 64px;
  }
  100% {
    width: var(--pill-width);
  }
}

@keyframes pill-shrink {
  0% {
    width: var(--pill-width);
    opacity: 1;
  }
  100% {
    width: 64px;
    opacity: 0.35;
  }
}

@keyframes badge-pop {
  0% {
    transform: scale(0);
    opacity: 0;
  }
  70% {
    transform: scale(1.08);
    opacity: 1;
  }
  100% {
    transform: scale(1);
    opacity: 1;
  }
}

@keyframes badge-out {
  0% {
    transform: scale(1);
    opacity: 1;
  }
  100% {
    transform: scale(0);
    opacity: 0;
  }
}

@keyframes text-in {
  0% {
    opacity: 0;
    transform: translateY(10px);
  }
  100% {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes text-out {
  0% {
    opacity: 1;
  }
  100% {
    opacity: 0;
  }
}

@keyframes shine {
  0% {
    transform: translateX(0) skewX(-20deg);
  }
  100% {
    transform: translateX(calc(var(--pill-width) + 112px)) skewX(-20deg);
  }
}

.position-mode .pill,
.static-preview .pill,
.position-mode .badge,
.static-preview .badge,
.position-mode .content,
.static-preview .content,
.position-mode .glow,
.static-preview .glow,
.position-mode .ring,
.static-preview .ring,
.position-mode .shine,
.static-preview .shine {
  animation: none;
}

.position-mode .pill,
.static-preview .pill {
  width: var(--pill-width);
}

.position-mode .pill {
  outline: 1.5px solid var(--accent);
  outline-offset: 2px;
}

.position-mode .pill::before,
.static-preview .pill::before {
  opacity: 1;
  animation: none;
}

.position-mode .content,
.static-preview .content {
  opacity: 1;
  transform: none;
}

.position-mode .glow,
.static-preview .glow,
.position-mode .ring,
.static-preview .ring,
.position-mode .shine,
.static-preview .shine {
  opacity: 0;
}

</style>
