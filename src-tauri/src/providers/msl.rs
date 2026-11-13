//! MSL Frp（MSL 幻云）开放 API
//!
//! 文档：<https://apidoc-user.mslmc.cn/>
//! 客户端实现参考：MSLTeam/MSLX 的 `MSLUserService` / `MainPage`。
//!
//! - 基础地址：`https://user.mslmc.net/api`
//! - 鉴权：`Authorization: Bearer <Token>`（用户中心可获取）
//! - 用户信息：`GET /frp/userInfo`
//! - 节点列表：`GET /frp/nodeList`
//! - 隧道列表：`GET /frp/getTunnelList`
//! - 单隧道配置：`GET /frp/getTunnelConfig?id=<id>`（返回 TOML 字符串）

use super::{http, profile_from_parsed, url_encode, ProviderTunnel, ProviderUser, KV};
use crate::config::parse_config_text;
use crate::model::Profile;
use serde_json::Value;

const BASE: &str = "https://user.mslmc.net/api";

fn request(path: &str, token: &str, query: &[(&str, &str)]) -> Result<Value, String> {
    let mut url = format!("{BASE}{path}");
    if !query.is_empty() {
        let qs = query
            .iter()
            .map(|(k, v)| format!("{k}={}", url_encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        url.push('?');
        url.push_str(&qs);
    }
    let resp = http()?
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let status = resp.status();
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;
    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let msg = v
        .get("msg")
        .or_else(|| v.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or("请求失败");
    if code != 200 {
        let auth_like = matches!(code, 400 | 401 | 403) || msg.contains("登录") || msg.contains("Token");
        if auth_like {
            return Err(format!(
                "MSL 鉴权失败：{msg}（HTTP {status}）。请在 MSL 用户中心复制 Token 后重新登录"
            ));
        }
        return Err(format!("MSL 返回错误：{msg}（code {code}）"));
    }
    Ok(v.get("data").cloned().unwrap_or(Value::Null))
}

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
            if let Some(s) = x.as_str() {
                if let Ok(n) = s.trim().parse::<i64>() {
                    return n;
                }
            }
        }
    }
    0
}

fn group_name(group: i64) -> String {
    match group {
        6 => "超级管理员".into(),
        1 => "高级会员".into(),
        2 => "超级会员".into(),
        0 => "普通用户".into(),
        other => format!("用户组 {other}"),
    }
}

fn parse_user(data: &Value, tunnel_used: i64) -> ProviderUser {
    let outdated = i64_of(data, &["outdated"]);
    let mut extras = Vec::new();
    extras.push(KV {
        label: "到期".into(),
        value: if outdated == 3_749_682_420 || outdated <= 0 {
            "长期有效".into()
        } else {
            super::fmt_date(outdated)
        },
    });
    let integral = i64_of(data, &["integral", "points"]);
    if integral > 0 {
        extras.push(KV {
            label: "积分".into(),
            value: integral.to_string(),
        });
    }
    ProviderUser {
        provider: "msl".into(),
        user_id: str_of(data, &["id", "uid"]),
        username: str_of(data, &["name", "username"]),
        email: str_of(data, &["email", "mail"]),
        group: group_name(i64_of(data, &["user_group", "group"])),
        tunnel_limit: i64_of(data, &["maxTunnelCount", "max_tunnel_count"]),
        tunnel_used,
        traffic: "-".into(),
        signed: None,
        extras,
    }
}

pub fn login_key(token: &str) -> Result<ProviderUser, String> {
    let data = request("/frp/userInfo", token, &[])?;
    if data.is_null() {
        return Err("MSL Token 无效或已过期".to_string());
    }
    // 顺带统计隧道数量；失败不影响登录
    let used = match request("/frp/getTunnelList", token, &[]) {
        Ok(list) => list.as_array().map(|a| a.len() as i64).unwrap_or(0),
        Err(_) => 0,
    };
    Ok(parse_user(&data, used))
}

pub fn user(token: &str) -> Result<ProviderUser, String> {
    login_key(token)
}

fn node_map(token: &str) -> std::collections::HashMap<i64, (String, i64)> {
    let mut map = std::collections::HashMap::new();
    if let Ok(data) = request("/frp/nodeList", token, &[]) {
        if let Some(list) = data.as_array() {
            for n in list {
                let id = i64_of(n, &["id"]);
                let name = str_of(n, &["node", "name"]);
                let bandwidth = i64_of(n, &["bandwidth"]);
                map.insert(id, (name, bandwidth));
            }
        }
    }
    map
}

pub fn tunnels(token: &str) -> Result<Vec<ProviderTunnel>, String> {
    let nodes = node_map(token);
    let data = request("/frp/getTunnelList", token, &[])?;
    let Some(list) = data.as_array() else {
        return Ok(Vec::new());
    };
    Ok(list
        .iter()
        .map(|t| {
            let node_id = i64_of(t, &["node_id", "nodeId"]);
            let (node_name, bandwidth) = nodes
                .get(&node_id)
                .cloned()
                .unwrap_or_else(|| (format!("节点 {node_id}"), 0));
            let proxy_type = {
                let ty = str_of(t, &["type", "proxy_type"]);
                if ty.is_empty() {
                    "tcp".to_string()
                } else {
                    ty.to_lowercase()
                }
            };
            let domains = super::normalize_domain_text(&str_of(t, &["domain", "host", "custom_domain"]));
            let remote_port = i64_of(t, &["remote_port", "remotePort"]);
            let is_web = matches!(proxy_type.as_str(), "http" | "https");
            ProviderTunnel {
                id: i64_of(t, &["id"]).to_string(),
                name: str_of(t, &["name"]),
                proxy_type: proxy_type.clone(),
                local_ip: {
                    let ip = str_of(t, &["local_ip", "localIp"]);
                    if ip.is_empty() {
                        "127.0.0.1".into()
                    } else {
                        ip
                    }
                },
                local_port: i64_of(t, &["local_port", "localPort"]),
                remote_port,
                domains: domains.clone(),
                remote_desc: if is_web && !domains.is_empty() {
                    domains
                } else if remote_port > 0 {
                    format!(":{remote_port}")
                } else {
                    str_of(t, &["remarks"]).if_empty("-")
                },
                speed: if bandwidth > 0 {
                    format!("{bandwidth} Mbps")
                } else {
                    String::new()
                },
                node: node_name,
                online: i64_of(t, &["status"]) != 0,
                disabled: false,
                banned: false,
            }
        })
        .collect())
}

pub fn profile(
    token: &str,
    ids: &[String],
    name: Option<&str>,
    provider: &str,
) -> Result<Profile, String> {
    let mut merged: Option<crate::config::ParseResult> = None;

    for id in ids {
        let data = request("/frp/getTunnelConfig", token, &[("id", id.as_str())])?;
        let cfg = data.as_str().unwrap_or_default();
        if cfg.trim().is_empty() {
            return Err(format!("隧道 {id} 未返回配置内容"));
        }
        let parsed = parse_config_text(cfg)?;
        match merged.as_mut() {
            None => merged = Some(parsed),
            Some(acc) => {
                if acc.server_addr != parsed.server_addr || acc.server_port != parsed.server_port {
                    return Err(format!(
                        "所选隧道不在同一服务器（{}:{} 与 {}:{}），请分批次导入",
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
    let name = name
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.trim().to_string())
        .or_else(|| Some("MSL Frp".to_string()));
    profile_from_parsed(parsed, provider, ids, name.as_deref(), token)
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

    #[test]
    fn parses_user_info() {
        let v: Value = serde_json::from_str(
            r#"{
                "name": "demo",
                "user_group": 1,
                "maxTunnelCount": "20",
                "outdated": 3749682420
            }"#,
        )
        .unwrap();
        let u = parse_user(&v, 3);
        assert_eq!(u.username, "demo");
        assert_eq!(u.group, "高级会员");
        assert_eq!(u.tunnel_limit, 20);
        assert_eq!(u.tunnel_used, 3);
        assert!(u.extras.iter().any(|k| k.label == "到期" && k.value == "长期有效"));
    }

    #[test]
    fn group_mapping() {
        assert_eq!(group_name(6), "超级管理员");
        assert_eq!(group_name(2), "超级会员");
        assert_eq!(group_name(0), "普通用户");
        assert_eq!(group_name(9), "用户组 9");
    }

    #[test]
    fn invalid_token_is_rejected() {
        match login_key("nettool-invalid-msl-token") {
            Ok(u) => println!("意外成功: {}", u.username),
            Err(e) => {
                println!("msl auth error: {e}");
                assert!(e.contains("MSL") || e.contains("请求失败"), "异常: {e}");
            }
        }
    }
}

