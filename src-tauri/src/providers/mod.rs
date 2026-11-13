//! 第三方 FRP 服务商接入（通用引擎）。
//!
//! 每个服务商实现同一组能力：`login_key` / `login_password` / `user` / `tunnels` / `profile`，
//! 上层的命令、账号持久化、配置导入与 TOML 渲染全部复用，新增服务商只需新增一个子模块。

pub mod chmlfrp;
pub mod mefrp;
pub mod msl;
pub mod openfrp;
pub mod sakura;

use crate::config::{parse_config_text, render_config, ParseResult};
use crate::model::{now_ms, Profile};
use crate::store::{gen_id, read_store, write_store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// 统一 UA（部分服务商要求自定义 UA，禁止伪装成官方客户端）
pub const UA: &str = "NetTool/0.1.0 (Tauri desktop frp manager)";

// ============ 通用数据类型 ============

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMeta {
    pub id: String,
    pub name: String,
    pub site: String,
    /// 支持的登录方式："key" / "password"
    pub login_modes: Vec<String>,
    /// 密码登录是否需要人机验证
    pub captcha: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KV {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderUser {
    pub provider: String,
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub group: String,
    pub tunnel_limit: i64,
    pub tunnel_used: i64,
    /// 流量展示文本（各服务商单位不同，统一由后端格式化）
    pub traffic: String,
    pub signed: Option<bool>,
    pub extras: Vec<KV>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderTunnel {
    pub id: String,
    pub name: String,
    pub proxy_type: String,
    pub local_ip: String,
    pub local_port: i64,
    pub remote_port: i64,
    pub domains: String,
    /// 远端访问信息展示文本（端口 / 域名）
    pub remote_desc: String,
    /// 隧道限速/带宽（服务商返回时展示）
    pub speed: String,
    pub node: String,
    pub online: bool,
    pub disabled: bool,
    pub banned: bool,
}

// ============ 账号持久化 ============

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderAccount {
    pub provider: String,
    pub token: String,
    pub username: String,
    pub email: String,
    pub login_method: String,
    pub saved_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStore {
    #[serde(default)]
    pub accounts: Vec<ProviderAccount>,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountView {
    pub provider: String,
    pub username: String,
    pub email: String,
    pub login_method: String,
    pub token_tail: String,
    pub saved_at: u64,
}

fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(crate::store::data_dir(app)?.join("providers.json"))
}

fn load_store(app: &tauri::AppHandle) -> ProviderStore {
    let Ok(path) = store_path(app) else {
        return ProviderStore::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return ProviderStore::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_store(app: &tauri::AppHandle, store: &ProviderStore) -> Result<(), String> {
    let path = store_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

fn save_account(app: &tauri::AppHandle, account: ProviderAccount) -> Result<(), String> {
    let mut store = load_store(app);
    store.accounts.retain(|a| a.provider != account.provider);
    store.accounts.push(account);
    save_store(app, &store)
}

fn token_of(app: &tauri::AppHandle, provider: &str) -> Result<String, String> {
    let acc = load_store(app)
        .accounts
        .into_iter()
        .find(|a| a.provider == provider)
        .ok_or_else(|| format!("尚未登录 {}", display_name(provider)))?;
    if acc.token.trim().is_empty() {
        return Err("登录凭据为空，请重新登录".to_string());
    }
    Ok(acc.token)
}

fn tail(token: &str) -> String {
    token.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect()
}

// ============ 服务商注册表 ============

pub fn provider_metas() -> Vec<ProviderMeta> {
    vec![
        ProviderMeta {
            id: "mefrp".into(),
            name: "ME Frp".into(),
            site: "https://www.mefrp.com".into(),
            login_modes: vec!["key".into(), "password".into()],
            captcha: true,
            note: "访问密钥即 API Token；密码登录需配合人机验证".into(),
        },
        ProviderMeta {
            id: "openfrp".into(),
            name: "OpenFrp".into(),
            site: "https://www.openfrp.net".into(),
            // web = 文档推荐的 Remote Login 网页授权；key = 旧版 Authorization 粘贴
            login_modes: vec!["web".into(), "key".into()],
            captcha: false,
            note: "推荐「网页登录」：拉起 OpenFrp 授权页，授权完成后自动登录。旧版 Authorization（OPENFRP...）仍可粘贴使用".into(),
        },
        ProviderMeta {
            id: "chmlfrp".into(),
            name: "ChmlFrp".into(),
            site: "https://www.chmlfrp.net".into(),
            login_modes: vec!["key".into()],
            captcha: false,
            note: "在面板「用户中心」复制访问令牌（access_token）后粘贴登录，程序按 v2 文档读取隧道与配置".into(),
        },
        ProviderMeta {
            id: "sakura".into(),
            name: "SakuraFrp".into(),
            site: "https://www.natfrp.com".into(),
            login_modes: vec!["key".into()],
            captcha: false,
            note: "使用「账户信息 → 访问密钥」，隧道配置由服务端下发".into(),
        },
        ProviderMeta {
            id: "msl".into(),
            name: "MSL Frp".into(),
            site: "https://user.mslmc.net".into(),
            login_modes: vec!["key".into()],
            captcha: false,
            note: "在 MSL 用户中心复制 Token 后粘贴登录，隧道配置由服务端生成".into(),
        },
    ]
}

fn display_name(id: &str) -> String {
    provider_metas()
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| p.name)
        .unwrap_or_else(|| id.to_string())
}

fn unsupported<T>(provider: &str, what: &str) -> Result<T, String> {
    Err(format!("{} 暂不支持{}", display_name(provider), what))
}

// ============ 工具 ============

pub fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(UA)
        .build()
        .map_err(|e| e.to_string())
}

/// 用于拼接查询串（节点名可能包含中文）
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 把服务端返回的域名（可能是 `["a.com"]` 文本化数组 / 逗号分隔 / 带协议）统一成逗号分隔
pub fn normalize_domain_text(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() || t == "[]" {
        return String::new();
    }
    if t.starts_with('[') {
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(t) {
            return arr
                .into_iter()
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect::<Vec<_>>()
                .join(",");
        }
    }
    t.trim_matches('"')
        .trim_matches('\'')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .replace(' ', "")
}

/// 数字转带单位的流量文本（输入按 MiB 处理）
pub fn mib_text(mib: i64) -> String {
    mb_text(mib as f64)
}

/// 数字转带单位的流量文本（输入按字节处理，如 SakuraFrp）
pub fn bytes_text(bytes: i64) -> String {
    if bytes <= 0 {
        return "0 B".to_string();
    }
    let mut v = bytes as f64;
    let mut unit = "B";
    for next in ["KB", "MB", "GB", "TB"] {
        if v >= 1024.0 {
            v /= 1024.0;
            unit = next;
        } else {
            break;
        }
    }
    if unit == "B" {
        format!("{v:.0} B")
    } else {
        format!("{v:.2} {unit}")
    }
}

/// Kbps → Mbps 展示
pub fn kbps_text(kbps: i64) -> String {
    if kbps <= 0 {
        return String::new();
    }
    let mbps = kbps as f64 / 1024.0;
    if mbps >= 1.0 {
        format!("{mbps:.1} Mbps")
    } else {
        format!("{kbps} Kbps")
    }
}

/// ME Frp 的 `traffic` 字段单位为 **MB**。
///
/// 面板逻辑（me-frp v5 UserInfoGrid）：`>= 1024` 显示 `GB`，否则显示 `MB`，均保留两位小数；
/// `<= 0` 显示「暂无」。
pub fn mefrp_traffic_text(mb: i64) -> String {
    if mb <= 0 {
        return "暂无".to_string();
    }
    if mb >= 1024 {
        format!("{:.2} GB", mb as f64 / 1024.0)
    } else {
        format!("{:.2} MB", mb as f64)
    }
}

/// ME Frp 的 `inBound` / `outBound` 单位为 **1/128 Mbps**。
///
/// 面板逻辑：`outBound / 128` Mbps / `inBound / 128` Mbps（8320 → 65 Mbps）。
pub fn mefrp_mbps_text(unit: i64) -> String {
    if unit <= 0 {
        return "-".to_string();
    }
    let mbps = unit as f64 / 128.0;
    if (mbps.fract()).abs() < 0.05 {
        format!("{mbps:.0} Mbps")
    } else {
        format!("{mbps:.1} Mbps")
    }
}

fn mb_text(mb: f64) -> String {
    if mb <= 0.0 {
        return "-".to_string();
    }
    if mb >= 1024.0 {
        format!("{:.2} GB", mb / 1024.0)
    } else {
        format!("{:.1} MB", mb)
    }
}

/// Unix 秒 → YYYY-MM-DD（无时区依赖，按 UTC 换算）
pub fn fmt_date(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }
    let days = ts.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// 由解析结果生成 Profile（统一走 TOML 渲染，保证格式一致）
pub(crate) fn profile_from_parsed(
    parsed: ParseResult,
    provider: &str,
    _ids: &[String],
    name: Option<&str>,
    fallback_token: &str,
) -> Result<Profile, String> {
    if parsed.tunnels.is_empty() {
        return Err("配置中未解析到隧道".to_string());
    }
    let now = now_ms();
    Ok(Profile {
        id: gen_id(),
        name: name
            .filter(|n| !n.trim().is_empty())
            .map(|n| n.trim().to_string())
            .unwrap_or_else(|| display_name(provider)),
        server_addr: parsed.server_addr.unwrap_or_default(),
        server_port: parsed.server_port.unwrap_or(7000),
        user: parsed.user.unwrap_or_default(),
        auth_token: parsed
            .auth_token
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| fallback_token.to_string()),
        transport: "tcp".into(),
        tls_enable: true,
        log_level: "info".into(),
        tunnels: parsed.tunnels,
        provider: Some(provider.to_string()),
        provider_proxy_ids: Vec::new(),
        created_at: now,
        updated_at: now,
    })
}

/// 把 Profile 的隧道名/ID 与远端 ID 绑定，供后续同步使用
pub(crate) fn bind_ids(mut profile: Profile, ids: &[String]) -> Profile {
    profile.provider_proxy_ids = ids
        .iter()
        .filter_map(|s| s.trim().parse::<i64>().ok())
        .collect();
    profile
}

// ============ 统一调度 ============

fn login_key(app: &tauri::AppHandle, provider: &str, key: String) -> Result<ProviderUser, String> {
    let token = key.trim().to_string();
    if token.is_empty() {
        return Err("凭据不能为空".to_string());
    }
    let (user, stored) = match provider {
        "mefrp" => (mefrp::login_key(&token)?, token.clone()),
        "openfrp" => {
            let t = openfrp::normalize_token(&token);
            match openfrp::login_key(&t) {
                Ok(u) => (u, t),
                Err(e) if openfrp::is_access_key(&token) => {
                    return Err(format!(
                        "{e}。提示：这是面板的 32 位「用户访问密钥」，不能直接用于 OpenFrp API 鉴权，请改用「网页登录」"
                    ));
                }
                Err(e) => return Err(e),
            }
        }
        "chmlfrp" => (chmlfrp::login_key(&token)?, token.clone()),
        "sakura" => (sakura::login_key(&token)?, token.clone()),
        "msl" => (msl::login_key(&token)?, token.clone()),
        _ => return unsupported(provider, "该登录方式"),
    };
    save_account(
        app,
        ProviderAccount {
            provider: provider.to_string(),
            token: stored,
            username: user.username.clone(),
            email: user.email.clone(),
            login_method: "key".into(),
            saved_at: now_ms(),
        },
    )?;
    Ok(user)
}

fn login_password(
    app: &tauri::AppHandle,
    provider: &str,
    username: String,
    password: String,
    captcha: String,
) -> Result<ProviderUser, String> {
    if username.trim().is_empty() || password.is_empty() {
        return Err("用户名与密码不能为空".to_string());
    }
    let (user, token) = match provider {
        "mefrp" => mefrp::login_password(username.trim(), &password, &captcha)?,
        _ => return unsupported(provider, "密码登录"),
    };
    save_account(
        app,
        ProviderAccount {
            provider: provider.to_string(),
            token,
            username: user.username.clone(),
            email: user.email.clone(),
            login_method: "password".into(),
            saved_at: now_ms(),
        },
    )?;
    Ok(user)
}

/// 网页授权登录（OpenFrp OAuth code 换 Authorization）
fn login_code(app: &tauri::AppHandle, provider: &str, code: String) -> Result<ProviderUser, String> {
    let code = code.trim();
    if code.is_empty() {
        return Err("请填写授权页面返回的 code".to_string());
    }
    let (user, token) = match provider {
        "openfrp" => openfrp::login_code(code)?,
        _ => return unsupported(provider, "网页授权登录"),
    };
    save_account(
        app,
        ProviderAccount {
            provider: provider.to_string(),
            token,
            username: user.username.clone(),
            email: user.email.clone(),
            login_method: "code".into(),
            saved_at: now_ms(),
        },
    )?;
    Ok(user)
}

fn fetch_user(app: &tauri::AppHandle, provider: &str) -> Result<ProviderUser, String> {
    let token = token_of(app, provider)?;
    match provider {
        "mefrp" => mefrp::user(&token),
        "openfrp" => openfrp::user(&token),
        "chmlfrp" => chmlfrp::user(&token),
        "sakura" => sakura::user(&token),
        "msl" => msl::user(&token),
        _ => unsupported(provider, "该操作"),
    }
}

fn fetch_tunnels(app: &tauri::AppHandle, provider: &str) -> Result<Vec<ProviderTunnel>, String> {
    let token = token_of(app, provider)?;
    match provider {
        "mefrp" => mefrp::tunnels(&token),
        "openfrp" => openfrp::tunnels(&token),
        "chmlfrp" => chmlfrp::tunnels(&token),
        "sakura" => sakura::tunnels(&token),
        "msl" => msl::tunnels(&token),
        _ => unsupported(provider, "该操作"),
    }
}

fn build_profile(
    app: &tauri::AppHandle,
    provider: &str,
    ids: &[String],
    name: Option<&str>,
) -> Result<Profile, String> {
    if ids.is_empty() {
        return Err("请至少选择一条隧道".to_string());
    }
    let token = token_of(app, provider)?;
    let profile = match provider {
        "mefrp" => mefrp::profile(&token, ids, name, provider)?,
        "openfrp" => openfrp::profile(&token, ids, name, provider)?,
        "chmlfrp" => chmlfrp::profile(&token, ids, name, provider)?,
        "sakura" => sakura::profile(&token, ids, name, provider)?,
        "msl" => msl::profile(&token, ids, name, provider)?,
        _ => return unsupported(provider, "该操作"),
    };
    Ok(bind_ids(profile, ids))
}

// ============ Tauri 命令 ============

#[tauri::command]
pub fn provider_list() -> Vec<ProviderMeta> {
    provider_metas()
}

#[tauri::command]
pub fn provider_accounts(app: tauri::AppHandle) -> Vec<AccountView> {
    load_store(&app)
        .accounts
        .into_iter()
        .map(|a| AccountView {
            token_tail: tail(&a.token),
            provider: a.provider,
            username: a.username,
            email: a.email,
            login_method: a.login_method,
            saved_at: a.saved_at,
        })
        .collect()
}

#[tauri::command]
pub async fn provider_login_key(
    app: tauri::AppHandle,
    provider: String,
    key: String,
) -> Result<ProviderUser, String> {
    tauri::async_runtime::spawn_blocking(move || login_key(&app, &provider, key))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_login_password(
    app: tauri::AppHandle,
    provider: String,
    username: String,
    password: String,
    captcha: String,
) -> Result<ProviderUser, String> {
    tauri::async_runtime::spawn_blocking(move || {
        login_password(&app, &provider, username, password, captcha)
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub fn provider_logout(app: tauri::AppHandle, provider: String) -> Result<(), String> {
    let mut store = load_store(&app);
    store.accounts.retain(|a| a.provider != provider);
    save_store(&app, &store)
}

/// 网页授权登录（目前仅 OpenFrp 支持）
#[tauri::command]
pub async fn provider_login_code(
    app: tauri::AppHandle,
    provider: String,
    code: String,
) -> Result<ProviderUser, String> {
    tauri::async_runtime::spawn_blocking(move || login_code(&app, &provider, code))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

/// 需要用户在浏览器中打开的授权页面
#[tauri::command]
pub fn provider_auth_url(provider: String) -> Result<String, String> {
    match provider.as_str() {
        "openfrp" => Ok(openfrp::auth_url()),
        _ => Err("该服务商无需网页授权".to_string()),
    }
}

// ============ OpenFrp 网页登录（Remote Login） ============

/// 发起网页登录：返回需要用户在浏览器中打开的授权地址
#[tauri::command]
pub async fn provider_web_login_start(provider: String) -> Result<openfrp::WebLoginStart, String> {
    if provider != "openfrp" {
        return Err("该服务商暂不支持网页登录".to_string());
    }
    tauri::async_runtime::spawn_blocking(openfrp::web_login_start)
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

/// 轮询网页登录结果：None = 尚未授权，Some = 已登录并保存账号
#[tauri::command]
pub async fn provider_web_login_poll(
    app: tauri::AppHandle,
    provider: String,
    request_uuid: String,
) -> Result<Option<ProviderUser>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if provider != "openfrp" {
            return Err("该服务商暂不支持网页登录".to_string());
        }
        match openfrp::web_login_poll(&request_uuid)? {
            openfrp::WebLoginPoll::Pending => Ok(None),
            openfrp::WebLoginPoll::Authorized(token) => {
                let user = openfrp::login_key(&token)?;
                save_account(
                    &app,
                    ProviderAccount {
                        provider: provider.clone(),
                        token: token.clone(),
                        username: user.username.clone(),
                        email: user.email.clone(),
                        login_method: "web".into(),
                        saved_at: now_ms(),
                    },
                )?;
                Ok(Some(user))
            }
        }
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

/// 取消网页登录，清理一次性密钥
#[tauri::command]
pub fn provider_web_login_cancel(request_uuid: String) {
    openfrp::web_login_cancel(&request_uuid);
}

/// 每日签到（目前仅 ME Frp 支持，且需要人机验证）
#[tauri::command]
pub async fn provider_sign(
    app: tauri::AppHandle,
    provider: String,
    captcha: String,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let token = token_of(&app, &provider)?;
        match provider.as_str() {
            "mefrp" => mefrp::sign(&token, &captcha),
            _ => unsupported(&provider, "签到"),
        }
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_user(app: tauri::AppHandle, provider: String) -> Result<ProviderUser, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_user(&app, &provider))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_tunnels(
    app: tauri::AppHandle,
    provider: String,
) -> Result<Vec<ProviderTunnel>, String> {
    tauri::async_runtime::spawn_blocking(move || fetch_tunnels(&app, &provider))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

/// 预览配置（统一渲染为标准 frpc.toml）
#[tauri::command]
pub async fn provider_config(
    app: tauri::AppHandle,
    provider: String,
    tunnel_ids: Vec<String>,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let profile = build_profile(&app, &provider, &tunnel_ids, None)?;
        Ok::<String, String>(render_config(&profile, &HashMap::new(), 0))
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_import(
    app: tauri::AppHandle,
    provider: String,
    tunnel_ids: Vec<String>,
    name: Option<String>,
) -> Result<Profile, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let profile = build_profile(&app, &provider, &tunnel_ids, name.as_deref())?;
        let mut store = read_store(&app);
        store.profiles.push(profile.clone());
        store.active_id = profile.id.clone();
        write_store(&app, &store)?;
        Ok::<Profile, String>(profile)
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_kick(
    app: tauri::AppHandle,
    provider: String,
    tunnel_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let token = token_of(&app, &provider)?;
        match provider.as_str() {
            "mefrp" => mefrp::kick(&token, &tunnel_id),
            _ => unsupported(&provider, "强制下线"),
        }
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[tauri::command]
pub async fn provider_toggle(
    app: tauri::AppHandle,
    provider: String,
    tunnel_id: String,
    disabled: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let token = token_of(&app, &provider)?;
        match provider.as_str() {
            "mefrp" => mefrp::toggle(&token, &tunnel_id, disabled),
            _ => unsupported(&provider, "启用/禁用隧道"),
        }
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaInfo {
    pub url: String,
    pub client: String,
}

#[tauri::command]
pub fn provider_captcha_info(provider: String) -> Result<CaptchaInfo, String> {
    match provider.as_str() {
        "mefrp" => Ok(mefrp::captcha_info()),
        _ => Err("该服务商无需人机验证".to_string()),
    }
}

#[tauri::command]
pub fn provider_decode_captcha(raw: String) -> Result<String, String> {
    mefrp::decode_captcha(&raw)
}

/// 解析任意 frpc 配置文本（TOML / 旧版 INI），供导入面板使用
#[tauri::command]
pub fn provider_parse_config(text: String) -> Result<ParseResult, String> {
    parse_config_text(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traffic_and_date_formatting() {
        // ME Frp：traffic 单位为 MB（与面板一致：>=1024 才显示 GB）
        assert_eq!(mefrp_traffic_text(1_969_711), "1923.55 GB");
        assert_eq!(mefrp_traffic_text(512), "512.00 MB");
        assert_eq!(mefrp_traffic_text(1024), "1.00 GB");
        assert_eq!(mefrp_traffic_text(0), "暂无");
        // ME Frp：带宽单位为 1/128 Mbps（8320 → 65 Mbps）
        assert_eq!(mefrp_mbps_text(8320), "65 Mbps");
        assert_eq!(mefrp_mbps_text(1280), "10 Mbps");
        assert_eq!(mefrp_mbps_text(64), "0.5 Mbps");
        assert_eq!(mefrp_mbps_text(0), "-");
        // SakuraFrp：traffic 单位为字节
        assert_eq!(bytes_text(15_998_753_178), "14.90 GB");
        assert_eq!(bytes_text(1024), "1.00 KB");
        assert_eq!(bytes_text(0), "0 B");
        // Kbps → Mbps（OpenFrp / SakuraFrp 侧仍使用）
        assert_eq!(kbps_text(8320), "8.1 Mbps");
        assert_eq!(kbps_text(0), "");
        // VIP 到期时间戳
        let vip = fmt_date(1_799_751_103);
        println!("vip date: {vip}");
        assert!(vip.starts_with("2027-"), "unexpected: {vip}");
        assert_eq!(fmt_date(0), "-");
    }
}

#[cfg(test)]
mod live_tests {
    use super::*;

    /// 使用本机已登录的真实账号做一次只读核对（未设置环境变量时跳过）
    #[test]
    fn live_account_snapshot() {
        if let Ok(token) = std::env::var("MEFRP_TOKEN") {
            if let Ok(u) = mefrp::user(&token) {
                println!("[mefrp] user={} group={} traffic={} used/limit={}/{} signed={:?}",
                    u.username, u.group, u.traffic, u.tunnel_used, u.tunnel_limit, u.signed);
                for kv in &u.extras {
                    println!("[mefrp] extra {}={}", kv.label, kv.value);
                }
                if let Ok(list) = mefrp::tunnels(&token) {
                    println!("[mefrp] tunnels={}", list.len());
                    for t in list.iter().take(2) {
                        println!("[mefrp] {} type={} remote={} speed={} online={}", t.name, t.proxy_type, t.remote_desc, t.speed, t.online);
                    }
                }
            }
        }
        if let Ok(token) = std::env::var("SAKURA_TOKEN") {
            if let Ok(u) = sakura::user(&token) {
                println!("[sakura] user={} group={} traffic={} limit={} signed={:?}",
                    u.username, u.group, u.traffic, u.tunnel_limit, u.signed);
            }
            if let Ok(list) = sakura::tunnels(&token) {
                println!("[sakura] tunnels={}", list.len());
                for t in list.iter().take(2) {
                    println!("[sakura] {} type={} remote={} speed={} online={}", t.name, t.proxy_type, t.remote_desc, t.speed, t.online);
                }
            }
        }
    }
}


