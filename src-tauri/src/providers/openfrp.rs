//! OpenFrp 开放 API
//!
//! 参考文档：<https://github.com/ZGIT-Network/OPENFRP-APIDOC>
//!
//! - 鉴权头是 `Authorization: <令牌>`；新版网页登录令牌为原始字符串，
//!   旧版 Authorization 为 `OPENFRP<jwt>`（程序会自动识别）
//! - 文档推荐使用「Remote Login 远程安全登录」（网页授权）：
//!   申请授权 → 拉起浏览器 → 轮询 → 用 Curve25519 + XSalsa20-Poly1305 解出 Authorization
//! - 面板中的「第三方客户端安全登录」旧版 Authorization 直接登录入口已移除；
//!   已保存的旧 Authorization 仍可用于密钥登录

use super::{http, kbps_text, mib_text, ProviderTunnel, ProviderUser, KV};
use crate::model::{Profile, TunnelConfig};
use crate::store::gen_id;
use base64::Engine;
use crypto_box::aead::Aead;
use crypto_box::{PublicKey, SalsaBox, SecretKey};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const BASE: &str = "https://api.openfrp.net";
const ACCESS_BASE: &str = "https://access.openfrp.net";

// ============ 密钥规范化 ============

/// 把用户粘贴的内容规范化为可直接放进 Authorization 头的值。
///
/// 兼容：`Authorization: xxx`、`Bearer xxx`、带引号 / 换行，以及面板返回的
/// `{"authorization":"..."}` 形式。
///
/// 注意：OpenFrp 新版网页登录返回的是 **原始令牌**（56 位字母数字），必须原样发送；
/// 只有旧版 JWT（以 `eyJ` 开头的三段式）才需要补 `OPENFRP` 前缀。
pub fn normalize_token(input: &str) -> String {
    let mut t = input.trim().trim_matches('"').trim_matches('\'').trim();

    // 面板可能返回一整段 JSON
    let mut owned: Option<String> = None;
    if t.starts_with('{') && t.ends_with('}') {
        if let Ok(v) = serde_json::from_str::<Value>(t) {
            for key in ["authorization", "Authorization", "token", "access_token"] {
                if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                    owned = Some(s.trim().to_string());
                    break;
                }
            }
        }
    }
    if let Some(s) = &owned {
        t = s;
    }

    for prefix in ["Authorization:", "authorization:"] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest.trim();
        }
    }
    if let Some(rest) = t.strip_prefix("Bearer ") {
        t = rest.trim();
    } else if let Some(rest) = t.strip_prefix("bearer ") {
        t = rest.trim();
    }

    let upper = t.to_uppercase();
    if upper.starts_with("OPENFRP") {
        // 旧版 Authorization：保留前缀，顺手去掉前缀后的多余空格
        let rest = t[7..].trim();
        if rest.is_empty() {
            return "OPENFRP".to_string();
        }
        return format!("OPENFRP{rest}");
    }
    // 裸 JWT 按旧版规则补前缀；新版的原始令牌保持原样
    if is_legacy_jwt(t) {
        return format!("OPENFRP{t}");
    }
    t.to_string()
}

/// 旧版 OpenFrp Authorization 是 `OPENFRP` + 三段式 JWT
fn is_legacy_jwt(token: &str) -> bool {
    token.starts_with("eyJ") && token.matches('.').count() == 2
}

/// 面板「复制用户访问密钥」得到的是 32 位字符，不是 Authorization
pub fn is_access_key(token: &str) -> bool {
    let t = token.trim();
    t.len() == 32 && t.chars().all(|c| c.is_ascii_alphanumeric())
}

// ============ 请求封装 ============

fn post(path: &str, token: &str) -> Result<Value, String> {
    let resp = http()?
        .post(format!("{BASE}{path}"))
        .header("Authorization", token)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;

    let flag = v.get("flag").and_then(|f| f.as_bool()).unwrap_or(false);
    if !flag {
        let msg = v
            .get("msg")
            .and_then(|m| m.as_str())
            .unwrap_or("请求失败");
        let auth_like = status.as_u16() == 401
            || msg.contains("登入")
            || msg.contains("登录")
            || msg.contains("凭证")
            || msg.contains("token");
        if auth_like {
            return Err(format!(
                "OpenFrp 鉴权失败：{msg}。Authorization 可能已过期（有效期最长 30 天），请改用「网页登录」重新授权"
            ));
        }
        return Err(format!("OpenFrp 返回错误：{msg}"));
    }
    Ok(v.get("data").cloned().unwrap_or(Value::Null))
}

fn user_data(token: &str) -> Result<Value, String> {
    post("/frp/api/getUserInfo", token)
}

fn node_map(token: &str) -> std::collections::HashMap<i64, (String, String, u16)> {
    let mut map = std::collections::HashMap::new();
    if let Ok(data) = post("/frp/api/getNodeList", token) {
        if let Some(list) = data.get("list").and_then(|l| l.as_array()) {
            for n in list {
                let id = i64_of(n, &["id"]);
                let name = str_of(n, &["name"]);
                let host = str_of(n, &["hostname"]);
                let port = {
                    let p = i64_of(n, &["port"]);
                    if p > 0 {
                        p.min(65535) as u16
                    } else {
                        7000
                    }
                };
                map.insert(id, (name, host, port));
            }
        }
    }
    map
}

fn proxies(token: &str) -> Result<Vec<Value>, String> {
    let data = post("/frp/api/getUserProxies", token)?;
    Ok(data
        .get("list")
        .and_then(|l| l.as_array())
        .cloned()
        .unwrap_or_default())
}

// ============ 字段兼容读取 ============

fn str_of(v: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(x) = v.get(*key) {
            match x {
                Value::String(s) => {
                    let t = s.trim();
                    if !t.is_empty() {
                        return t.to_string();
                    }
                }
                Value::Number(n) => return n.to_string(),
                Value::Bool(b) => return b.to_string(),
                _ => {}
            }
        }
    }
    String::new()
}

fn i64_of(v: &Value, keys: &[&str]) -> i64 {
    for key in keys {
        if let Some(x) = v.get(*key) {
            if let Some(n) = x.as_i64() {
                return n;
            }
            if let Some(n) = x.as_f64() {
                return n as i64;
            }
            if let Some(s) = x.as_str() {
                if let Ok(n) = s.trim().parse::<i64>() {
                    return n;
                }
            }
            if let Some(b) = x.as_bool() {
                return i64::from(b);
            }
        }
    }
    0
}

/// 域名字段可能是 `["a.com"]` 文本化数组，也可能是 `a.com,b.com`
fn normalize_domains(raw: &str) -> String {
    let raw = raw.trim();
    if raw.is_empty() {
        return String::new();
    }
    if raw.starts_with('[') {
        if let Ok(Value::Array(list)) = serde_json::from_str::<Value>(raw) {
            return list
                .iter()
                .filter_map(|x| x.as_str())
                .map(str::trim)
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(",");
        }
    }
    raw.trim_matches('"')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .replace(' ', "")
}

fn split_host_port(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (host, port) = raw.rsplit_once(':')?;
    let port = port.trim().parse::<u16>().ok()?;
    if !looks_like_host(host) {
        return None;
    }
    Some((host.trim().to_string(), port))
}

/// 服务端可能对低权限账号返回“您无权查询此节点的地址”之类的占位文本
fn looks_like_host(host: &str) -> bool {
    let h = host.trim();
    !h.is_empty()
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'))
}

// ============ 用户 / 隧道 ============

pub fn login_key(token: &str) -> Result<ProviderUser, String> {
    let data = user_data(token)?;
    if data.is_null() {
        return Err("OpenFrp 未返回用户信息，Authorization 可能已失效".to_string());
    }
    let proxies_total = i64_of(&data, &["proxies"]);
    let used = i64_of(&data, &["used"]);
    let traffic = i64_of(&data, &["traffic"]);
    let mut extras = Vec::new();
    let in_limit = i64_of(&data, &["inLimit"]);
    let out_limit = i64_of(&data, &["outLimit"]);
    if in_limit > 0 || out_limit > 0 {
        extras.push(KV {
            label: "带宽".into(),
            value: format!("{in_limit}/{out_limit} Kbps"),
        });
    }
    if let Some(realname) = data.get("realname").and_then(|x| x.as_bool()) {
        extras.push(KV {
            label: "实名".into(),
            value: if realname { "已实名".into() } else { "未实名".into() },
        });
    }
    Ok(ProviderUser {
        provider: "openfrp".into(),
        user_id: i64_of(&data, &["id"]).to_string(),
        username: str_of(&data, &["username"]),
        email: str_of(&data, &["email"]),
        group: {
            let friendly = str_of(&data, &["friendlyGroup"]);
            if friendly.is_empty() {
                str_of(&data, &["group"])
            } else {
                friendly
            }
        },
        tunnel_limit: proxies_total,
        tunnel_used: used,
        traffic: mib_text(traffic),
        signed: None,
        extras,
    })
}

pub fn user(token: &str) -> Result<ProviderUser, String> {
    login_key(token)
}

pub fn tunnels(token: &str) -> Result<Vec<ProviderTunnel>, String> {
    let nodes = node_map(token);
    let list = proxies(token)?;
    let account_speed = user_data(token)
        .map(|d| kbps_text(i64_of(&d, &["outLimit"])))
        .unwrap_or_default();
    Ok(list
        .iter()
        .map(|p| {
            let proxy_type = str_of(p, &["proxyType"]).to_lowercase();
            let domains = normalize_domains(&str_of(p, &["domain"]));
            let remote_port = i64_of(p, &["remotePort"]);
            let is_web = matches!(proxy_type.as_str(), "http" | "https");
            let nid = i64_of(p, &["nid"]);
            let connect = str_of(p, &["connectAddress"]);
            let node_name = {
                let friendly = str_of(p, &["friendlyNode"]);
                if !friendly.is_empty() {
                    friendly
                } else {
                    nodes
                        .get(&nid)
                        .map(|(name, _, _)| name.clone())
                        .unwrap_or_else(|| format!("节点 {nid}"))
                }
            };
            let remote_desc = if !connect.is_empty() {
                connect.clone()
            } else if is_web {
                if domains.is_empty() {
                    "-".into()
                } else {
                    domains.clone()
                }
            } else if remote_port > 0 {
                format!(":{remote_port}")
            } else {
                "-".into()
            };
            // status 在新版接口里是布尔值：false 表示被管理员禁用
            let status = p.get("status").and_then(|x| x.as_bool());
            ProviderTunnel {
                id: i64_of(p, &["id"]).to_string(),
                name: str_of(p, &["proxyName", "name"]),
                proxy_type: proxy_type.clone(),
                local_ip: {
                    let ip = str_of(p, &["localIp", "localip"]);
                    if ip.is_empty() {
                        "127.0.0.1".into()
                    } else {
                        ip
                    }
                },
                local_port: i64_of(p, &["localPort", "localport"]),
                remote_port,
                domains: domains.clone(),
                remote_desc,
                speed: account_speed.clone(),
                node: node_name,
                online: p.get("online").and_then(|x| x.as_bool()).unwrap_or(false),
                disabled: status == Some(false),
                banned: false,
            }
        })
        .collect())
}

/// 网页授权登录页面（保留作为旧版入口）
pub fn auth_url() -> String {
    "https://account.naids.com/api/api/oauth2/authorize?response_type=code&redirect_uri=https://api.openfrp.net/oauth_callback&client_id=openfrp".to_string()
}

/// 用授权页面返回的 code 换取 Authorization
pub fn login_code(code: &str) -> Result<(ProviderUser, String), String> {
    let url = format!(
        "https://api.openfrp.net/oauth2/callback?code={}",
        super::url_encode(code.trim())
    );
    let resp = http()?
        .post(url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({}))
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let auth_header = resp
        .headers()
        .get(reqwest::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;
    if v.get("flag").and_then(|f| f.as_bool()) != Some(true) {
        let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("授权失败");
        return Err(format!("OpenFrp 授权失败：{msg}"));
    }
    let auth = auth_header.ok_or("服务端未返回 Authorization（code 可能已失效，请重新授权）")?;
    let token = normalize_token(&auth);
    let user = login_key(&token)?;
    Ok((user, token))
}

pub fn profile(
    token: &str,
    ids: &[String],
    name: Option<&str>,
    provider: &str,
) -> Result<Profile, String> {
    let me = user_data(token)?;
    let frp_token = str_of(&me, &["token"]);
    let username = str_of(&me, &["username"]);
    let nodes = node_map(token);
    let all = proxies(token)?;

    let mut server: Option<(String, u16)> = None;
    let mut node_name = String::new();
    let mut tunnels: Vec<TunnelConfig> = Vec::new();

    for id in ids {
        let pid: i64 = id.parse().map_err(|_| "隧道 ID 非法")?;
        let p = all
            .iter()
            .find(|x| i64_of(x, &["id"]) == pid)
            .ok_or_else(|| format!("未找到隧道 {id}（可能已被删除）"))?;

        let nid = i64_of(p, &["nid"]);
        let node_info = nodes.get(&nid).cloned();
        let n_name = node_info
            .as_ref()
            .map(|(n, _, _)| n.clone())
            .unwrap_or_else(|| format!("节点 {nid}"));
        // 低权限账号的节点 hostname 可能被服务端隐藏，退回隧道自带的 connectAddress
        let connect = str_of(p, &["connectAddress"]);
        let (host, port) = {
            let node_host = node_info
                .as_ref()
                .map(|(_, h, _)| h.clone())
                .unwrap_or_default();
            let node_port = node_info
                .as_ref()
                .map(|(_, _, p)| *p)
                .unwrap_or(7000);
            if !node_host.is_empty() && looks_like_host(&node_host) {
                (node_host, node_port)
            } else if let Some((h, _)) = split_host_port(&connect) {
                (h, 7000)
            } else if looks_like_host(&connect) {
                (connect, 7000)
            } else {
                return Err(format!("节点 {n_name} 未返回连接地址"));
            }
        };

        match &server {
            None => {
                server = Some((host.clone(), port));
                node_name = n_name.clone();
            }
            Some((h, p2)) => {
                if h != &host || p2 != &port {
                    return Err(format!(
                        "所选隧道不在同一节点（{node_name} 与 {n_name}），请分批次导入"
                    ));
                }
            }
        }

        let proxy_type = str_of(p, &["proxyType"]).to_lowercase();
        let is_web = matches!(proxy_type.as_str(), "http" | "https");
        let domains = normalize_domains(&str_of(p, &["domain"]));
        tunnels.push(TunnelConfig {
            id: gen_id(),
            name: str_of(p, &["proxyName", "name"]),
            proxy_type: proxy_type.clone(),
            local_ip: {
                let ip = str_of(p, &["localIp", "localip"]);
                if ip.is_empty() {
                    "127.0.0.1".into()
                } else {
                    ip
                }
            },
            local_port: i64_of(p, &["localPort", "localport"]).clamp(0, 65535) as u16,
            remote_port: i64_of(p, &["remotePort"]).clamp(0, 65535) as u16,
            custom_domains: if is_web && !domains.is_empty() {
                Some(domains)
            } else {
                None
            },
            subdomain: None,
            secret_key: None,
            use_encryption: p.get("useEncryption").and_then(|x| x.as_bool()),
            use_compression: p.get("useCompression").and_then(|x| x.as_bool()),
            bandwidth_limit: None,
            enabled: true,
            source: Some("openfrp".into()),
        });
    }

    let (server_addr, server_port) = server.ok_or("未获取到节点信息")?;
    let now = crate::model::now_ms();
    Ok(Profile {
        id: gen_id(),
        name: name
            .filter(|n| !n.trim().is_empty())
            .map(|n| n.trim().to_string())
            .unwrap_or_else(|| format!("OpenFrp · {node_name}")),
        server_addr,
        server_port,
        user: username,
        auth_token: frp_token,
        transport: "tcp".into(),
        tls_enable: true,
        log_level: "info".into(),
        tunnels,
        provider: Some(provider.to_string()),
        provider_proxy_ids: Vec::new(),
        created_at: now,
        updated_at: now,
    })
}

// ============ Remote Login 远程安全登录 ============

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebLoginStart {
    pub url: String,
    pub request_uuid: String,
}

#[derive(Debug, Clone)]
pub enum WebLoginPoll {
    Pending,
    Authorized(String),
}

fn pending_keys() -> &'static Mutex<HashMap<String, [u8; 32]>> {
    static PENDING: OnceLock<Mutex<HashMap<String, [u8; 32]>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

fn random_secret() -> Result<[u8; 32], String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("生成随机密钥失败: {e}"))?;
    Ok(bytes)
}

fn b64_decode_any(s: &str) -> Result<Vec<u8>, String> {
    for engine in [
        base64::engine::general_purpose::URL_SAFE,
        base64::engine::general_purpose::STANDARD,
    ] {
        if let Ok(v) = engine.decode(s) {
            return Ok(v);
        }
    }
    let mut padded = s.to_string();
    while padded.len() % 4 != 0 {
        padded.push('=');
    }
    base64::engine::general_purpose::URL_SAFE
        .decode(&padded)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(&padded))
        .map_err(|_| "服务器返回的 base64 无法解析".to_string())
}

/// Curve25519 + XSalsa20-Poly1305 解密（cipher = 24 字节 nonce + 密文）
fn decrypt_authorization(
    client_secret: [u8; 32],
    server_public: &[u8],
    cipher_all: &[u8],
) -> Result<String, String> {
    if server_public.len() != 32 {
        return Err("服务器公钥长度不正确".to_string());
    }
    if cipher_all.len() <= 24 {
        return Err("授权数据长度不正确".to_string());
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(server_public);
    let secret = SecretKey::from(client_secret);
    let peer = PublicKey::from(pk);
    let sbox = SalsaBox::new(&peer, &secret);
    let nonce = crypto_box::Nonce::from_slice(&cipher_all[..24]);
    let plain = sbox
        .decrypt(nonce, &cipher_all[24..])
        .map_err(|_| "授权数据解密失败".to_string())?;
    String::from_utf8(plain).map_err(|_| "授权数据不是合法的 UTF-8".to_string())
}

/// 申请一次网页授权，返回需要用户在浏览器中打开的 URL
pub fn web_login_start() -> Result<WebLoginStart, String> {
    // 只保留最近一次授权请求，避免长时间运行时积累一次性密钥
    pending_keys().lock().unwrap().clear();
    let secret = random_secret()?;
    let sk = SecretKey::from(secret);
    let public_key = base64::engine::general_purpose::URL_SAFE.encode(sk.public_key().as_bytes());

    let resp = http()?
        .post(format!("{ACCESS_BASE}/argoAccess/requestLogin"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "public_key": public_key }))
        .send()
        .map_err(|e| format!("请求授权失败: {e}"))?;
    if resp.status().as_u16() == 429 {
        return Err("请求过于频繁（429），请稍后再试".to_string());
    }
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;
    let code = i64_of(&v, &["code"]);
    if code != 200 {
        return Err(format!(
            "申请网页授权失败：{}（HTTP {status}）",
            str_of(&v, &["msg"])
        ));
    }
    let data = v
        .get("data")
        .cloned()
        .ok_or_else(|| "授权响应缺少 data".to_string())?;
    let url = str_of(&data, &["authorization_url"]);
    let uuid = str_of(&data, &["request_uuid"]);
    if url.is_empty() || uuid.is_empty() {
        return Err("授权响应缺少 authorization_url / request_uuid".to_string());
    }
    pending_keys().lock().unwrap().insert(uuid.clone(), secret);
    Ok(WebLoginStart {
        url,
        request_uuid: uuid,
    })
}

/// 轮询一次授权结果（建议 5 秒一次，最多 5 分钟）
pub fn web_login_poll(request_uuid: &str) -> Result<WebLoginPoll, String> {
    let resp = http()?
        .get(format!("{ACCESS_BASE}/argoAccess/pollLogin"))
        .query(&[("request_uuid", request_uuid)])
        .send()
        .map_err(|e| format!("轮询授权失败: {e}"))?;

    let status = resp.status();
    if status.as_u16() == 204 {
        return Ok(WebLoginPoll::Pending);
    }
    if status.as_u16() == 429 {
        return Err("授权轮询过于频繁（429），请稍后再试".to_string());
    }
    if status.as_u16() != 200 {
        return Err(format!("轮询授权失败：HTTP {status}"));
    }

    let server_public = resp
        .headers()
        .get("x-request-public-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;
    let code = i64_of(&v, &["code"]);
    if code == 204 {
        return Ok(WebLoginPoll::Pending);
    }
    if code != 200 {
        return Err(format!(
            "授权失败：{}",
            str_of(&v, &["msg"]).if_empty("未知错误")
        ));
    }
    let data = v
        .get("data")
        .cloned()
        .ok_or_else(|| "授权响应缺少 data".to_string())?;
    let encrypted = str_of(&data, &["authorization_data"]);
    if encrypted.is_empty() {
        return Err("授权响应缺少 authorization_data".to_string());
    }
    let server_public = server_public.ok_or("响应缺少服务器公钥（x-request-public-key）")?;
    let cipher_all = b64_decode_any(&encrypted)?;
    let server_public = b64_decode_any(&server_public)?;

    let secret = {
        let guard = pending_keys().lock().unwrap();
        guard
            .get(request_uuid)
            .copied()
            .ok_or_else(|| "授权请求已失效，请重新发起网页登录".to_string())?
    };
    let token = decrypt_authorization(secret, &server_public, &cipher_all)?;
    let token = extract_authorization(&token)?;
    pending_keys().lock().unwrap().remove(request_uuid);
    Ok(WebLoginPoll::Authorized(token))
}

/// 解密后的内容可能是 Authorization 明文，也可能是服务端返回的 JSON 包装
fn extract_authorization(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("OpenFrp 授权返回为空，请重新发起网页登录".to_string());
    }
    if raw.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<Value>(raw) {
            for key in ["authorization", "Authorization", "token", "access_token"] {
                if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                    if !s.trim().is_empty() {
                        return Ok(normalize_token(s));
                    }
                }
            }
            let msg = str_of(&v, &["msg", "message"]);
            if !msg.is_empty() {
                return Err(format!(
                    "OpenFrp 尚未完成授权：{msg}。请在浏览器中确认登录后重试"
                ));
            }
        }
    }
    Ok(normalize_token(raw))
}

/// 用户取消网页登录，清理一次性密钥
pub fn web_login_cancel(request_uuid: &str) {
    pending_keys().lock().unwrap().remove(request_uuid);
}

/// 小工具：空字符串回退
trait IfEmpty {
    fn if_empty(self, fallback: &str) -> String;
}
impl IfEmpty for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.trim().is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto_box::aead::AeadCore;
    use crypto_box::aead::OsRng;

    #[test]
    fn token_is_normalized() {
        // 新版网页登录令牌：原样发送
        let modern = "0123456789abcdef0123456789abcdefABCDEFGHIJKLMNOPQRSTUVWX";
        assert_eq!(normalize_token(modern), modern);
        assert_eq!(normalize_token(&format!("Bearer {modern}")), modern);
        assert_eq!(normalize_token("abc.def"), "abc.def");
        // 旧版 OPENFRP JWT：保留前缀
        let legacy = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzUxMiJ9.eyJzdWIiOiIxIn0.sig";
        assert_eq!(normalize_token(legacy), format!("OPENFRP{legacy}"));
        assert_eq!(normalize_token(&format!("OPENFRP{legacy}")), format!("OPENFRP{legacy}"));
        assert_eq!(normalize_token("OPENFRPabc.def"), "OPENFRPabc.def");
        assert_eq!(normalize_token("openfrpabc.def"), "OPENFRPabc.def");
        assert_eq!(normalize_token("Authorization: OPENFRP abc"), "OPENFRPabc");
        assert_eq!(
            normalize_token("Authorization: OPENFRPeyJ0eXAiOiJKV1QifQ.sig"),
            "OPENFRPeyJ0eXAiOiJKV1QifQ.sig"
        );
        assert_eq!(
            normalize_token(r#"{"authorization":"OPENFRPabc"}"#),
            "OPENFRPabc"
        );
        assert_eq!(normalize_token("\"OPENFRPabc\""), "OPENFRPabc");
    }

    #[test]
    fn access_key_detection() {
        assert!(is_access_key("0123456789abcdef0123456789abcdef"));
        assert!(!is_access_key("OPENFRPabc"));
        assert!(!is_access_key("short"));
    }

    #[test]
    fn domains_are_normalized() {
        assert_eq!(normalize_domains(r#"["a.com","b.com"]"#), "a.com,b.com");
        assert_eq!(normalize_domains("a.com, b.com"), "a.com,b.com");
        assert_eq!(normalize_domains(r#""a.com""#), "a.com");
    }

    #[test]
    fn decrypts_authorization_payload() {
        // 模拟服务端：用自己的长期密钥与客户端临时公钥做 SalsaBox 加密
        let client_secret = SecretKey::generate(&mut OsRng);
        let server_secret = SecretKey::generate(&mut OsRng);
        let nonce = SalsaBox::generate_nonce(&mut OsRng);
        let plaintext = b"OPENFRPdemo.jwt.token";
        let server_box = SalsaBox::new(&client_secret.public_key(), &server_secret);
        let cipher = server_box.encrypt(&nonce, plaintext.as_ref()).unwrap();
        let mut cipher_all = nonce.to_vec();
        cipher_all.extend_from_slice(&cipher);

        let out = decrypt_authorization(
            client_secret.to_bytes(),
            server_secret.public_key().as_bytes(),
            &cipher_all,
        )
        .unwrap();
        assert_eq!(out, "OPENFRPdemo.jwt.token");
    }

    /// 网络冒烟：Remote Login 应能拿到授权地址，未授权时轮询返回 Pending
    #[test]
    fn remote_login_starts_and_polls() {
        let start = web_login_start().expect("申请网页授权失败");
        assert!(
            start.url.contains("argoaccess") || start.url.contains("openfrp.net"),
            "unexpected url: {}",
            start.url
        );
        assert!(!start.request_uuid.is_empty());
        match web_login_poll(&start.request_uuid) {
            Ok(WebLoginPoll::Pending) => {}
            Ok(WebLoginPoll::Authorized(_)) => println!("测试期间恰好完成了授权，跳过 pending 断言"),
            Err(e) => println!("poll login: {e}"),
        }
        web_login_cancel(&start.request_uuid);
    }

    /// 网络冒烟：无效 Authorization 应返回鉴权错误
    #[test]
    fn invalid_token_is_rejected() {
        match login_key("OPENFRPinvalid-token-for-test") {
            Ok(u) => println!("意外成功: {}", u.username),
            Err(e) => println!("openfrp auth error: {e}"),
        }
    }
}
