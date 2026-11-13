import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  Profile,
  ProfileStore,
  TunnelConfig,
  InstalledCore,
  PlatformInfo,
  ReleaseInfo,
  DownloadResult,
  CoreProgress,
  FrpProcessInfo,
  StartResult,
  TrafficSnapshot,
  ProviderMeta,
  ProviderUser,
  ProviderTunnel,
  ProviderAccount,
  CaptchaInfo,
  AppSettings,
  WebLoginStart,
  LogEntry,
  StorageInfo,
} from "./types";

// ============ 多配置 ============
export const loadProfiles = () => invoke<ProfileStore>("load_profiles");
export const saveProfiles = (store: ProfileStore) =>
  invoke<ProfileStore>("save_profiles", { store });
export const newProfileId = () => invoke<string>("new_profile_id");

// ============ 配置解析 / 渲染 ============
export interface ParsedToml {
  serverAddr: string | null;
  serverPort: number | null;
  user: string | null;
  authMethod: string | null;
  authToken: string | null;
  tunnels: TunnelConfig[];
}
export const parseTomlTunnels = (content: string) =>
  invoke<ParsedToml>("parse_toml_tunnels", { content });
export const previewConfig = (profile: Profile) =>
  invoke<string>("preview_config", { profile });
export const tunnelsToToml = (args: {
  serverAddr: string;
  serverPort: number;
  user: string;
  authToken: string;
  tunnels: TunnelConfig[];
}) => invoke<string>("tunnels_to_toml", args);

export const readConfig = (path: string) => invoke<string>("read_config", { path });
export const writeConfig = (path: string, content: string) =>
  invoke<void>("write_config", { path, content });
export const listDirFiles = (path: string) => invoke<string[]>("list_dir_files", { path });
export const getAppDataDir = () => invoke<string>("get_app_data_dir");
export const revealPath = (path: string) => invoke<void>("reveal_path", { path });

// ============ 核心下载 ============
export const detectPlatform = () => invoke<PlatformInfo>("detect_platform");
export const getInstalledCore = () => invoke<InstalledCore | null>("get_installed_core");
export const listFrpReleases = (limit = 8) =>
  invoke<ReleaseInfo[]>("list_frp_releases", { limit });
export const downloadFrp = (version: string, os?: string, arch?: string) =>
  invoke<DownloadResult>("download_frp", { version, os, arch });
export const verifyCore = (path: string) => invoke<string>("verify_core", { path });

// ============ 运行控制 ============
export const startFrpc = (profile?: Profile, frpcPath?: string) =>
  invoke<StartResult>("start_frpc", { profile, frpcPath });
export const stopFrpc = () => invoke<FrpProcessInfo>("stop_frpc");
export const getFrpStatus = () => invoke<FrpProcessInfo>("get_frp_status");
export const getTraffic = () => invoke<TrafficSnapshot>("get_traffic");
export const resetTraffic = () => invoke<void>("reset_traffic");
export const emitLog = (level: string, message: string) =>
  invoke<void>("emit_log", { level, message });

// ============ 第三方服务（通用） ============
export const providerList = () => invoke<ProviderMeta[]>("provider_list");
export const providerAccounts = () => invoke<ProviderAccount[]>("provider_accounts");
export const providerLoginKey = (provider: string, key: string) =>
  invoke<ProviderUser>("provider_login_key", { provider, key });
export const providerLoginPassword = (
  provider: string,
  username: string,
  password: string,
  captcha: string
) => invoke<ProviderUser>("provider_login_password", { provider, username, password, captcha });
export const providerLoginCode = (provider: string, code: string) =>
  invoke<ProviderUser>("provider_login_code", { provider, code });
export const providerAuthUrl = (provider: string) =>
  invoke<string>("provider_auth_url", { provider });
/** OpenFrp 网页登录：申请授权，返回浏览器授权地址 */
export const providerWebLoginStart = (provider: string) =>
  invoke<WebLoginStart>("provider_web_login_start", { provider });
/** 轮询网页登录：null 表示尚未授权，返回用户信息表示登录成功 */
export const providerWebLoginPoll = (provider: string, requestUuid: string) =>
  invoke<ProviderUser | null>("provider_web_login_poll", { provider, requestUuid });
/** 取消网页登录，清理后端一次性密钥 */
export const providerWebLoginCancel = (requestUuid: string) =>
  invoke<void>("provider_web_login_cancel", { requestUuid });
export const providerSign = (provider: string, captcha: string) =>
  invoke<string>("provider_sign", { provider, captcha });
export const providerLogout = (provider: string) =>
  invoke<void>("provider_logout", { provider });
export const providerUser = (provider: string) =>
  invoke<ProviderUser>("provider_user", { provider });
export const providerTunnels = (provider: string) =>
  invoke<ProviderTunnel[]>("provider_tunnels", { provider });
export const providerConfig = (provider: string, tunnelIds: string[]) =>
  invoke<string>("provider_config", { provider, tunnelIds });
export const providerImport = (provider: string, tunnelIds: string[], name?: string) =>
  invoke<Profile>("provider_import", { provider, tunnelIds, name });
export const providerKick = (provider: string, tunnelId: string) =>
  invoke<void>("provider_kick", { provider, tunnelId });
export const providerToggle = (provider: string, tunnelId: string, disabled: boolean) =>
  invoke<void>("provider_toggle", { provider, tunnelId, disabled });
export const providerCaptchaInfo = (provider: string) =>
  invoke<CaptchaInfo>("provider_captcha_info", { provider });
export const providerDecodeCaptcha = (raw: string) =>
  invoke<string>("provider_decode_captcha", { raw });

/** 打开外部链接：Tauri 环境走系统浏览器，浏览器环境回退 window.open */
export async function openExternal(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank");
  }
}

// ============ 应用设置 / 关闭行为 ============
export const getAppSettings = () => invoke<AppSettings>("get_app_settings");
export const saveAppSettings = (settings: AppSettings) =>
  invoke<AppSettings>("save_app_settings", { settings });
/** action: tray 后台运行 / quit 直接关闭 / cancel 取消 */
export const closeAction = (action: string, remember: boolean) =>
  invoke<void>("close_action", { action, remember });
export const onAppCloseRequest = (cb: () => void) =>
  listen("app-close-request", () => cb());

// ============ 存储位置 / 开机自启动 ============
export const getStorageInfo = () => invoke<StorageInfo>("get_storage_info");
export const pickDirectory = (defaultPath?: string) =>
  invoke<string | null>("pick_directory", { defaultPath });
export const applyStorageSettings = (args: {
  dataDir?: string | null;
  logDir?: string | null;
  migrate: boolean;
}) => invoke<string>("apply_storage_settings", args);
export const getAutostart = () => invoke<boolean>("get_autostart");
export const setAutostart = (enabled: boolean) =>
  invoke<string>("set_autostart", { enabled });

// ============ 事件 ============
export const onFrpStatus = (cb: (data: FrpProcessInfo) => void) =>
  listen<FrpProcessInfo>("frp-status", (e) => cb(e.payload));
export const onFrpLog = (cb: (data: any) => void) =>
  listen("frp-log", (e) => cb(e.payload));
export const onFrpTraffic = (cb: (data: TrafficSnapshot) => void) =>
  listen<TrafficSnapshot>("frp-traffic", (e) => cb(e.payload));
export const onCoreDownload = (cb: (data: CoreProgress) => void) =>
  listen<CoreProgress>("core-download", (e) => cb(e.payload));

// ============ 本地轻量持久化（界面偏好） ============
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
  try {
    localStorage.setItem(PREFIX + key, JSON.stringify(value));
  } catch {
    /* 忽略配额错误 */
  }
}

export const STORE_KEYS = {
  settings: "settings",
  logs: "logs",
  user: "user",
  frpVersion: "frp_version",
  theme: "theme",
} as const;

// ============ 日志清洗 ============

/** 去掉 ANSI 颜色/控制序列 */
export function stripAnsi(input: string): string {
  // eslint-disable-next-line no-control-regex
  return input.replace(/\u001b\[[0-9;?]*[ -/]*[@-~]/g, "").replace(/\r/g, "");
}

/**
 * 旧版本把 frpc 原始行（含 ANSI 颜色码与重复时间戳）直接存进了日志。
 * 这里统一清洗：去色、提取 HH:MM:SS 时间戳与 [I]/[W]/[E] 级别。
 */
export function normalizeLogEntry(entry: LogEntry): LogEntry {
  let message = entry.message || "";
  let timestamp = entry.timestamp || "";
  let level = entry.level || "info";
  if (!message.includes("\u001b") && !/^\s*\d{4}-\d{2}-\d{2}\s/.test(message)) {
    return entry;
  }
  message = stripAnsi(message).trim();
  // 形如 2026-11-09 00:06:37.514 [I] [client/x.go:1] msg
  const m = message.match(/^(\d{4}-\d{2}-\d{2}\s+)?(\d{2}:\d{2}:\d{2})(?:\.\d+)?\s*\[([IWEDT])\]\s*(.*)$/);
  if (m) {
    timestamp = m[2];
    const map: Record<string, LogEntry["level"]> = {
      I: "info",
      W: "warn",
      E: "error",
      D: "debug",
      T: "trace",
    };
    level = map[m[3]] || level;
    message = m[4].trim();
  } else {
    message = message.replace(
      /^(\d{4}-\d{2}-\d{2}\s+)?(\d{2}:\d{2}:\d{2})(?:\.\d+)?\s*/,
      (_full, _date, time: string) => {
        timestamp = time;
        return "";
      }
    );
  }
  return { ...entry, timestamp, level, message };
}

export function normalizeLogEntries(entries: LogEntry[]): LogEntry[] {
  return entries.map(normalizeLogEntry);
}

// ============ 工具函数 ============
export function genId(): string {
  return Math.random().toString(36).slice(2, 10);
}

export function fmtBytes(bytes: number): string {
  if (!bytes) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(i === 0 ? 0 : v >= 100 ? 0 : 1)} ${units[i]}`;
}

export function fmtSpeed(bytesPerSec: number): string {
  return `${fmtBytes(bytesPerSec)}/s`;
}

/** HTTP(S) 隧道的访问地址（取第一个自定义域名；无域名时用子域名） */
export function tunnelAccessUrl(t: TunnelConfig): string {
  if (t.type !== "http" && t.type !== "https") return "";
  const domain =
    (t.customDomains || "")
      .split(",")
      .map((d) => d.trim())
      .filter(Boolean)[0] || (t.subdomain || "").trim();
  return domain ? `${t.type}://${domain}` : "";
}

/** 隧道远端展示文本：端口 / 域名 / 密钥 */
export function tunnelRemoteText(t: TunnelConfig): string {
  if (t.type === "tcp" || t.type === "udp") {
    return t.remotePort ? `:${t.remotePort}` : "-";
  }
  if (t.type === "stcp" || t.type === "xtcp") {
    return t.secretKey ? "密钥隧道" : "密钥隧道（未设密钥）";
  }
  return tunnelAccessUrl(t) || "未配置域名";
}

export const EMPTY_SNAPSHOT: TrafficSnapshot = {
  running: false,
  upSpeed: 0,
  downSpeed: 0,
  upTotal: 0,
  downTotal: 0,
  conns: 0,
  profileId: "",
  profileName: "",
  startedAt: 0,
  tunnels: [],
  updatedAt: 0,
};

export function emptyProfile(id: string, name = "新配置"): Profile {
  return {
    id,
    name,
    serverAddr: "",
    serverPort: 7000,
    user: "",
    authToken: "",
    transport: "tcp",
    tlsEnable: true,
    logLevel: "info",
    tunnels: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
}

// 内置版本回退（GitHub API 不可用时使用）
export const FRP_VERSION_FALLBACK: ReleaseInfo[] = [
  { tag: "v0.72.0", version: "0.72.0", publishedAt: "-", prerelease: false, sizeHint: "" },
  { tag: "v0.71.1", version: "0.71.1", publishedAt: "-", prerelease: false, sizeHint: "" },
  { tag: "v0.71.0", version: "0.71.0", publishedAt: "-", prerelease: false, sizeHint: "" },
  { tag: "v0.62.1", version: "0.62.1", publishedAt: "-", prerelease: false, sizeHint: "" },
  { tag: "v0.61.1", version: "0.61.1", publishedAt: "-", prerelease: false, sizeHint: "" },
];
