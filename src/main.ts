import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.vue";
import { installScrollFade } from "./lib/scrollFade";
import { useConfigStore } from "./stores/config";
import { useDevicesStore } from "./stores/devices";
import "./styles/tokens.css";
import "./styles/base.css";

function currentWindowLabel(): string {
  const query = new URLSearchParams(window.location.search).get("window");
  if (query) return query;
  try {
    return getCurrentWindow().label;
  } catch {
    return "main";
  }
}

const pinia = createPinia();
const app = createApp(App, { windowLabel: currentWindowLabel() });

// 屏蔽 WebView2 默认右键菜单（另存为 / 打印等）
document.addEventListener("contextmenu", (event) => event.preventDefault());

// 全局滚动条：滑动时出现，停手后隐藏（所有窗口共用）
installScrollFade();

app.use(pinia);
async function bootstrap(): Promise<void> {
  const results = await Promise.allSettled([
    useConfigStore(pinia).startEvents(),
    useDevicesStore(pinia).startEvents(),
  ]);
  for (const result of results) {
    if (result.status === 'rejected') console.error('全局事件订阅失败', result.reason);
  }
  app.mount("#app");
}

void bootstrap();
