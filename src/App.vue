<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";

const MainWindow = defineAsyncComponent(() => import("./windows/MainWindow.vue").then((m) => m.default));
const TrayOverlayWindow = defineAsyncComponent(() => import("./windows/TrayOverlayWindow.vue").then((m) => m.default));
const AddDeviceWindow = defineAsyncComponent(() => import("./windows/AddDeviceWindow.vue").then((m) => m.default));
const InspectorWindow = defineAsyncComponent(() => import("./windows/InspectorWindow.vue").then((m) => m.default));
const AlertWindow = defineAsyncComponent(() => import("./windows/AlertWindow.vue").then((m) => m.default));
const TrayMenuWindow = defineAsyncComponent(() => import("./windows/TrayMenuWindow.vue").then((m) => m.default));
const BackgroundWindow = defineAsyncComponent(() => import("./windows/BackgroundWindow.vue").then((m) => m.default));

const props = defineProps<{ windowLabel?: string }>();

const views = {
  main: MainWindow,
  popup: TrayOverlayWindow,
  tooltip: TrayOverlayWindow,
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
