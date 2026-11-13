//! ChmlFrp 开放 API
//!
//! 参考文档：<https://docs.chmlfrp.net/API/v2/>
//!
//! - 基础地址：`https://cf-v2.uapis.cn`（文档标注为 http，实际仅 https 可用）
//! - 鉴权：`access_token` 查询参数（用户令牌；`token` 参数用于统一登录态，二者不要混用）
//! - 隧道配置：`/tunnel_config` 返回 **INI** 格式文本，需要走旧版解析器
//!
//! 文档 v2 的字段为全小写（`localip` / `nport` / `dorp` / `usergroup` …），
//! 而旧版 SDK 与 v1 使用驼峰（`localIP` / `localPort` / `bandDomain` / `userGroup` …）。
//! 这里统一做双写兼容，避免服务端在 v1/v2 之间切换时登录成功却读不到任何数据。

use super::{http, profile_from_parsed, url_encode, ProviderTunnel, ProviderUser, KV};
use crate::config::parse_config_text;
use crate::model::Profile;
use serde_json::Value;

const BASE: &str = "https://cf-v2.uapis.cn";

fn get(path: &str, token: &str, extra: &[(&str, &str)]) -> Result<Value, String> {
    let mut url = format!("{BASE}{path}?access_token={}", url_encode(token));
    for (k, v) in extra {
        url.push('&');
        url.push_str(&format!("{k}={}", url_encode(v)));
    }
    let resp = http()?
        .get(url)
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let text = resp.text().map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))?;

    let code = v.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
    let state = v.get("state").and_then(|s| s.as_str()).unwrap_or("");
    let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("请求失败");
    if code != 200 || (!state.is_empty() && state != "success") {
        // 统一把鉴权类错误翻译成用户可执行的提示
        let auth_like = matches!(code, 400 | 401 | 403)
            || msg.contains("登录态")
            || msg.contains("登录状态")
            || msg.contains("令牌")
            || msg.contains("token");
        if auth_like {
            return Err(format!(
                "ChmlFrp 访问令牌无效或已过期（{msg}）。请在面板「用户中心 → 访问令牌」重新复制后粘贴登录"
            ));
        }
        return Err(format!("ChmlFrp 返回错误：{msg}（code {code}）"));
    }
    Ok(v.get("data").cloned().unwrap_or(Value::Null))
}

pub fn login_key(token: &str) -> Result<ProviderUser, String> {
    let data = get("/userinfo", token, &[])?;
    if data.is_null() {
        return Err("访问令牌无效或已过期，请重新复制".to_string());
    }
    Ok(parse_user(&data))
}

pub fn user(token: &str) -> Result<ProviderUser, String> {
    login_key(token)
}

pub fn tunnels(token: &str) -> Result<Vec<ProviderTunnel>, String> {
    let data = get("/tunnel", token, &[])?;
    let account_speed = get("/userinfo", token, &[])
        .map(|u| {
            let bw = i64_of(&u, &["bandwidth"]);
            if bw <= 0 {
                String::new()
            } else {
                format!("{bw} Mbps")
            }
        })
        .unwrap_or_default();
    let Some(list) = data.as_array() else {
        return Ok(Vec::new());
    };
    Ok(list.iter().map(|t| tunnel_of(t, &account_speed)).collect())
}

pub fn profile(
    token: &str,
    ids: &[String],
    name: Option<&str>,
    provider: &str,
) -> Result<Profile, String> {
    let all = tunnels(token)?;
    let mut node = String::new();
    let mut names: Vec<String> = Vec::new();

    for id in ids {
        let t = all
            .iter()
            .find(|x| &x.id == id)
            .ok_or_else(|| format!("未找到隧道 {id}（可能已被删除）"))?;
        if node.is_empty() {
            node = t.node.clone();
        } else if node != t.node {
            return Err(format!(
                "所选隧道不在同一节点（{node} 与 {}），请分批次导入",
                t.node
            ));
        }
        names.push(t.name.clone());
    }

    if node.is_empty() {
        return Err("未获取到隧道所属节点".to_string());
    }

    let data = get(
        "/tunnel_config",
        token,
        &[("node", &node), ("tunnel_names", &names.join(","))],
    )?;
    let cfg = data.as_str().unwrap_or_default();
    if cfg.trim().is_empty() {
        return Err("服务端未返回配置内容".to_string());
    }
    let parsed = parse_config_text(cfg)?;
    profile_from_parsed(parsed, provider, ids, name, token)
}

// ============ 字段兼容读取 ============

/// 按优先级读取字符串字段，兼容字符串 / 数字 / 布尔
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

/// 按优先级读取整数字段，兼容字符串数字 / 浮点 / 布尔
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
                if let Ok(n) = s.trim().parse::<f64>() {
                    return n as i64;
                }
            }
            if let Some(b) = x.as_bool() {
                return i64::from(b);
            }
        }
    }
    0
}

fn bool_of(v: &Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(x) = v.get(*key) {
            if let Some(b) = x.as_bool() {
                return Some(b);
            }
            if let Some(n) = x.as_i64() {
                return Some(n != 0);
            }
            if let Some(s) = x.as_str() {
                return Some(matches!(
                    s.trim().to_lowercase().as_str(),
                    "true" | "1" | "yes" | "on" | "是"
                ));
            }
        }
    }
    None
}

/// 形如 `["a.com","b.com"]` / `a.com,b.com` / `a.com` 的域名统一成逗号分隔
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
        .trim_matches('\'')
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .replace(' ', "")
}

/// 从 `12345` / `host:12345` 中取端口
fn port_of(raw: &str) -> i64 {
    raw.trim()
        .rsplit_once(':')
        .map(|(_, p)| p)
        .unwrap_or(raw)
        .trim()
        .parse::<i64>()
        .unwrap_or(0)
}

fn parse_user(v: &Value) -> ProviderUser {
    let mut extras = Vec::new();
    let bandwidth = i64_of(v, &["bandwidth"]);
    if bandwidth > 0 {
        extras.push(KV {
            label: "带宽".into(),
            value: format!("{bandwidth} Mbps"),
        });
    }
    let integral = str_of(v, &["integral"]);
    if !integral.is_empty() && integral != "0" {
        extras.push(KV {
            label: "积分".into(),
            value: integral,
        });
    }
    let real_name = str_of(v, &["realname", "realName"]);
    if !real_name.is_empty() {
        extras.push(KV {
            label: "实名".into(),
            value: if real_name == "true" { "已实名".into() } else { real_name },
        });
    }
    let term = str_of(v, &["term"]);
    if !term.is_empty() {
        extras.push(KV {
            label: "到期".into(),
            value: term,
        });
    }
    let qq = str_of(v, &["qq"]);
    if !qq.is_empty() && qq != "0" {
        extras.push(KV {
            label: "QQ".into(),
            value: qq,
        });
    }

    let upload = i64_of(v, &["total_upload"]);
    let download = i64_of(v, &["total_download"]);
    let traffic = if upload + download > 0 {
        super::bytes_text(upload + download)
    } else {
        "-".to_string()
    };

    // v2：tunnel = 已开通隧道数，tunnnelCount（文档拼写）= 上限；
    // v1：tunnel = 可开通隧道数，usedTunnel = 已开通数。
    let legacy_used = v.get("usedTunnel").map(|_| i64_of(v, &["usedTunnel"]));
    let (used, limit) = match legacy_used {
        Some(used) => {
            let limit = i64_of(v, &["tunnel", "tunnelCount"]);
            (used, if limit > 0 { limit } else { used })
        }
        None => {
            let used = i64_of(v, &["tunnel", "tunnelstate"]);
            let limit = i64_of(v, &["tunnnelCount", "tunnelCount"]);
            (used, if limit > 0 { limit } else { used })
        }
    };

    ProviderUser {
        provider: "chmlfrp".into(),
        user_id: str_of(v, &["id", "userID", "userid"]),
        username: str_of(v, &["username", "userName"]),
        email: str_of(v, &["email", "mail"]),
        group: str_of(v, &["usergroup", "userGroup"]),
        tunnel_limit: limit,
        tunnel_used: used,
        traffic,
        signed: bool_of(v, &["qianDao"]),
        extras,
    }
}

fn is_online(state: &str) -> bool {
    matches!(
        state.trim().to_lowercase().as_str(),
        "true" | "1" | "online" | "running" | "在线" | "运行中"
    )
}

fn tunnel_of(t: &Value, account_speed: &str) -> ProviderTunnel {
    let proxy_type = str_of(t, &["type", "portType", "proxyType"]).to_lowercase();
    let is_web = matches!(proxy_type.as_str(), "http" | "https");
    let raw_remote = str_of(t, &["dorp", "bandDomain", "domain"]);
    let domains = if is_web {
        normalize_domains(&raw_remote)
    } else {
        String::new()
    };
    let remote_port = {
        let p = i64_of(t, &["remotePort", "remote_port"]);
        if p > 0 {
            p
        } else if !is_web {
            port_of(&raw_remote)
        } else {
            0
        }
    };
    let local_ip = {
        let ip = str_of(t, &["localip", "localIP", "localIp"]);
        if ip.is_empty() {
            "127.0.0.1".into()
        } else {
            ip
        }
    };
    let local_port = i64_of(t, &["nport", "localPort", "local_port"]);
    let state = str_of(t, &["state", "tunnelState", "nodestate"]);

    let remote_desc = if is_web {
        if domains.is_empty() {
            "-".into()
        } else {
            domains.clone()
        }
    } else if !raw_remote.is_empty() {
        raw_remote.clone()
    } else if remote_port > 0 {
        format!(":{remote_port}")
    } else {
        "-".into()
    };

    ProviderTunnel {
        id: str_of(t, &["id", "tunnelID", "tunnelId"]),
        name: str_of(t, &["name", "tunnelName"]),
        proxy_type,
        local_ip,
        local_port,
        remote_port,
        domains,
        remote_desc,
        speed: account_speed.to_string(),
        node: str_of(t, &["node", "nodeName"]),
        online: is_online(&state),
        disabled: false,
        banned: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_v2_user_fields() {
        let v: Value = serde_json::from_str(
            r#"{
                "id": 42,
                "username": "demo",
                "email": "demo@example.com",
                "usergroup": "VIP",
                "realname": "已实名",
                "tunnel": 3,
                "tunnnelCount": 8,
                "bandwidth": 10,
                "integral": 120,
                "total_upload": 1024,
                "total_download": 2048,
                "term": "2027-01-01",
                "qq": "123456"
            }"#,
        )
        .unwrap();
        let u = parse_user(&v);
        assert_eq!(u.user_id, "42");
        assert_eq!(u.username, "demo");
        assert_eq!(u.email, "demo@example.com");
        assert_eq!(u.group, "VIP");
        assert_eq!(u.tunnel_used, 3);
        assert_eq!(u.tunnel_limit, 8);
        assert_eq!(u.traffic, "3.00 KB");
        assert!(u.extras.iter().any(|k| k.label == "带宽" && k.value == "10 Mbps"));
    }

    #[test]
    fn parses_legacy_user_fields() {
        let v: Value = serde_json::from_str(
            r#"{
                "userID": "7",
                "userName": "old",
                "mail": "old@example.com",
                "userGroup": "普通用户",
                "tunnel": 2,
                "usedTunnel": 1,
                "bandwidth": "5"
            }"#,
        )
        .unwrap();
        let u = parse_user(&v);
        assert_eq!(u.user_id, "7");
        assert_eq!(u.username, "old");
        assert_eq!(u.email, "old@example.com");
        assert_eq!(u.tunnel_limit, 2);
        assert_eq!(u.tunnel_used, 1);
    }

    #[test]
    fn parses_v2_tunnel_fields() {
        let v: Value = serde_json::from_str(
            r#"{
                "id": 12,
                "name": "web",
                "node": "上海-1",
                "type": "https",
                "localip": "127.0.0.1",
                "nport": 8080,
                "dorp": "[\"demo.chmlfrp.net\"]",
                "state": "online"
            }"#,
        )
        .unwrap();
        let t = tunnel_of(&v, "10 Mbps");
        assert_eq!(t.id, "12");
        assert_eq!(t.name, "web");
        assert_eq!(t.node, "上海-1");
        assert_eq!(t.local_ip, "127.0.0.1");
        assert_eq!(t.local_port, 8080);
        assert_eq!(t.domains, "demo.chmlfrp.net");
        assert!(t.online);
        assert!(t.remote_desc.contains("demo.chmlfrp.net"));
    }

    #[test]
    fn parses_tcp_tunnel_fields() {
        let v: Value = serde_json::from_str(
            r#"{
                "id": 9,
                "name": "ssh",
                "node": "北京-2",
                "type": "tcp",
                "localip": "192.168.1.2",
                "nport": 22,
                "dorp": "6000",
                "state": "离线"
            }"#,
        )
        .unwrap();
        let t = tunnel_of(&v, "");
        assert_eq!(t.remote_port, 6000);
        assert_eq!(t.remote_desc, "6000");
        assert!(!t.online);
    }

    /// 网络冒烟：无效 token 应返回可读的鉴权错误
    #[test]
    fn invalid_token_is_rejected() {
        match super::user("nettool-bad-token") {
            Ok(u) => println!("意外成功: {}", u.username),
            Err(e) => {
                println!("chmlfrp auth error: {e}");
                assert!(e.contains("访问令牌") || e.contains("请求失败"), "异常: {e}");
            }
        }
    }

    #[test]
    fn online_state_parsing() {
        assert!(is_online("true"));
        assert!(is_online("True"));
        assert!(is_online("运行中"));
        assert!(!is_online("false"));
        assert!(!is_online(""));
    }
}
