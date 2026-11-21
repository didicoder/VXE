<script setup lang="ts">
import { colorHex } from '../../lib/colors';
import { COLOR_KEYS, type ColorKey } from '../../types/battery';

const { current } = defineProps<{
  current: ColorKey;
}>();

const emit = defineEmits<{
  select: [color: ColorKey];
}>();

/** 色名只用于无障碍标签（点本身不带文字，V16 `_palette_row` 同款）。 */
const COLOR_NAMES: Record<ColorKey, string> = {
  blue: '蓝色',
  red: '红色',
  green: '绿色',
  purple: '紫色',
  amber: '橙色',
  cyan: '青色',
};
</script>

<template>
  <div class="palette">
    <button
      v-for="key in COLOR_KEYS"
      :key="key"
      type="button"
      class="palette__slot"
      :aria-label="COLOR_NAMES[key]"
      @click.stop="emit('select', key)"
    >
      <span
        class="palette__dot"
        :class="{ 'palette__dot--on': key === current }"
        :style="{ '--dot': colorHex(key) }"
      ></span>
    </button>
  </div>
</template>

<style scoped>
/* 六点一排：槽 32、色点间距 PAL_PITCH=32，右缘与档位块对齐 */
.palette {
  display: flex;
  height: 32px;
}

.palette__slot {
  display: grid;
  place-items: center;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
}

.palette__dot {
  width: 15px;
  height: 15px;
  border-radius: 50%;
  background: var(--dot);
}

.palette__dot--on {
  width: 19px;
  height: 19px;
  outline: 1.8px solid var(--dot);
  outline-offset: 2.8px;
}

.palette__slot:hover .palette__dot:not(.palette__dot--on) {
  outline: 1.4px solid var(--accent);
  outline-offset: 4.5px;
}
</style>
