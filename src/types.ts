export type TunnelType = "tcp" | "udp" | "http" | "https" | "stcp" | "xtcp" | "tcpmux";

// 单条隧道配置
export interface TunnelConfig {
  id: string;
  name: string;
  type: TunnelType;
  localIp: string;
  localPort: number;
  remotePort: number;
  customDomains?: string;
  subdomain?: string;
  secretKey?: string;
  useEncryption?: boolean;
  useCompression?: boolean;
  bandwidthLimit?: string;
  enabled: boolean;
  source?: "toml" | "json" | "manual";
}

// 一套完整的内网穿透配置（多配置管理）
export interface Profile {
  id: string;
  name: string;
  serverAddr: string;
  serverPort: number;
  user: string;
  authToken: string;
  transport: "tcp" | "kcp" | "websocket";
  tlsEnable: boolean;
  logLevel: LogLevel;
  tunnels: TunnelConfig[];
  /** 配置来源：空表示手动配置，mefrp 表示来自第三方服务 */
  provider?: string;
  /** 第三方服务中的隧道 ID */
  providerProxyIds?: number[];
  createdAt: number;
  updatedAt: number;
}

export interface ProfileStore {
  activeId: string;
  profiles: Profile[];
}

// ============ 第三方服务（通用） ============
export interface ProviderMeta {
  id: string;
  name: string;
  site: string;
  /** 支持的登录方式：key / password / web / code */
  loginModes: string[];
  /** 密码登录是否需要人机验证 */
  captcha: boolean;
  note: string;
}

export interface KV {
  label: string;
  value: string;
}

export interface ProviderUser {
  provider: string;
  userId: string;
  username: string;
  email: string;
  group: string;
  tunnelLimit: number;
  tunnelUsed: number;
  traffic: string;
  signed: boolean | null;
  extras: KV[];
}

export interface ProviderTunnel {
  id: string;
  name: string;
  proxyType: string;
  localIp: string;
  localPort: number;
  remotePort: number;
  domains: string;
  remoteDesc: string;
  /** 隧道限速/带宽（服务商返回时展示） */
  speed: string;
  node: string;
  online: boolean;
  disabled: boolean;
  banned: boolean;
}

export interface ProviderAccount {
  provider: string;
  username: string;
  email: string;
  loginMethod: string;
  tokenTail: string;
  savedAt: number;
}

export interface CaptchaInfo {
  url: string;
  client: string;
}

/** OpenFrp 网页登录发起结果 */
export interface WebLoginStart {
  url: string;
  requestUuid: string;
}

// 已安装核心
export interface InstalledCore {
  path: string;
  version: string;
  os: string;
  arch: string;
  installedAt: number;
  size: number;
}

export interface PlatformInfo {
  os: string;
  arch: string;
  installed: InstalledCore | null;
}

export interface ReleaseInfo {
  tag: string;
  version: string;
  publishedAt: string;
  prerelease: boolean;
  sizeHint: string;
}

export interface DownloadResult {
  path: string;
  version: string;
  size: number;
  os: string;
  arch: string;
}

export interface CoreProgress {
  stage: "download" | "retry" | "extract" | "done" | "error" | string;
  message: string;
  downloaded: number;
  total: number;
  percent: number;
}

export interface FrpProcessInfo {
  pid: number;
  running: boolean;
  profileId: string;
  profileName: string;
  configPath: string;
  webPort: number;
  startedAt: number;
}

export interface GatewayBinding {
  tunnelId: string;
  name: string;
  port: number;
  target: string;
}

export interface StartResult {
  pid: number;
  profileId: string;
  profileName: string;
  configPath: string;
  webPort: number;
  gateways: GatewayBinding[];
}

// 单条隧道的真实流量
export interface TunnelTraffic {
  id: string;
  name: string;
  type: string;
  target: string;
  gatewayPort: number;
  upSpeed: number;
  downSpeed: number;
  upTotal: number;
  downTotal: number;
  conns: number;
  connTotal: number;
  status: string;
  err: string;
  remoteAddr: string;
}

export interface TrafficSnapshot {
  running: boolean;
  upSpeed: number;
  downSpeed: number;
  upTotal: number;
  downTotal: number;
  conns: number;
  profileId: string;
  profileName: string;
  startedAt: number;
  tunnels: TunnelTraffic[];
  updatedAt: number;
}

// 日志
export interface LogEntry {
  id: number;
  timestamp: string;
  level: LogLevel;
  message: string;
}

export type LogLevel = "info" | "warn" | "error" | "debug" | "trace";

// 用户信息（本地展示）
export interface UserInfo {
  nickname: string;
  email: string;
  avatar: string;
  plan: string;
  expireAt: string;
  bandwidth: string;
}

// 应用级设置
export interface AppSettings {
  /** ask 每次询问 / tray 后台运行 / quit 直接退出 */
  closeBehavior: "ask" | "tray" | "quit";
  /** 自定义数据目录（空表示默认） */
  dataDir?: string | null;
  /** 自定义日志目录（空表示 {数据目录}/logs） */
  logDir?: string | null;
}

/** 存储位置信息 */
export interface StorageInfo {
  defaultDataDir: string;
  defaultLogDir: string;
  dataDir: string;
  logDir: string;
}
