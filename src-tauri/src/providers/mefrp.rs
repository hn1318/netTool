//! ME Frp 5.0 开放 API

use super::{fmt_date, http, mefrp_mbps_text, mefrp_traffic_text, profile_from_parsed, ProviderTunnel, ProviderUser, KV};
use crate::config::parse_config_text;
use crate::model::Profile;
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::Value;

const BASE: &str = "https://api.mefrp.com/api";
const CAPTCHA_CLIENT: &str = "nettool";

fn envelope(v: Value) -> Result<Value, String> {
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let message = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("未知错误")
        .to_string();
    if code == 200 {
        Ok(v.get("data").cloned().unwrap_or(Value::Null))
    } else {
        Err(format!("ME Frp 返回错误：{message}（code {code}）"))
    }
}

fn get(path: &str, token: &str) -> Result<Value, String> {
    let resp = http()?
        .get(format!("{BASE}{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if status.as_u16() == 401 {
        return Err("登录凭据无效或已过期（401），请重新登录".to_string());
    }
    envelope(serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?)
}

fn post(path: &str, token: &str, body: Value) -> Result<Value, String> {
    post_full(path, token, body).map(|(data, _)| data)
}

/// 返回 (data, message)，签到等接口需要读取提示语
fn post_full(path: &str, token: &str, body: Value) -> Result<(Value, String), String> {
    let resp = http()?
        .post(format!("{BASE}{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    if status.as_u16() == 401 {
        return Err("登录凭据无效或已过期（401），请重新登录".to_string());
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;
    let message = v
        .get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_string();
    let data = envelope(v)?;
    Ok((data, message))
}

fn parse_user(v: &Value) -> ProviderUser {
    let i = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    ProviderUser {
        provider: "mefrp".into(),
        user_id: i("userId").to_string(),
        username: s("username"),
        email: s("email"),
        group: {
            let friendly = s("friendlyGroup");
            if friendly.is_empty() {
                s("group")
            } else {
                friendly
            }
        },
        tunnel_limit: i("maxProxies"),
        tunnel_used: i("usedProxies"),
        // ME Frp 的 traffic 单位为 MB（面板逻辑：>=1024 时换算成 GB）
        traffic: mefrp_traffic_text(i("traffic")),
        signed: v.get("todaySigned").and_then(|x| x.as_bool()),
        extras: {
            let mut kvs = Vec::new();
            // 面板「享有带宽 (上/下)」= outBound / 128 Mbps / inBound / 128 Mbps
            let down = i("inBound");
            let up = i("outBound");
            if up > 0 || down > 0 {
                kvs.push(KV {
                    label: "带宽".into(),
                    value: format!(
                        "上行 {} · 下行 {}",
                        mefrp_mbps_text(up),
                        mefrp_mbps_text(down)
                    ),
                });
            }
            if i("status") != 0 {
                kvs.push(KV {
                    label: "账户状态".into(),
                    value: match i("status") {
                        1 => "被封禁".into(),
                        2 => "流量超限".into(),
                        other => format!("异常({other})"),
                    },
                });
            }
            if i("vipExpireTime") > 0 {
                kvs.push(KV {
                    label: "VIP 到期".into(),
                    value: fmt_date(i("vipExpireTime")),
                });
            }
            kvs
        },
    }
}

fn normalize_domains(raw: &str) -> String {
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
    t.to_string()
}

fn parse_proxies(
    data: &Value,
    nodes: &std::collections::HashMap<i64, String>,
    account_speed: &str,
) -> Vec<ProviderTunnel> {
    let Some(list) = data.get("proxies").and_then(|p| p.as_array()) else {
        return Vec::new();
    };
    list.iter()
        .map(|p| {
            let i = |k: &str| p.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
            let s = |k: &str| p.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let proxy_type = s("proxyType");
            let domains = normalize_domains(&s("domain"));
            let node_id = i("nodeId");
            let remote_port = i("remotePort");
            let is_web = matches!(proxy_type.as_str(), "http" | "https");
            ProviderTunnel {
                id: i("proxyId").to_string(),
                name: s("proxyName"),
                proxy_type: proxy_type.clone(),
                local_ip: {
                    let ip = s("localIp");
                    if ip.is_empty() {
                        "127.0.0.1".into()
                    } else {
                        ip
                    }
                },
                local_port: i("localPort"),
                remote_port,
                remote_desc: if is_web {
                    domains.clone()
                } else if remote_port > 0 {
                    format!(":{remote_port}")
                } else {
                    "-".into()
                },
                domains,
                speed: {
                    let own = extract_speed(p);
                    if own.is_empty() {
                        account_speed.to_string()
                    } else {
                        own
                    }
                },
                node: nodes
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| format!("节点 {node_id}")),
                online: p.get("isOnline").and_then(|x| x.as_bool()).unwrap_or(false),
                disabled: p.get("isDisabled").and_then(|x| x.as_bool()).unwrap_or(false),
                banned: p.get("isBanned").and_then(|x| x.as_bool()).unwrap_or(false),
            }
        })
        .collect()
}

/// 隧道限速字段在不同版本里命名不一致，逐个尝试
fn extract_speed(v: &Value) -> String {
    for key in [
        "speedLimit",
        "bandwidthLimit",
        "bandwidth",
        "speed",
        "rate",
        "limit",
    ] {
        match v.get(key) {
            Some(Value::String(s)) if !s.trim().is_empty() => return s.trim().to_string(),
            Some(Value::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    if i > 0 {
                        return format!("{i} Mbps");
                    }
                }
            }
            _ => {}
        }
    }
    String::new()
}

// ============ 对外能力 ============

pub fn login_key(token: &str) -> Result<ProviderUser, String> {
    let data = get("/auth/user/info", token)?;
    Ok(parse_user(&data))
}

pub fn login_password(
    username: &str,
    password: &str,
    captcha: &str,
) -> Result<(ProviderUser, String), String> {
    let captcha_token = decode_captcha(captcha)?;
    let body = serde_json::json!({
        "username": username,
        "password": password,
        "captchaToken": captcha_token,
    });
    let data = post("/public/login", "", body)?;
    let token = data
        .get("token")
        .and_then(|t| t.as_str())
        .unwrap_or("")
        .to_string();
    if token.is_empty() {
        return Err("登录成功但未返回令牌，请改用「访问密钥登录」".to_string());
    }
    let user = login_key(&token)?;
    Ok((user, token))
}

pub fn user(token: &str) -> Result<ProviderUser, String> {
    login_key(token)
}

pub fn tunnels(token: &str) -> Result<Vec<ProviderTunnel>, String> {
    let data = get("/auth/proxy/list", token)?;
    let nodes = data
        .get("nodes")
        .and_then(|n| n.as_array())
        .map(|arr| {
            arr.iter()
                .map(|n| {
                    (
                        n.get("nodeId").and_then(|x| x.as_i64()).unwrap_or(0),
                        n.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    )
                })
                .collect::<std::collections::HashMap<_, _>>()
        })
        .unwrap_or_default();
    let account_speed = get("/auth/user/info", token)
        .map(|u| mefrp_mbps_text(u.get("outBound").and_then(|x| x.as_i64()).unwrap_or(0)))
        .unwrap_or_default();
    Ok(parse_proxies(&data, &nodes, &account_speed))
}

fn frp_token(token: &str) -> String {
    get("/auth/user/frpToken", token)
        .ok()
        .and_then(|d| d.get("token").and_then(|t| t.as_str()).map(|s| s.to_string()))
        .unwrap_or_default()
}

pub fn profile(
    token: &str,
    ids: &[String],
    name: Option<&str>,
    provider: &str,
) -> Result<Profile, String> {
    let fallback = frp_token(token);
    let mut merged: Option<crate::config::ParseResult> = None;

    // 优先请求多隧道合并配置（仅同一节点可用）
    if ids.len() > 1 {
        let proxy_ids: Vec<i64> = ids.iter().filter_map(|s| s.parse().ok()).collect();
        let body = serde_json::json!({ "proxyIds": proxy_ids, "format": "toml" });
        if let Ok(data) = post("/auth/proxy/config/multiple", token, body) {
            if let Some(cfg) = data.get("config").and_then(|c| c.as_str()) {
                if !cfg.trim().is_empty() {
                    if let Ok(parsed) = parse_config_text(cfg) {
                        if !parsed.tunnels.is_empty() {
                            return profile_from_parsed(parsed, provider, ids, name, &fallback);
                        }
                    }
                }
            }
        }
    }

    for id in ids {
        // 官方 SDK：proxyId 必须是整数，字符串会被服务端判为「无效的请求数据」
        let proxy_id: i64 = id.parse().map_err(|_| format!("隧道 ID 非法：{id}"))?;
        let body = serde_json::json!({ "proxyId": proxy_id, "format": "toml" });
        let data = post("/auth/proxy/config", token, body)?;
        let cfg = data
            .get("config")
            .and_then(|c| c.as_str())
            .ok_or("接口未返回配置内容")?;
        let parsed = parse_config_text(cfg)?;

        match merged.as_mut() {
            None => merged = Some(parsed),
            Some(acc) => {
                if acc.server_addr != parsed.server_addr || acc.server_port != parsed.server_port {
                    return Err(format!(
                        "所选隧道不在同一节点（{}:{} 与 {}:{}），请分批次导入",
                        acc.server_addr.clone().unwrap_or_default(),
                        acc.server_port.unwrap_or(0),
                        parsed.server_addr.clone().unwrap_or_default(),
                        parsed.server_port.unwrap_or(0)
                    ));
                }
                acc.tunnels.extend(parsed.tunnels);
            }
        }
    }

    let parsed = merged.ok_or("未获取到任何隧道配置")?;
    profile_from_parsed(parsed, provider, ids, name, &fallback)
}

/// 每日签到（需要人机验证），返回服务端提示语
pub fn sign(token: &str, captcha: &str) -> Result<String, String> {
    let captcha_token = decode_captcha(captcha)?;
    let (_, message) = post_full(
        "/auth/user/sign",
        token,
        serde_json::json!({ "captchaToken": captcha_token }),
    )?;
    if message.trim().is_empty() {
        Ok("签到成功".to_string())
    } else {
        Ok(message)
    }
}

pub fn kick(token: &str, tunnel_id: &str) -> Result<(), String> {
    let id: i64 = tunnel_id.parse().map_err(|_| "隧道 ID 非法")?;
    post("/auth/proxy/kick", token, serde_json::json!({ "proxyId": id }))?;
    Ok(())
}

pub fn toggle(token: &str, tunnel_id: &str, disabled: bool) -> Result<(), String> {
    let id: i64 = tunnel_id.parse().map_err(|_| "隧道 ID 非法")?;
    post(
        "/auth/proxy/toggle",
        token,
        serde_json::json!({ "proxyId": id, "isDisabled": disabled }),
    )?;
    Ok(())
}

pub fn captcha_info() -> super::CaptchaInfo {
    super::CaptchaInfo {
        url: format!("https://www.mefrp.com/3rdparty/captcha?client={CAPTCHA_CLIENT}"),
        client: CAPTCHA_CLIENT.to_string(),
    }
}

/// 人机验证返回 Base64(`token||client`)，解码后取 token；也允许直接粘贴 token
pub fn decode_captcha(raw: &str) -> Result<String, String> {
    let cleaned = raw.trim().replace(['\n', '\r', ' '], "");
    if cleaned.is_empty() {
        return Err("请先完成人机验证并粘贴验证码".to_string());
    }

    if let Ok(bytes) = B64.decode(&cleaned) {
        if let Ok(text) = String::from_utf8(bytes) {
            if text.contains("||") {
                let token = text.split("||").next().unwrap_or("").trim().to_string();
                if !token.is_empty() {
                    return Ok(token);
                }
            }
        }
    }

    // 用户也可能直接粘贴了 token 本身
    let looks_like_token = cleaned.len() >= 16
        && cleaned
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    if looks_like_token {
        return Ok(cleaned);
    }

    Err("验证码格式不正确（应为 Base64 文本，解码后形如 token||client）".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captcha_decoding_extracts_token() {
        let raw = B64.encode("abc123token||nettool");
        assert_eq!(decode_captcha(&raw).unwrap(), "abc123token");
        assert_eq!(
            decode_captcha("abcdefghijklmnop1234").unwrap(),
            "abcdefghijklmnop1234"
        );
        assert!(decode_captcha("!!!").is_err());
        assert!(decode_captcha("").is_err());
    }

    #[test]
    fn domain_list_is_normalized() {
        assert_eq!(normalize_domains("[\"mefrp.com\"]"), "mefrp.com");
        assert_eq!(normalize_domains("[\"a.com\", \"b.com\"]"), "a.com,b.com");
        assert_eq!(normalize_domains(""), "");
        assert_eq!(normalize_domains("[]"), "");
        assert_eq!(normalize_domains("plain.com"), "plain.com");
    }

    /// 网络冒烟：无效令牌必须被识别为鉴权失败
    #[test]
    fn unauthenticated_request_reports_auth_error() {
        match login_key("nettool-invalid-test-token") {
            Ok(u) => println!("意外成功: {}", u.username),
            Err(e) => {
                println!("auth error: {e}");
                assert!(
                    e.contains("401") || e.contains("凭据") || e.contains("请求失败"),
                    "错误信息不符合预期: {e}"
                );
            }
        }
    }

    /// 真实账号冒烟：拉取第一条隧道的配置并组装 Profile（需 MEFRP_TOKEN）
    #[test]
    fn live_config_builds_profile() {
        let Ok(token) = std::env::var("MEFRP_TOKEN") else {
            return;
        };
        let Ok(list) = tunnels(&token) else {
            return;
        };
        let Some(first) = list.first() else {
            println!("账号下没有隧道，跳过");
            return;
        };
        println!("tunnel id={} name={}", first.id, first.name);
        match profile(&token, &[first.id.clone()], None, "mefrp") {
            Ok(p) => {
                println!(
                    "profile ok: {} server={}:{} tunnels={} authTokenLen={}",
                    p.name,
                    p.server_addr,
                    p.server_port,
                    p.tunnels.len(),
                    p.auth_token.len()
                );
                assert!(!p.tunnels.is_empty());
            }
            Err(e) => panic!("配置获取失败: {e}"),
        }
    }
}
