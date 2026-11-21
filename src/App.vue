<script setup lang="ts">
import { computed } from "vue";
import MainWindow from "./windows/MainWindow.vue";
import PopupWindow from "./windows/PopupWindow.vue";
import TooltipWindow from "./windows/TooltipWindow.vue";
import AddDeviceWindow from "./windows/AddDeviceWindow.vue";
import InspectorWindow from "./windows/InspectorWindow.vue";
import AlertWindow from "./windows/AlertWindow.vue";
import TrayMenuWindow from "./windows/TrayMenuWindow.vue";
import BackgroundWindow from "./windows/BackgroundWindow.vue";

const props = defineProps<{ windowLabel?: string }>();

const views = {
  main: MainWindow,
  popup: PopupWindow,
  tooltip: TooltipWindow,
  add: AddDeviceWindow,
  inspector: InspectorWindow,
  alert: AlertWindow,
  menu: TrayMenuWindow,
  bg: BackgroundWindow,
} as const;

const view = computed(() => views[props.windowLabel as keyof typeof views] ?? MainWindow);
</script>

<template>
  <component :is="view" :window-label="props.windowLabel" />
</template>
