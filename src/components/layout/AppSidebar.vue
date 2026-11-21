<script setup lang="ts">
import LineIcon, { type LineIconName } from '../icons/LineIcon.vue';
import appIconDark from '../../assets/app-icon-dark.png';
import appIconLight from '../../assets/app-icon-light.png';
import appIconTray from '../../assets/card/app-icon-tray.png';
import type { UiTheme } from '../../types/config';
import type { UiPage } from '../../stores/ui';

const { page, theme, variant = 'classic', collapsed = false } = defineProps<{
  page: UiPage;
  theme: UiTheme;
  /** `photo` = 实物卡片 UI（品牌图标兼作侧栏开关、VXE 字标、径向光晕）。 */
  variant?: 'photo' | 'classic';
  collapsed?: boolean;
}>();

const emit = defineEmits<{
  nav: [page: UiPage];
  quit: [];
  'toggle-collapse': [];
}>();

/** 侧栏导航（V16 `mainwin.NAV`：总览 / 设备 / 设置）。 */
const NAV: ReadonlyArray<{ key: UiPage; label: string; icon: LineIconName }> = [
  { key: 'summary', label: '总览', icon: 'gauge' },
  { key: 'devices', label: '设备', icon: 'mouse' },
  { key: 'settings', label: '设置', icon: 'sliders' },
];

const toggleTitle = () => (collapsed ? '展开侧栏' : '收起侧栏');
</script>

<template>
  <aside
    class="sidebar"
    :class="[`sidebar--${variant}`, { 'sidebar--min': collapsed }]"
  >
    <!-- 侧栏顶部 0–70 是拖动区（V16 `title` 热区第二块），品牌块不接收点击 -->
    <div class="sidebar__drag" data-tauri-drag-region aria-hidden="true"></div>

    <div class="brand">
      <button
        type="button"
        class="brand__btn"
        :title="toggleTitle()"
        :aria-label="toggleTitle()"
        :aria-expanded="!collapsed"
        @click="emit('toggle-collapse')"
      >
        <img
          class="brand__icon"
          :src="variant === 'photo' ? appIconTray : theme === 'light' ? appIconLight : appIconDark"
          alt=""
          aria-hidden="true"
          draggable="false"
        >
      </button>
      <div class="brand__lines">
        <span v-if="variant === 'photo'" class="brand__mark">VXE</span>
        <template v-else>
          <span class="brand__name">VXE</span>
          <span class="brand__sub">电量监控</span>
        </template>
      </div>
    </div>

    <!-- 光轨贴边导航：轨在侧栏最左缘，发光段随选中项滑动 -->
    <nav class="nav" role="radiogroup" aria-label="页面导航">
      <div class="nav__rail" aria-hidden="true"></div>
      <div
        class="nav__glider"
        :style="{ transform: `translateY(${NAV.findIndex((item) => item.key === page) * 100}%)` }"
        aria-hidden="true"
      ></div>
      <button
        v-for="item in NAV"
        :key="item.key"
        type="button"
        class="nav__item"
        :class="{ 'nav__item--active': item.key === page }"
        :aria-current="item.key === page ? 'page' : undefined"
        :aria-label="item.label"
        role="radio"
        :aria-checked="item.key === page"
        @click="emit('nav', item.key)"
      >
        <LineIcon :name="item.icon" :size="30" :stroke="2.08" />
        <span class="nav__label">{{ item.label }}</span>
      </button>
    </nav>

    <div class="info">
      <span class="info__line info__line--title">本地 HID 读取</span>
      <span class="info__line info__line--sub">不联网 · 不上传 · 无遥测</span>
    </div>

    <button type="button" class="quit" @click="emit('quit')">
      <LineIcon name="power" :size="20" :stroke="1.9" />
      <span class="quit__label">退出应用</span>
    </button>
  </aside>
</template>

<style scoped>
.sidebar {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 2;
  width: 180px;
  height: 720px;
  background: var(--sidebar-bg);
  box-shadow: var(--sidebar-line);
  transition: width 0.26s var(--ease);
}

.sidebar__drag {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 0;
  width: 180px;
  height: 70px;
}

/* 品牌块：经典 UI 居中；实物卡片 UI 左对齐到导航图标列 */
.brand {
  position: absolute;
  top: 0;
  left: 0;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10.8px;
  width: 180px;
  height: 120px;
  padding: 0 10px;
  pointer-events: none;
}

.brand__icon {
  display: block;
  flex: none;
  width: 54px;
  height: 54px;
  filter: drop-shadow(var(--brand-shadow));
  transform-origin: center;
  transition: transform 0.26s var(--ease);
}

.brand__lines {
  display: flex;
  flex-direction: column;
  flex: none;
}

.brand__name,
.brand__sub {
  display: flex;
  align-items: center;
  height: 28px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

.brand__name {
  font-size: 15px;
}

.brand__sub {
  color: var(--ink-3);
  font-size: 13px;
  font-weight: 400;
}

/* ---------- 实物卡片 UI 的品牌块 ---------- */
.sidebar--photo .brand {
  top: 10px;
  justify-content: flex-start;
  width: 180px;
  padding: 0 10px 8px 30px;
}

.brand__btn {
  display: grid;
  place-items: center;
  flex: none;
  width: 54px;
  height: 54px;
  padding: 0;
  border: 0;
  border-radius: 15px;
  background: none;
  cursor: pointer;
  pointer-events: auto;
  transition: transform 0.22s var(--ease);
}

.brand__btn:hover {
  transform: scale(1.05);
}

.brand__btn:active {
  transform: scale(0.98);
}

.brand__btn:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
}

.brand__mark {
  font-size: 18px;
  font-weight: 800;
  line-height: 1;
  letter-spacing: 0.02em;
  transform: translateY(-3px);
}

/* 光轨贴边导航（Uiverse "quick-panther-98" 适配）：轨 1px 贴窗口左缘。
 * 两种界面风格统一：列表位置（图标/文字列）用实物卡片版（left:24），
 * 蓝色渐变用经典卡片版（线性 --nav-glow，不用径向晕开）。 */
.nav {
  --total-radio: 3;
  position: absolute;
  top: 126px;
  left: 24px;
  display: flex;
  flex-direction: column;
  width: 152px;
}

.nav__rail {
  position: absolute;
  top: 0;
  bottom: 0;
  left: -24px;
  width: 1px;
  background: linear-gradient(0deg, rgba(0, 0, 0, 0) 0%, var(--nav-line) 50%, rgba(0, 0, 0, 0) 100%);
  pointer-events: none;
}

.nav__glider {
  position: absolute;
  top: 0;
  left: -24px;
  width: 1px;
  height: calc(100% / var(--total-radio));
  background: linear-gradient(0deg, rgba(0, 0, 0, 0) 0%, var(--rail-core) 50%, rgba(0, 0, 0, 0) 100%);
  box-shadow: 0 0 10px 1px color-mix(in srgb, var(--accent) 55%, transparent);
  transition: transform 0.5s cubic-bezier(0.37, 1.95, 0.66, 0.56);
  /* 光轨 + 光晕是纯装饰：必须穿透指针，否则会挡住导航按钮的点击 */
  pointer-events: none;
}

/* 竖向光柱：9px 宽 + 12px 模糊，压在选中行上 */
.nav__glider::before {
  position: absolute;
  top: 50%;
  left: 0;
  width: 900%;
  height: 150%;
  background: linear-gradient(
    180deg,
    rgba(0, 0, 0, 0) 0%,
    var(--accent) 28%,
    var(--accent) 72%,
    rgba(0, 0, 0, 0) 100%
  );
  filter: blur(12px);
  opacity: 0.55;
  content: "";
  transform: translateY(-50%);
  pointer-events: none;
}

/* 横向晕开：必须给 top:50% + translateY(-50%)，否则会跑到上一行去 */
.nav__glider::after {
  position: absolute;
  top: 50%;
  left: 0;
  width: 210px;
  height: 100%;
  background: linear-gradient(90deg, var(--nav-glow) 0%, var(--nav-glow) 0%, rgba(0, 0, 0, 0) 100%);
  content: "";
  transform: translateY(-50%);
  pointer-events: none;
}

/* 实物卡片 UI：导航列右移 10px（两种风格共用，见上）。
 * 蓝色渐变两版统一走经典卡片的线性 --nav-glow，不再用径向晕开。 */

.nav__item {
  display: flex;
  align-items: center;
  gap: 13px;
  width: 152px;
  height: 54px;
  padding: 0 0 0 16px;
  border: 0;
  border-radius: 0;
  background: transparent;
  color: var(--ink-2);
  cursor: pointer;
  transition: color 0.3s ease-in-out;
}

.nav__item .line-icon {
  display: flex;
  flex: none;
  color: var(--icon-muted);
  transition: color 0.3s ease-in-out;
}

.nav__item:hover {
  color: var(--ink);
}

.nav__item:hover .line-icon {
  color: var(--ink);
}

/* 选中态必须压过 :hover（`.nav__item:hover` 是 2 个选择器，单类的 --active 会输），
 * 否则鼠标一进到当前项，蓝色高亮就变回墨色。 */
.nav__item--active,
.nav__item--active:hover {
  color: var(--accent);
}

.nav__item--active .line-icon,
.nav__item--active:hover .line-icon {
  color: var(--accent);
}

.nav__item:focus-visible {
  outline: 2px solid var(--focus);
  outline-offset: 2px;
  border-radius: var(--r-ctrl);
}

.nav__label {
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

/* 信息卡 (14,584)-(166,650)，两行中心 604 / 626 */
.info {
  position: absolute;
  top: 584px;
  left: 14px;
  width: 152px;
  height: 66px;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--veil-bg);
  box-shadow: var(--veil-shadow);
  transition: opacity 0.18s ease;
}

.info__line {
  position: absolute;
  left: 0;
  width: 152px;
  font-weight: 700;
  line-height: 1;
  text-align: center;
  transform: translateY(-50%);
  white-space: nowrap;
}

.info__line--title {
  top: 20px;
  font-size: 13px;
  color: var(--ink);
}

.info__line--sub {
  top: 42px;
  font-size: 13px;
  font-weight: 400;
  color: var(--ink-3);
}

/* 退出 (14,662)-(166,704)，圆角 12 */
.quit {
  position: absolute;
  top: 662px;
  left: 14px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 10px;
  width: 152px;
  height: 42px;
  padding: 0;
  border: 1px solid var(--ctrl-line);
  border-radius: var(--r-ctrl);
  background: var(--ctrl-bg);
  box-shadow: var(--ctrl-shadow);
  color: var(--ink-2);
  cursor: pointer;
  transition:
    background var(--speed) var(--ease),
    color var(--speed) var(--ease),
    border-color var(--speed) var(--ease),
    width 0.26s var(--ease),
    left 0.26s var(--ease);
}

.quit:hover {
  background: var(--ctrl-bg-hover);
  border-color: var(--ctrl-line-hover);
  color: var(--ink);
}

.quit__label {
  font-size: 18px;
  font-weight: 700;
  line-height: 1;
  white-space: nowrap;
}

/* ---------- 收起态：只留 60px 图标轨道（vxe / 总览 / 设备 / 设置 / 退出） ---------- */
.sidebar--min {
  width: 60px;
}

.sidebar--min .sidebar__drag {
  width: 60px;
}

.sidebar--min .brand {
  justify-content: center;
  width: 60px;
  padding: 0 0 8px;
}

.sidebar--min .brand__icon {
  transform: scale(0.6);
}

.sidebar--min .brand__lines {
  display: none;
}

.sidebar--min .nav {
  left: 0;
  align-items: center;
  width: 60px;
}

.sidebar--min .nav__item {
  justify-content: center;
  width: 60px;
  padding: 0;
  gap: 0;
}

.sidebar--min .nav__label {
  display: none;
}

.sidebar--min .nav__rail,
.sidebar--min .nav__glider {
  left: 0;
}

/* 收起态：横向晕开跟着缩短到轨道内，不能凸出 60px 侧栏 */
.sidebar--min .nav__glider::after {
  width: 56px;
}

.sidebar--min .info {
  opacity: 0;
  pointer-events: none;
}

.sidebar--min .quit {
  left: 8px;
  width: 44px;
  padding: 0;
  gap: 0;
}

.sidebar--min .quit__label {
  display: none;
}
</style>