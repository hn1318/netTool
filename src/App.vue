<script setup lang="ts">
import { ref, onMounted, markRaw } from "vue";
import Sidebar from "./components/Sidebar.vue";
import Dashboard from "./views/Dashboard.vue";
import Tunnels from "./views/Tunnels.vue";
import CoreDownload from "./views/CoreDownload.vue";
import Settings from "./views/Settings.vue";
import Logs from "./views/Logs.vue";
import UserCenter from "./views/UserCenter.vue";
import About from "./views/About.vue";
import {
  loadStore,
  saveStore,
  STORE_KEYS,
  SAMPLE_TUNNELS,
  DEFAULT_SETTINGS,
} from "./api";
import type { TunnelConfig, AppSettings, LogEntry, UserInfo } from "./types";

// 图标（内联 SVG）
const icons = {
  dashboard: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="7" height="9" rx="1"/><rect x="14" y="3" width="7" height="5" rx="1"/><rect x="14" y="12" width="7" height="9" rx="1"/><rect x="3" y="16" width="7" height="5" rx="1"/></svg>`,
  tunnel: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 2a10 10 0 1 0 10 10"/><path d="M12 6a6 6 0 1 0 6 6"/><path d="M12 10a2 2 0 1 0 2 2"/></svg>`,
  download: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><path d="m7 10 5 5 5-5"/><path d="M12 15V3"/></svg>`,
  settings: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/></svg>`,
  log: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><path d="M14 2v6h6"/><path d="M8 13h8M8 17h5"/></svg>`,
  user: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/><circle cx="12" cy="7" r="4"/></svg>`,
  about: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/></svg>`,
};

const navItems = [
  { id: "dashboard", label: "控制面板", icon: icons.dashboard },
  { id: "tunnels", label: "隧道管理", icon: icons.tunnel },
  { id: "download", label: "核心下载", icon: icons.download },
  { id: "settings", label: "设置", icon: icons.settings },
  { id: "logs", label: "日志", icon: icons.log },
  { id: "user", label: "用户中心", icon: icons.user },
  { id: "about", label: "关于", icon: icons.about },
];

const activeView = ref("dashboard");
const collapsed = ref(false);

// 主题管理
const theme = ref(localStorage.getItem("nettool_theme") || "dark");

function applyTheme(t: string) {
  document.documentElement.setAttribute("data-theme", t);
}

function toggleTheme() {
  theme.value = theme.value === "dark" ? "light" : "dark";
  localStorage.setItem("nettool_theme", theme.value);
  applyTheme(theme.value);
}

function setTheme(t: string) {
  theme.value = t;
  localStorage.setItem("nettool_theme", t);
  applyTheme(t);
}

onMounted(() => {
  applyTheme(theme.value);
});

// 全局状态
const tunnels = ref<TunnelConfig[]>(loadStore<TunnelConfig[]>(STORE_KEYS.tunnels, SAMPLE_TUNNELS));
const settings = ref<AppSettings>(loadStore<AppSettings>(STORE_KEYS.settings, DEFAULT_SETTINGS));
const logs = ref<LogEntry[]>(loadStore<LogEntry[]>(STORE_KEYS.logs, []));
const user = ref<UserInfo>(
  loadStore<UserInfo>(STORE_KEYS.user, {
    nickname: "开发者",
    email: "dev@example.com",
    avatar: "",
    plan: "免费版",
    expireAt: "2026-12-31",
    bandwidth: "12.5 GB / 30 GB",
  })
);

// 隧道操作
function addTunnel(t: TunnelConfig) {
  tunnels.value.push(t);
  persistTunnels();
}
function removeTunnel(id: string) {
  tunnels.value = tunnels.value.filter((t) => t.id !== id);
  persistTunnels();
}
function toggleTunnel(id: string) {
  const t = tunnels.value.find((t) => t.id === id);
  if (t) {
    t.enabled = !t.enabled;
    persistTunnels();
  }
}
function persistTunnels() {
  saveStore(STORE_KEYS.tunnels, tunnels.value);
}

// 设置操作
function updateSettings(s: AppSettings) {
  settings.value = s;
  saveStore(STORE_KEYS.settings, s);
}

// 日志操作
function addLog(entry: LogEntry) {
  logs.value.push(entry);
  if (logs.value.length > 500) logs.value.splice(0, logs.value.length - 500);
  saveStore(STORE_KEYS.logs, logs.value);
}
function clearLogs() {
  logs.value = [];
  saveStore(STORE_KEYS.logs, []);
}

// 用户操作
function updateUser(u: UserInfo) {
  user.value = u;
  saveStore(STORE_KEYS.user, u);
}

// 启动时初始化日志（模拟启动日志）
onMounted(() => {
  if (logs.value.length === 0) {
    addLog({ id: Date.now(), timestamp: timeStr(), level: "info", message: "NetTool 已启动" });
    addLog({ id: Date.now() + 1, timestamp: timeStr(), level: "info", message: "正在加载 FRP 配置..." });
    addLog({ id: Date.now() + 2, timestamp: timeStr(), level: "debug", message: "frpc 核心路径检测完成" });
  }
});

function timeStr() {
  const d = new Date();
  return `${d.getHours().toString().padStart(2, "0")}:${d.getMinutes().toString().padStart(2, "0")}:${d.getSeconds().toString().padStart(2, "0")}`;
}

const viewMap = {
  dashboard: markRaw(Dashboard),
  tunnels: markRaw(Tunnels),
  download: markRaw(CoreDownload),
  settings: markRaw(Settings),
  logs: markRaw(Logs),
  user: markRaw(UserCenter),
  about: markRaw(About),
};
</script>

<template>
  <div class="app-shell">
    <Sidebar
      :items="navItems"
      :active="activeView"
      :collapsed="collapsed"
      :theme="theme"
      @select="activeView = $event"
      @toggle="collapsed = !collapsed"
      @toggle-theme="toggleTheme"
    />

    <main class="content">
      <transition name="view" mode="out-in">
        <component
          :is="viewMap[activeView as keyof typeof viewMap]"
          :key="activeView"
          :tunnels="tunnels"
          :settings="settings"
          :logs="logs"
          :user="user"
          :theme="theme"
          @add="addTunnel"
          @remove="removeTunnel"
          @toggle="toggleTunnel"
          @update="updateSettings"
          @update-user="updateUser"
          @set-theme="setTheme"
          @add-log="addLog"
          @clear="clearLogs"
        />
      </transition>
    </main>
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  height: 100%;
  width: 100%;
}
.content {
  flex: 1;
  overflow-y: auto;
  padding: 24px 28px;
  background: var(--bg);
}
</style>
