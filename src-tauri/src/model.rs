use serde::{Deserialize, Serialize};

fn default_true() -> bool {
    true
}

fn default_proxy_type() -> String {
    "tcp".to_string()
}

fn default_local_ip() -> String {
    "127.0.0.1".to_string()
}

fn default_transport() -> String {
    "tcp".to_string()
}

fn default_log_level() -> String {
    "info".to_string()
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn now_hms() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // 本地时区偏移由系统提供，这里用 chrono 之外的简易换算（UTC+8 现场）
    let secs = secs + 8 * 3600;
    let s = secs % 86400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s % 3600) / 60, s % 60)
}

/// 单条隧道（代理）配置，字段与前端 TunnelConfig 一一对应
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelConfig {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type", default = "default_proxy_type")]
    pub proxy_type: String,
    /// 前端历史字段名为 localIp，旧 TOML 解析产出为 localIP，两者都接受
    #[serde(alias = "localIP", default = "default_local_ip")]
    pub local_ip: String,
    #[serde(default)]
    pub local_port: u16,
    #[serde(default)]
    pub remote_port: u16,
    #[serde(default)]
    pub custom_domains: Option<String>,
    #[serde(default)]
    pub subdomain: Option<String>,
    #[serde(default)]
    pub secret_key: Option<String>,
    #[serde(default)]
    pub use_encryption: Option<bool>,
    #[serde(default)]
    pub use_compression: Option<bool>,
    #[serde(default)]
    pub bandwidth_limit: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub source: Option<String>,
}

impl Default for TunnelConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            proxy_type: default_proxy_type(),
            local_ip: default_local_ip(),
            local_port: 0,
            remote_port: 0,
            custom_domains: None,
            subdomain: None,
            secret_key: None,
            use_encryption: None,
            use_compression: None,
            bandwidth_limit: None,
            enabled: true,
            source: None,
        }
    }
}

impl TunnelConfig {
    /// frp 中以 TCP 承载的代理类型（可使用本地计量网关做流量统计）
    pub fn is_tcp_based(&self) -> bool {
        matches!(
            self.proxy_type.as_str(),
            "tcp" | "http" | "https" | "stcp" | "xtcp" | "tcpmux"
        )
    }

    pub fn target(&self) -> String {
        format!("{}:{}", self.local_ip, self.local_port)
    }
}

/// 一套完整的内网穿透配置（多配置管理的核心单位）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub server_addr: String,
    #[serde(default)]
    pub server_port: u16,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub auth_token: String,
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(default = "default_true")]
    pub tls_enable: bool,
    #[serde(default = "default_log_level")]
    pub log_level: String,
    #[serde(default)]
    pub tunnels: Vec<TunnelConfig>,
    /// 配置来源：为空表示手动配置，`mefrp` 表示来自第三方服务
    #[serde(default)]
    pub provider: Option<String>,
    /// 第三方服务中的隧道 ID，便于后续同步
    #[serde(default)]
    pub provider_proxy_ids: Vec<i64>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub updated_at: u64,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "新配置".to_string(),
            server_addr: String::new(),
            server_port: 7000,
            user: String::new(),
            auth_token: String::new(),
            transport: default_transport(),
            tls_enable: true,
            log_level: default_log_level(),
            tunnels: Vec::new(),
            provider: None,
            provider_proxy_ids: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }
    }
}

impl Profile {
    pub fn enabled_tunnels(&self) -> Vec<&TunnelConfig> {
        self.tunnels.iter().filter(|t| t.enabled).collect()
    }
}

/// profiles.json 的持久化结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStore {
    #[serde(default)]
    pub active_id: String,
    #[serde(default)]
    pub profiles: Vec<Profile>,
}

impl ProfileStore {
    pub fn find(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn active(&self) -> Option<&Profile> {
        if !self.active_id.is_empty() {
            if let Some(p) = self.find(&self.active_id) {
                return Some(p);
            }
        }
        self.profiles.first()
    }
}

/// 已安装的 frpc 核心元数据
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstalledCore {
    pub path: String,
    pub version: String,
    pub os: String,
    pub arch: String,
    pub installed_at: u64,
    pub size: u64,
}
