<script setup lang="ts">
import { computed } from 'vue';
import BatteryRing from '../BatteryRing.vue';
import StateChip from '../StateChip.vue';
import DeviceIcon, { type DeviceIconName } from '../icons/DeviceIcon.vue';
import LineIcon from '../icons/LineIcon.vue';
import ColorPalette from './ColorPalette.vue';
import IconPicker from './IconPicker.vue';
import RenameInput from './RenameInput.vue';
import { deviceIdent } from '../../lib/deviceIdentity';
import { bandColor, bandOf, isCharging } from '../../lib/bands';
import { colorHex } from '../../lib/colors';
import { levelOf, levelText } from '../../lib/state';
import {
  BATTERY_BANDS,
  type BandColors,
  type BatteryBand,
  type ColorKey,
} from '../../types/battery';
import type { DeviceConfig, DeviceIcon as DeviceIconKey, DeviceStatus } from '../../types/device';

/** 配色块里正在改的档位；`accent` = 设备强调色（链接芯片）。 */
type ColorPickKind = BatteryBand | 'accent';

const props = defineProps<{
  device: DeviceConfig;
  status: DeviceStatus | null;
  good: number;
  warn: number;
  bandColors: BandColors;
  /** 链接芯片列宽（跨行统一，状态文案才对得齐） */
  chipWidth: number;
  editing: boolean;
  colorPick: ColorPickKind | null;
  iconPicking: boolean;
  /** 图标选择器贴窗口底部时翻到行上方（V16 `_icon_picker`） */
  pickerUp: boolean;
  pendingDel: boolean;
}>();

const emit = defineEmits<{
  edit: [];
  rename: [text: string];
  cancelRename: [];
  openColor: [kind: ColorPickKind];
  closeColor: [];
  pickColor: [color: ColorKey];
  openIcon: [];
  pickIcon: [icon: DeviceIconKey];
  remove: [];
}>();

const BAND_LABEL: Record<BatteryBand, string> = {
  good: '电量充足',
  mid: '正常使用',
  low: '电量不足',
};

const NAME_SIZE = 19;
const NAME_MIN = 11;
const NAME_MAXW = 166;

/** 名称列宽固定：放不下先缩字号，缩到下限再交给 CSS 截断（V16 `_fit`）。 */
let nameCtx: CanvasRenderingContext2D | null | undefined;

function nameWidth(text: string, size: number): number {
  if (nameCtx === undefined) {
    nameCtx =
      typeof document === 'undefined' ? null : document.createElement('canvas').getContext('2d');
  }
  if (!nameCtx) return text.length * size;
  nameCtx.font = `700 ${size}px "Microsoft YaHei", "Segoe UI", sans-serif`;
  return nameCtx.measureText(text).width;
}

/** 档位色 → token：三档走 `--good/--mid/--low`，其余走六色板。 */
function bandToken(key: ColorKey): string {
  if (key === 'green') return 'var(--good)';
  if (key === 'blue') return 'var(--mid)';
  if (key === 'red') return 'var(--low)';
  return colorHex(key);
}

const display = computed(() =>
  props.status ? levelOf(props.status) : { level: null, live: false },
);
const band = computed(() => bandOf(display.value.level, props.good, props.warn));
const ringColor = computed(() =>
  display.value.level == null
    ? 'var(--faint)'
    : bandToken(bandColor(band.value, props.bandColors, props.device.colors)),
);
const pctColor = computed(() => (display.value.live ? 'var(--txt)' : 'var(--muted)'));

const iconName = computed<DeviceIconName>(() => {
  if (props.device.icon) return props.device.icon;
  const profile = props.device.profile.toLowerCase();
  if (
    profile.startsWith('bt-audio') ||
    props.device.link.includes('音频') ||
    props.device.link.includes('耳机')
  ) {
    return 'headset';
  }
  return 'mouse';
});

const label = computed(() => props.device.label);
const labelSize = computed(() => {
  let size = NAME_SIZE;
  while (size > NAME_MIN && nameWidth(label.value, size) > NAME_MAXW) size -= 0.5;
  return size;
});
const ident = computed(() => deviceIdent(props.device));
const link = computed(() => props.device.link || '2.4G');
const chipColor = computed(() => colorHex(props.device.color));
const charging = computed(() => (props.status ? isCharging(props.status) : false));
const chipState = computed(() => props.status?.state ?? 'error');
const chipText = computed(() => (props.status ? undefined : '--'));
const stateLeft = computed(() => `${362 + props.chipWidth}px`);

const bandSlots = computed(() =>
  BATTERY_BANDS.map((key) => ({
    key,
    label: BAND_LABEL[key],
    current: key === band.value,
    color: bandToken(bandColor(key, props.bandColors, props.device.colors)),
  })),
);

const paletteCurrent = computed<ColorKey>(() => {
  if (props.colorPick === 'accent') return props.device.color;
  if (props.colorPick) return bandColor(props.colorPick, props.bandColors, props.device.colors);
  return props.device.color;
});

const pickerStyle = computed(() => ({ top: props.pickerUp ? '-140px' : '108px' }));
</script>

<template>
  <article
    class="row glass-card"
    :class="{ 'row--raised': iconPicking }"
    data-drag-item
  >
    <button
      type="button"
      class="row__grip"
      aria-label="拖动排序"
      title="按住拖动排序"
      @click.stop
    >
      <LineIcon name="move" :size="14" :stroke="1.5" />
    </button>
    <button type="button" class="row__icon" aria-label="选择设备图标" @click.stop="emit('openIcon')">
      <BatteryRing
        class="row__ring"
        :level="display.level"
        :size="72"
        :thickness="8"
        :color="ringColor"
      />
      <DeviceIcon class="row__glyph" :name="iconName" :size="32" />
      <span v-if="iconPicking" class="row__halo"></span>
    </button>

    <button v-if="!editing" type="button" class="row__name" @click.stop="emit('edit')">
      <span class="row__name-text" :style="{ fontSize: `${labelSize}px` }">{{ label }}</span>
    </button>
    <RenameInput
      v-else
      class="row__rename"
      :value="device.label"
      @commit="(text) => emit('rename', text)"
      @cancel="emit('cancelRename')"
    />

    <span class="row__ident">{{ ident }}</span>

    <span class="row__pct" :style="{ color: pctColor }">{{ levelText(display.level) }}</span>
    <span
      v-if="display.level != null"
      class="row__sign"
      :style="{ color: display.live ? ringColor : 'var(--muted)' }"
    >%</span>

    <button
      type="button"
      class="row__chip"
      :style="{ '--chip': chipColor }"
      aria-label="选择强调色"
      @click.stop="emit('openColor', 'accent')"
    >{{ link }}</button>

    <div class="row__state" :style="{ left: stateLeft }">
      <StateChip :state="chipState" :charging="charging" :text="chipText" />
    </div>

    <div v-if="!pendingDel && !colorPick" class="row__bands">
      <button
        v-for="(slot, index) in bandSlots"
        :key="slot.key"
        type="button"
        class="row__band"
        :style="{ left: `${index * 70}px` }"
        :aria-label="slot.label"
        @click.stop="emit('openColor', slot.key)"
      >
        <span class="row__band-label" :class="{ 'row__band-label--on': slot.current }">
          {{ slot.label }}
        </span>
        <span
          class="row__band-dot"
          :class="{ 'row__band-dot--on': slot.current }"
          :style="{ '--dot': slot.color }"
        ></span>
      </button>
    </div>

    <ColorPalette
      v-else-if="colorPick"
      class="row__palette"
      :current="paletteCurrent"
      @select="(color) => emit('pickColor', color)"
    />

    <button
      v-if="colorPick"
      type="button"
      class="row__palette-close"
      @click.stop="emit('closeColor')"
    >选颜色（点此收起）</button>

    <button
      v-if="!pendingDel"
      type="button"
      class="row__del"
      aria-label="删除设备"
      @click.stop="emit('remove')"
    >
      <LineIcon name="trash" :size="27" :stroke="2" />
    </button>
    <button v-else type="button" class="row__confirm" @click.stop="emit('remove')">确认删除</button>

    <IconPicker
      v-if="iconPicking"
      class="row__picker"
      :style="pickerStyle"
      :current="iconName"
      @select="(icon) => emit('pickIcon', icon)"
    />
  </article>
</template>

<style scoped>
/* 行卡 816×100，五元素按卡内固定坐标落位（V16 `_device_row`）。表面走 .glass-card */
.row {
  position: relative;
  width: 816px;
  height: 100px;
  flex: none;
}

/* 图标/配色选择器伸出卡片外，豁免 .glass-card 的圆角 mask，否则选择器被裁掉 */
.row.glass-card {
  -webkit-mask-image: none;
  mask-image: none;
}

/* 右上角拖拽把手 */
.row__grip {
  position: absolute;
  top: 5px;
  right: 5px;
  z-index: 6;
  display: grid;
  place-items: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: rgb(127 127 127 / 16%);
  color: var(--ink-3);
  cursor: grab;
  touch-action: none;
}

.row__grip:hover {
  background: rgb(127 127 127 / 28%);
  color: var(--ink);
}

.row__grip:active {
  cursor: grabbing;
}

.row--raised {
  z-index: 5;
}

/* 图标位：圆心 (50,cy+8)，环 r32/w8，剪影 32 */
.row__icon {
  position: absolute;
  top: 16px;
  left: 10px;
  width: 76px;
  height: 80px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  cursor: pointer;
}

.row__ring {
  position: absolute;
  top: 6px;
  left: 4px;
}

.row__glyph {
  position: absolute;
  top: 26px;
  left: 24px;
  display: block;
  color: var(--ink);
  opacity: 0.9;
}

.row__halo {
  position: absolute;
  top: 4px;
  left: 2px;
  width: 76px;
  height: 76px;
  border: 1.5px solid var(--accent);
  border-radius: 50%;
}

/* 名称：18 bold，中心 y=37；悬停变强调色 + 下划线 */
.row__name {
  position: absolute;
  top: 20px;
  left: 84px;
  z-index: 2;
  width: 232px;
  height: 38px;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.row__name-text {
  position: absolute;
  top: 19px;
  left: 8px;
  max-width: 216px;
  overflow: hidden;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  letter-spacing: -0.01em;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  transform: translateY(-50%);
  transition: color 0.3s ease;
}

.row__name:hover .row__name-text {
  color: var(--accent);
  text-decoration: underline;
  text-decoration-thickness: 1.4px;
  text-underline-offset: 4px;
}

.row__rename {
  position: absolute;
  top: 23px;
  left: 88px;
  z-index: 2;
}

/* 纯展示文本不拦截鼠标，避免压住名字/按钮的命中区 */
.row__ident {
  position: absolute;
  top: 66px;
  left: 92px;
  color: var(--ink-3);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  pointer-events: none;
  transform: translateY(-50%);
}

/* 电量：数字右对齐 x=316、24 numbold；% 在 +4、中心 y=57 */
.row__pct {
  position: absolute;
  top: 50px;
  left: 0;
  width: 316px;
  color: var(--ink);
  font-family: var(--font-num);
  font-size: 24px;
  font-weight: 700;
  line-height: 1;
  pointer-events: none;
  text-align: right;
  transform: translateY(-50%);
}

.row__sign {
  position: absolute;
  top: 57px;
  left: 320px;
  font-family: var(--font-num);
  font-size: 13px;
  font-weight: 600;
  line-height: 1;
  pointer-events: none;
  transform: translateY(-50%);
}

/* 链接芯片：左缘 x=352、高 36；胶囊 + 强调色预混 */
.row__chip {
  position: absolute;
  top: 32px;
  left: 352px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 36px;
  padding: 0 10px;
  border: 1px solid color-mix(in srgb, var(--chip) 26%, transparent);
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--chip) 15%, var(--chip-bg-mix));
  color: color-mix(in srgb, var(--chip) var(--chip-text-weight), var(--chip-text-mix));
  font-family: var(--font-body);
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition:
    background-color var(--speed) var(--ease),
    border-color var(--speed) var(--ease);
}

.row__chip:hover {
  border-color: color-mix(in srgb, var(--chip) 42%, transparent);
  background: color-mix(in srgb, var(--chip) 22%, var(--chip-bg-mix));
}

/* 状态：芯片右 +10，20 bold */
.row__state {
  position: absolute;
  top: 0;
  display: flex;
  align-items: center;
  height: 100px;
  font-size: 18px;
}

/* 三档配色：块右缘 x1−72，档距 70、热区 70 */
.row__bands {
  position: absolute;
  top: 0;
  right: 72px;
  width: 210px;
  height: 100px;
}

.row__band {
  position: absolute;
  top: 15px;
  width: 70px;
  height: 70px;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.row__band-label {
  position: absolute;
  top: 18px;
  left: 0;
  width: 70px;
  color: var(--muted);
  font-size: 13px;
  font-weight: 400;
  line-height: 1;
  text-align: center;
  white-space: nowrap;
  transform: translateY(-50%);
}

.row__band-label--on {
  color: var(--txt);
  font-weight: 700;
}

.row__band-dot {
  position: absolute;
  top: 46px;
  left: 50%;
  width: 15px;
  height: 15px;
  margin: -7.5px 0 0 -7.5px;
  border-radius: 50%;
  background: var(--dot);
}

.row__band-dot--on {
  width: 19px;
  height: 19px;
  margin: -9.5px 0 0 -9.5px;
  outline: 1.7px solid var(--dot);
  outline-offset: 4.2px;
}

.row__band:hover .row__band-dot:not(.row__band-dot--on) {
  outline: 1.4px solid var(--line);
  outline-offset: 8.5px;
}

/* 就地选色：六点一排落在档位块的位置上 */
.row__palette {
  position: absolute;
  top: 45px;
  right: 72px;
}

.row__palette-close {
  position: absolute;
  top: 33px;
  left: 639px;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--muted);
  font-family: var(--font-body);
  font-size: 12.5px;
  font-weight: 400;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transform: translate(-50%, -50%);
}

/* 删除：圆心 (x1−44, cy)，图标 27 */
.row__del {
  position: absolute;
  top: 31px;
  left: 752px;
  display: grid;
  place-items: center;
  width: 40px;
  height: 38px;
  padding: 0;
  border: 0;
  border-radius: 50%;
  background: transparent;
  color: var(--muted);
  cursor: pointer;
  transition:
    background-color var(--dur-fast) var(--ease-out),
    color var(--dur-fast) var(--ease-out);
}

.row__del:hover {
  background: color-mix(in srgb, var(--low) 14%, var(--card));
  color: var(--ink-low);
}

.row__confirm {
  position: absolute;
  top: 33px;
  left: 688px;
  width: 116px;
  height: 34px;
  padding: 0;
  border: 0;
  border-radius: var(--r-pill);
  background: color-mix(in srgb, var(--low) 14%, var(--card));
  color: var(--ink-low);
  font-family: var(--font-body);
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
  cursor: pointer;
  transition: background-color var(--dur-fast) var(--ease-out);
}

.row__confirm:hover {
  background: color-mix(in srgb, var(--low) 30%, var(--card));
}

/* 图标选择器：锚定行下方 8 px，放不下翻到上方 */
.row__picker {
  position: absolute;
  left: 6px;
  z-index: 6;
}
</style>
