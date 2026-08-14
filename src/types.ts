// 隧道配置类型
export interface TunnelConfig {
  id: string;
  name: string;
  type: "tcp" | "udp" | "http" | "https" | "stcp" | "xtcp";
  localIp: string;
  localPort: number;
  remotePort: number;
  customDomains?: string;
  enabled: boolean;
  source?: "toml" | "json" | "manual";
}

// FRP 版本信息
export interface FrpVersion {
  version: string;
  tag: string;
  publishedAt: string;
}

export interface FrpPlatform {
  os: "windows" | "linux" | "darwin" | "freebsd";
  arch: "amd64" | "arm64" | "386" | "arm";
}

// 日志条目
export interface LogEntry {
  id: number;
  timestamp: string;
  level: "info" | "warn" | "error" | "debug" | "trace";
  message: string;
}

export type LogLevel = LogEntry["level"];

// 设置
export interface AppSettings {
  serverAddr: string;
  serverPort: number;
  authToken: string;
  user: string;
  logLevel: LogLevel;
  logFile: string;
  logMaxDays: number;
  transport: "tcp" | "kcp" | "websocket";
  tlsEnable: boolean;
  dashboardAddr: string;
  dashboardPort: number;
  dashboardUser: string;
  dashboardPwd: string;
}

// 用户信息
export interface UserInfo {
  nickname: string;
  email: string;
  avatar: string;
  plan: string;
  expireAt: string;
  bandwidth: string;
}
