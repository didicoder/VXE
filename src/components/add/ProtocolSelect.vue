<script setup lang="ts">
/**
 * 电量协议选择器：搜索 + 厂家分组多级菜单。
 * - 第一行搜索框，命中 label / 厂家 / 关键词（型号别名，如「示例型号」）
 * - 厂家组默认收起，点组头展开；搜索时自动展开命中组
 * - 「其他」组永远排最后
 * - 面板与电量协议整卡等宽（含右侧试读按钮区）；实色底 + 设置页同款自绘滑块
 */
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from 'vue';
import LineIcon from '../icons/LineIcon.vue';
import MenuToggleIcon from '../icons/MenuToggleIcon.vue';
import type { ProfileChoice } from '../../types/discover';

const {
  modelValue = '',
  options = [],
  emptyLabel = '不指定',
  expandAll = false,
} = defineProps<{
  modelValue?: string;
  options?: ProfileChoice[];
  /** 空 id 固定行的文案（型号选择器用「请选择型号」）。 */
  emptyLabel?: string;
  /** 型号选择器：选项少，打开时直接展开全部厂家。 */
  expandAll?: boolean;
}>();

const emit = defineEmits<{
  'update:modelValue': [id: string];
}>();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const searchInput = ref<HTMLInputElement | null>(null);
const search = ref('');
/** 展开的厂家组（默认全部收起）。 */
const expanded = ref<Set<string>>(new Set());

const groups = computed(() => {
  const map = new Map<string, ProfileChoice[]>();
  for (const choice of options) {
    if (!choice.id) continue; // 「不指定」固定行，不进组
    const vendor = choice.vendor || '其他';
    if (!map.has(vendor)) map.set(vendor, []);
    map.get(vendor)!.push(choice);
  }
  const query = search.value.trim().toLowerCase();
  let list = [...map.entries()].map(([vendor, items]) => ({ vendor, items }));
  if (query) {
    list = list
      .map((group) => ({
        vendor: group.vendor,
        items: group.items.filter((item) =>
          [item.label, group.vendor, item.keywords ?? '']
            .join(' ')
            .toLowerCase()
            .includes(query),
        ),
      }))
      .filter((group) => group.items.length > 0);
  }
  // 「其他」排最后，其余保持协议表顺序。
  return list.sort((a, b) => {
    const rank = (vendor: string): number => (vendor === '其他' ? 1 : 0);
    return rank(a.vendor) - rank(b.vendor);
  });
});

const current = computed(
  () => options.find((choice) => choice.id === modelValue) ?? options[0],
);

function pick(id: string): void {
  emit('update:modelValue', id);
  open.value = false;
}

function isExpanded(vendor: string): boolean {
  return expandAll || search.value.trim().length > 0 || expanded.value.has(vendor);
}

function toggleGroup(vendor: string): void {
  const next = new Set(expanded.value);
  if (next.has(vendor)) next.delete(vendor);
  else next.add(vendor);
  expanded.value = next;
}

function onDocClick(event: MouseEvent): void {
  if (root.value && !root.value.contains(event.target as Node)) open.value = false;
}

// ---------- 滚动：样式走全局 .fade-scroll（滑动时出现，停手隐藏） ----------

watch(open, (next) => {
  if (next) {
    // 打开时展开当前项所在厂家，并滚到该分组，避免只看到分组标题。
    const vendor = current.value?.vendor;
    expanded.value = new Set(vendor ? [vendor] : []);
    void nextTick(() => {
      searchInput.value?.focus();
      if (!vendor) return;
      const group = Array.from(
        root.value?.querySelectorAll<HTMLElement>('.pmenu__group') ?? [],
      ).find((element) => element.textContent?.trim().startsWith(vendor));
      group?.scrollIntoView({ block: 'start' });
    });
  } else {
    search.value = '';
  }
});

onMounted(() => document.addEventListener('click', onDocClick, true));
onBeforeUnmount(() => document.removeEventListener('click', onDocClick, true));
</script>

<template>
  <div ref="root" class="pselect">
    <button
      type="button"
      class="pselect__trigger"
      :aria-expanded="open"
      @click.stop="open = !open"
    >
      <span class="pselect__label">{{ current?.label ?? emptyLabel }}</span>
      <MenuToggleIcon :size="15.3" :active="open" class="pselect__arrow" />
    </button>

    <Transition name="pmenu">
      <div v-if="open" class="pmenu">
        <div class="pmenu__search">
          <LineIcon name="search" :size="11.9" :stroke="1.53" class="pmenu__search-ico" />
          <input
            ref="searchInput"
            v-model="search"
            class="pmenu__search-input"
            type="text"
            placeholder="搜索协议 / 型号 / 厂家…"
            spellcheck="false"
            autocomplete="off"
            @keydown.esc.stop="open = false"
            @click.stop
          >
        </div>

        <div class="pmenu__body fade-scroll" role="listbox">
          <button
            v-if="!search.trim()"
            type="button"
            class="pmenu__item pmenu__item--none"
            :class="{ 'pmenu__item--on': !modelValue }"
            role="option"
            :aria-selected="!modelValue"
            @click.stop="pick('')"
          >
            {{ emptyLabel }}
          </button>

          <template v-for="group in groups" :key="group.vendor">
            <button
              type="button"
              class="pmenu__group"
              @click.stop="toggleGroup(group.vendor)"
            >
              <LineIcon
                name="chevron"
                :size="9.35"
                :stroke="1.7"
                class="pmenu__gchevron"
                :class="{ 'pmenu__gchevron--open': isExpanded(group.vendor) }"
              />
              <span>{{ group.vendor }}</span>
              <span class="pmenu__count">{{ group.items.length }}</span>
            </button>
            <template v-if="isExpanded(group.vendor)">
              <button
                v-for="choice in group.items"
                :key="choice.id"
                type="button"
                class="pmenu__item"
                :class="{ 'pmenu__item--on': choice.id === modelValue }"
                role="option"
                :aria-selected="choice.id === modelValue"
                @click.stop="pick(choice.id)"
              >
                {{ choice.label }}
              </button>
            </template>
          </template>

          <p v-if="groups.length === 0" class="pmenu__empty">没有匹配的协议</p>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.pselect {
  position: static; /* 面板以 .protocol-card 为定位基准 → 与整卡等宽 */
  flex: 1;
  min-width: 0;
}

.pselect__trigger {
  display: flex;
  gap: 6.8px;
  align-items: center;
  width: 100%;
  height: 28.9px;
  padding: 0 10.2px 0 11.9px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-pill);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 11.05px;
  font-weight: 700;
  cursor: pointer;
}

.pselect__label {
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.pselect__arrow {
  margin-left: auto;
  flex: none;
  color: var(--ink-3);
}

/* 实色面板（--menu-solid：与玻璃卡面协调的搭配色）；与电量协议整卡等宽 */
.pmenu {
  position: absolute;
  top: auto;
  right: 0;
  bottom: calc(100% + 5.1px);
  left: 0;
  z-index: 30;
  display: flex;
  flex-direction: column;
  max-height: 219.3px;
  overflow: hidden;
  border: 0;
  border-radius: var(--r-card);
  background: var(--menu-solid, var(--desk-solid, #f6f7f9));
  box-shadow:
    inset 0 0 0 1px var(--card-line),
    0 22px 56px rgb(0 0 0 / 26%);
}

.pmenu__search {
  display: flex;
  flex: none;
  gap: 6.8px;
  align-items: center;
  height: 32.3px;
  padding: 0 10.2px;
  border-bottom: 1px solid var(--card-line);
}

.pmenu__search-ico {
  flex: none;
  color: var(--ink-3);
}

.pmenu__search-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 11.9px;
  outline: none;
}

.pmenu__search-input::placeholder {
  color: var(--ink-3);
}

.pmenu__body {
  flex: 1;
  min-height: 0;
  padding: 5.1px;
  overflow-y: auto;
}

.pmenu__group {
  display: flex;
  gap: 6.8px;
  align-items: center;
  width: 100%;
  height: 28.9px;
  padding: 0 8.5px;
  border: 0;
  border-radius: var(--r-ctrl);
  background: transparent;
  color: var(--ink-3);
  font-family: var(--font-body);
  font-size: 11.475px;
  font-weight: 700;
  letter-spacing: 0.03em;
  cursor: pointer;
}

.pmenu__group:hover {
  background: var(--surface-hover);
}

.pmenu__gchevron {
  flex: none;
  transition: transform var(--speed) var(--ease);
  transform: rotate(-90deg);
}

.pmenu__gchevron--open {
  transform: rotate(0deg);
}

.pmenu__count {
  margin-left: auto;
  font-weight: 400;
  opacity: 0.7;
}

.pmenu__item {
  display: block;
  width: 100%;
  height: 30.6px;
  padding: 0 10.2px 0 25.5px;
  overflow: hidden;
  border: 0;
  border-radius: var(--r-ctrl);
  background: transparent;
  color: var(--ink);
  font-family: var(--font-body);
  font-size: 12.75px;
  font-weight: 600;
  text-align: left;
  white-space: nowrap;
  text-overflow: ellipsis;
  cursor: pointer;
}

.pmenu__item:hover {
  background: var(--surface-hover);
}

.pmenu__item--on {
  background: var(--surface-hover);
  color: var(--accent, var(--ink));
}

.pmenu__item--none {
  padding-left: 10.2px;
  font-weight: 700;
}

.pmenu__empty {
  margin: 11.9px 8.5px;
  color: var(--ink-3);
  font-size: 11.475px;
  text-align: center;
}

.pmenu-enter-active,
.pmenu-leave-active {
  transition:
    opacity var(--speed) var(--ease),
    transform var(--speed) var(--ease);
}

.pmenu-enter-from,
.pmenu-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}
</style>
