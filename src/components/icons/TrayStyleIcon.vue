<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(
  defineProps<{
    kind: 'bolt' | 'ring' | 'battery'
    level?: number
    size?: number
  }>(),
  { level: 60, size: 24 },
)

const lv = computed(() => Math.min(100, Math.max(0, props.level)))
const clipId = `tray-bolt-${Math.random().toString(36).slice(2, 9)}`
const boltClipTop = computed(() => 57.333 - (50.666 * lv.value) / 100)
const boltFill = computed(() => lv.value > 0)
const ringDash = computed(() => ((2 * Math.PI * 24 * lv.value) / 100).toFixed(2))
const ringFill = computed(() => lv.value > 0)
const batteryFillWidth = computed(() => (38 * Math.max(0.06, lv.value / 100)).toFixed(2))
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 64 64"
    fill="none"
    class="tray-style-icon"
    aria-hidden="true"
  >
    <template v-if="kind === 'bolt'">
      <defs>
        <clipPath :id="clipId">
          <rect x="0" :y="boltClipTop" width="64" height="64" />
        </clipPath>
      </defs>
      <path
        v-if="boltFill"
        d="M36 6.667 L16 36 H29.333 L26.667 57.333 L48 28 H33.333 Z"
        fill="currentColor"
        :clip-path="`url(#${clipId})`"
      />
      <path
        d="M36 6.667 L16 36 H29.333 L26.667 57.333 L48 28 H33.333 Z"
        stroke="currentColor"
        stroke-width="4"
        stroke-linejoin="round"
        opacity="0.63"
      />
    </template>

    <template v-else-if="kind === 'ring'">
      <circle
        cx="32"
        cy="32"
        r="24"
        stroke="currentColor"
        stroke-width="8"
        opacity="0.32"
      />
      <circle
        v-if="ringFill"
        cx="32"
        cy="32"
        r="24"
        stroke="currentColor"
        stroke-width="8"
        :stroke-dasharray="`${ringDash} 150.80`"
        transform="rotate(-90 32 32)"
      />
    </template>

    <template v-else>
      <rect
        x="6"
        y="18"
        width="46"
        height="28"
        rx="6"
        stroke="currentColor"
        stroke-width="4"
      />
      <rect x="54" y="26" width="4" height="12" rx="2" fill="currentColor" />
      <rect
        x="10"
        y="22"
        :width="batteryFillWidth"
        height="20"
        rx="3"
        fill="currentColor"
      />
    </template>
  </svg>
</template>

<style scoped>
.tray-style-icon {
  display: block;
  flex: none;
  color: #3ce28a;
}
</style>