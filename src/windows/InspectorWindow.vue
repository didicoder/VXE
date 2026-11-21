<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import CardInspector from '../components/devices/CardInspector.vue';
import { useThemeClass } from '../composables/useThemeClass';
import { useWallpaper } from '../composables/useWallpaper';
import { useWindowMotion } from '../composables/useWindowMotion';
import { useWindowScale } from '../composables/useWindowScale';
import * as ipc from '../lib/ipc';
import { useCardImagesStore } from '../stores/cardImages';
import { useConfigStore } from '../stores/config';
import { useDevicesStore } from '../stores/devices';
import type { DeviceConfig } from '../types/device';

defineProps<{ windowLabel?: string }>();

const config = useConfigStore();
const deviceStore = useDevicesStore();
const cardImages = useCardImagesStore();
const themeClass = useThemeClass();
useWallpaper();
const { entering, leaving, enter, leave } = useWindowMotion();
const inspectorScale = useWindowScale('inspector');

const targetId = ref<string | null>(null);
const imageError = ref('');
const imageBusy = ref(false);
let pending: Partial<DeviceConfig> = {};
let closing = false;

const device = computed(() =>
  targetId.value ? (config.devices.find((item) => item.id === targetId.value) ?? null) : null,
);
const status = computed(() =>
  targetId.value ? (deviceStore.statuses.find((item) => item.id === targetId.value) ?? null) : null,
);
const image = computed(() => (targetId.value ? cardImages.imageFor(targetId.value) : null));
const custom = computed(() => image.value !== null);

function setTarget(id: string | null): void {
  targetId.value = id;
  pending = {};
  imageError.value = '';
  if (id) {
    void cardImages.loadOne(id);
    void enter();
  }
}

async function finalizePending(): Promise<void> {
  const id = targetId.value;
  const patch = pending;
  pending = {};
  if (!id) return;
  if (Object.keys(patch).length > 0) {
    const ok = await config.updateDevice(id, patch);
    if (!ok) imageError.value = config.error ?? '卡片设置保存失败';
  }
  await ipc.previewCard(id, {});
}

function onPreview(patch: Partial<DeviceConfig>): void {
  const id = targetId.value;
  if (!id) return;
  pending = { ...pending, ...patch };
  void ipc.previewCard(id, patch);
}

async function onCommit(patch: Partial<DeviceConfig>): Promise<void> {
  const id = targetId.value;
  if (!id) return;
  const full = { ...pending, ...patch };
  pending = {};
  const ok = await config.updateDevice(id, full);
  if (!ok) imageError.value = config.error ?? '卡片设置保存失败';
  await ipc.previewCard(id, {});
}

async function saveImage(id: string, dataUrl: string): Promise<void> {
  imageError.value = '';
  imageBusy.value = true;
  try {
    await cardImages.save(id, dataUrl);
    const ok = await config.updateDevice(id, { card_img: 'custom' });
    if (!ok) imageError.value = `图片已替换，但写入设备配置失败：${config.error ?? '未知错误'}`;
  } catch (cause) {
    imageError.value = `图片保存失败：${ipc.errorMessage(cause)}`;
  } finally {
    imageBusy.value = false;
  }
}

async function onSetImage(dataUrl: string): Promise<void> {
  if (targetId.value) await saveImage(targetId.value, dataUrl);
}

async function onSetImagePath(path: string): Promise<void> {
  const id = targetId.value;
  if (!id) return;
  imageError.value = '';
  imageBusy.value = true;
  try {
    await cardImages.saveFromPath(id, path);
    const ok = await config.updateDevice(id, { card_img: 'custom' });
    if (!ok) imageError.value = `图片已替换，但写入设备配置失败：${config.error ?? '未知错误'}`;
  } catch (cause) {
    imageError.value = `图片载入失败：${ipc.errorMessage(cause)}`;
  } finally {
    imageBusy.value = false;
  }
}

async function onClearImage(): Promise<void> {
  const id = targetId.value;
  if (!id) return;
  imageError.value = '';
  try {
    await cardImages.clear(id);
    await config.updateDevice(id, { card_img: '' });
  } catch (cause) {
    imageError.value = `清除图片失败：${ipc.errorMessage(cause)}`;
  }
}

async function onReset(): Promise<void> {
  const id = targetId.value;
  if (!id) return;
  pending = {};
  imageError.value = '';
  await ipc.previewCard(id, {});
  try {
    await cardImages.clear(id);
    await config.updateDevice(id, {
      card_brand: '',
      card_name: '',
      card_wm: '',
      card_img: '',
      card_ratio: 80,
      card_dx: 0,
      card_dy: 0,
      card_rot: 0,
      card_shadow: true,
      card_glow: true,
      card_fields: { link: true, ident: true, power: true },
    });
  } catch (cause) {
    imageError.value = `恢复默认失败：${ipc.errorMessage(cause)}`;
  }
}

async function closeWindow(): Promise<void> {
  if (closing) return;
  closing = true;
  await finalizePending();
  await leave(async () => {
    try {
      await ipc.hideCardInspector();
    } catch {
      /* 非 Tauri 环境忽略 */
    }
  });
  targetId.value = null;
  closing = false;
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== 'Escape') return;
  event.preventDefault();
  event.stopPropagation();
  void closeWindow();
}

let unlisten: UnlistenFn[] = [];

onMounted(async () => {
  window.addEventListener('keydown', onKeydown, true);
  try {
    unlisten.push(
      await listen<string>('inspector-open', (event) => {
        setTarget(event.payload);
      }),
    );
    unlisten.push(
      await listen('inspector-close', () => {
        void finalizePending();
        targetId.value = null;
      }),
    );
    unlisten.push(
      await listen<string>(ipc.CARD_IMAGE_CHANGED_EVENT, (event) => {
        if (event.payload === targetId.value) void cardImages.loadOne(event.payload);
      }),
    );
  } catch {
    /* 浏览器预览/单测没有事件通道，忽略 */
  }
  await Promise.all([config.load(), deviceStore.loadCached()]);
  void cardImages.loadAll(config.devices);
  try {
    const target = await ipc.getCardInspectorTarget();
    if (target) setTarget(target);
  } catch {
    /* 非 Tauri 环境忽略 */
  }
});

onUnmounted(() => {
  window.removeEventListener('keydown', onKeydown, true);
  for (const off of unlisten) off();
  unlisten = [];
});
</script>

<template>
  <section
    class="inspector-window glass-window"
    :class="[themeClass, { 'window-enter': entering, 'window-leave': leaving }]"
    :style="{ '--s': inspectorScale }"
    data-window="inspector"
  >
    <CardInspector
      v-if="device"
      :device="device"
      :status="status"
      :image="image"
      :custom="custom"
      :error="imageError"
      :busy="imageBusy"
      @preview="onPreview"
      @commit="onCommit"
      @set-image="onSetImage"
      @set-image-from-path="onSetImagePath"
      @clear-image="onClearImage"
      @reset="onReset"
      @close="closeWindow"
    />
    <div v-else class="inspector-empty">
      {{ targetId ? '设备不存在或已删除' : '请选择一张卡片进行设置' }}
    </div>
  </section>
</template>

<style scoped>
.inspector-window {
  position: absolute;
  top: 0;
  left: 0;
  width: 400px;
  height: 630px;
  overflow: hidden;
  border-radius: 18px;
  box-shadow: inset 0 0 0 1px var(--card-line), var(--card-shadow);
  color: var(--txt);
  transform: scale(var(--s));
  transform-origin: top left;
}

.inspector-empty {
  display: grid;
  place-items: center;
  width: 100%;
  height: 100%;
  color: var(--ink-3);
  font-size: 14px;
}
</style>