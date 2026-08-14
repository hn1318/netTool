import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TunnelConfig, FrpVersion, AppSettings } from "./types";

// ---- 后端命令封装 ----
export const getAppDataDir = () => invoke<string>("get_app_data_dir");
export const readConfig = (path: string) => invoke<string>("read_config", { path });
export const writeConfig = (path: string, content: string) =>
  invoke<void>("write_config", { path, content });
export const parseToml = (content: string) => invoke<any>("parse_toml", { content });
export const jsonToToml = (content: string) => invoke<string>("json_to_toml", { content });
export const listDirFiles = (path: string) => invoke<string[]>("list_dir_files", { path });
export const startFrp = () => invoke<any>("start_frp");
export const stopFrp = () => invoke<any>("stop_frp");
export const getFrpStatus = () => invoke<any>("get_frp_status");
export const emitLog = (level: string, message: string) =>
  invoke<void>("emit_log", { level, message });

// ---- 事件监听 ----
export const onFrpStatus = (cb: (data: any) => void) =>
  listen("frp-status", (e) => cb(e.payload));
export const onFrpLog = (cb: (data: any) => void) =>
  listen("frp-log", (e) => cb(e.payload));

// ---- 本地存储持久化 ----
const PREFIX = "nettool_";

export function loadStore<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(PREFIX + key);
    if (raw == null) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function saveStore<T>(key: string, value: T): void {
  localStorage.setItem(PREFIX + key, JSON.stringify(value));
}

export const STORE_KEYS = {
  tunnels: "tunnels",
  settings: "settings",
  logs: "logs",
  user: "user",
  frpVersions: "frp_versions",
  selectedConfig: "selected_config",
  corePath: "core_path",
} as const;

// ---- FRP 版本列表（模拟远程数据）----
export const FRP_VERSIONS: FrpVersion[] = [
  { version: "0.61.1", tag: "v0.61.1", publishedAt: "2025-08-02" },
  { version: "0.61.0", tag: "v0.61.0", publishedAt: "2025-07-18" },
  { version: "0.60.0", tag: "v0.60.0", publishedAt: "2025-06-30" },
  { version: "0.59.1", tag: "v0.59.1", publishedAt: "2025-05-12" },
  { version: "0.58.1", tag: "v0.58.1", publishedAt: "2025-04-25" },
  { version: "0.57.0", tag: "v0.57.0", publishedAt: "2025-03-09" },
];

// ---- 默认设置 ----
export const DEFAULT_SETTINGS: AppSettings = {
  serverAddr: "frp.example.com",
  serverPort: 7000,
  authToken: "",
  user: "default_user",
  logLevel: "info",
  logFile: "./frpc.log",
  logMaxDays: 3,
  transport: "tcp",
  tlsEnable: true,
  dashboardAddr: "0.0.0.0",
  dashboardPort: 7400,
  dashboardUser: "admin",
  dashboardPwd: "admin",
};

// ---- 默认隧道示例 ----
export function genId(): string {
  return Math.random().toString(36).slice(2, 10);
}

export const SAMPLE_TUNNELS: TunnelConfig[] = [
  {
    id: genId(),
    name: "ssh",
    type: "tcp",
    localIp: "127.0.0.1",
    localPort: 22,
    remotePort: 6000,
    enabled: true,
    source: "manual",
  },
  {
    id: genId(),
    name: "web",
    type: "http",
    localIp: "127.0.0.1",
    localPort: 8080,
    remotePort: 80,
    customDomains: "web.example.com",
    enabled: true,
    source: "manual",
  },
];
