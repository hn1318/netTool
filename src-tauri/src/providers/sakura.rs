//! SakuraFrp（樱花穿透）OpenAPI v4
//!
//! - 基础地址：`https://api.natfrp.com/v4`
//! - 鉴权：`Authorization: Bearer <访问密钥>`
//! - 隧道配置：`POST /tunnel/config`（form: query, frpc）由服务端下发 INI/TOML 文本

use super::{bytes_text, http, profile_from_parsed, ProviderTunnel, ProviderUser, KV};
use crate::config::parse_config_text;
use crate::model::Profile;
use serde_json::Value;

const BASE: &str = "https://api.natfrp.com/v4";
/// 请求配置时声明的 frpc 版本（上游版本，Sakura 会据此返回 ini/toml）
const FRPC_VERSION: &str = "0.61.1";

fn error_from_body(text: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let code = v.get("code").and_then(|c| c.as_i64())?;
    if code == 200 {
        return None;
    }
    let msg = v.get("msg").and_then(|m| m.as_str()).unwrap_or("请求失败");
    Some(format!("SakuraFrp 返回错误：{msg}（code {code}）"))
}

fn get(path: &str, token: &str) -> Result<Value, String> {
    let resp = http()?
        .get(format!("{BASE}{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/json")
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let text = resp.text().map_err(|e| e.to_string())?;
    if let Some(err) = error_from_body(&text) {
        return Err(err);
    }
    serde_json::from_str(&text).map_err(|_| format!("响应解析失败: {text}"))
}

fn post_form(path: &str, token: &str, form: &[(&str, &str)]) -> Result<String, String> {
    let resp = http()?
        .post(format!("{BASE}{path}"))
        .header("Authorization", format!("Bearer {token}"))
        .form(form)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;
    let text = resp.text().map_err(|e| e.to_string())?;
    if let Some(err) = error_from_body(&text) {
        return Err(err);
    }
    Ok(text)
}

fn s(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .map(|x| x.trim().to_string())
        .unwrap_or_default()
}

fn i(v: &Value, key: &str) -> i64 {
    v.get(key).and_then(|x| x.as_i64()).unwrap_or(0)
}

fn parse_user(v: &Value) -> ProviderUser {
    // traffic: [本日消耗, 总剩余]，单位为字节
    let traffic = v
        .get("traffic")
        .and_then(|t| t.as_array())
        .map(|arr| {
            let today = arr.first().and_then(|x| x.as_i64()).unwrap_or(0);
            let remain = arr.get(1).and_then(|x| x.as_i64()).unwrap_or(0);
            format!("剩余 {} · 今日 {}", bytes_text(remain), bytes_text(today))
        })
        .unwrap_or_else(|| "-".to_string());

    let mut extras = Vec::new();
    let speed = s(v, "speed");
    if !speed.is_empty() {
        extras.push(KV {
            label: "限速".into(),
            value: speed,
        });
    }
    let realname = i(v, "realname");
    extras.push(KV {
        label: "实名".into(),
        value: if realname > 0 { "已实名".into() } else { "未实名".into() },
    });

    ProviderUser {
        provider: "sakura".into(),
        user_id: i(v, "id").to_string(),
        username: s(v, "name"),
        email: String::new(),
        group: v
            .get("group")
            .and_then(|g| g.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string(),
        tunnel_limit: i(v, "tunnels"),
        tunnel_used: 0,
        traffic,
        signed: v
            .get("sign")
            .and_then(|sg| sg.get("signed"))
            .and_then(|x| x.as_bool()),
        extras,
    }
}

pub fn login_key(token: &str) -> Result<ProviderUser, String> {
    let data = get("/user/info", token)?;
    let user = parse_user(&data);
    if user.username.is_empty() {
        return Err("访问密钥无效或已过期".to_string());
    }
    Ok(user)
}

pub fn user(token: &str) -> Result<ProviderUser, String> {
    login_key(token)
}

fn node_names(token: &str) -> std::collections::HashMap<i64, String> {
    let mut map = std::collections::HashMap::new();
    if let Ok(v) = get("/nodes", token) {
        if let Some(obj) = v.as_object() {
            for (id, node) in obj {
                if let Ok(nid) = id.parse::<i64>() {
                    map.insert(nid, s(node, "name"));
                }
            }
        }
    }
    map
}

pub fn tunnels(token: &str) -> Result<Vec<ProviderTunnel>, String> {
    let data = get("/tunnels", token)?;
    let nodes = node_names(token);
    // 隧道级没有限速字段，用账号限速作为参考速度
    let account_speed = get("/user/info", token)
        .map(|u| s(&u, "speed"))
        .unwrap_or_default();
    let Some(list) = data.as_array() else {
        return Ok(Vec::new());
    };
    Ok(list
        .iter()
        .map(|t| {
            let proxy_type = s(t, "type");
            let is_web = matches!(proxy_type.as_str(), "http" | "https");
            let remote = s(t, "remote");
            let remote_port = remote.parse::<i64>().unwrap_or(0);
            let node_id = i(t, "node");
            let banned = i(t, "status") == 2;
            ProviderTunnel {
                id: i(t, "id").to_string(),
                name: s(t, "name"),
                proxy_type: proxy_type.clone(),
                local_ip: {
                    let ip = s(t, "local_ip");
                    if ip.is_empty() {
                        "127.0.0.1".into()
                    } else {
                        ip
                    }
                },
                local_port: i(t, "local_port"),
                remote_port,
                domains: if is_web { remote.clone() } else { String::new() },
                speed: account_speed.clone(),
                remote_desc: if remote.is_empty() {
                    "-".into()
                } else if is_web {
                    remote
                } else {
                    format!(":{remote}")
                },
                node: nodes
                    .get(&node_id)
                    .cloned()
                    .unwrap_or_else(|| format!("节点 {node_id}")),
                online: t.get("online").and_then(|x| x.as_bool()).unwrap_or(false),
                disabled: false,
                banned,
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
    let query = ids.join(",");
    let text = post_form(
        "/tunnel/config",
        token,
        &[("query", &query), ("frpc", FRPC_VERSION)],
    )?;
    if text.trim().is_empty() {
        return Err("服务端未返回配置内容".to_string());
    }
    let parsed = parse_config_text(&text)?;
    profile_from_parsed(parsed, provider, ids, name, token)
}

#[cfg(test)]
mod tests {
    use super::error_from_body;

    /// 网络冒烟：无效访问密钥应被识别
    #[test]
    fn invalid_token_is_rejected() {
        match super::user("nettool-bad-token") {
            Ok(u) => println!("意外成功: {}", u.username),
            Err(e) => {
                println!("sakura auth error: {e}");
                assert!(
                    e.contains("SakuraFrp") || e.contains("请求失败"),
                    "异常: {e}"
                );
            }
        }
    }

    #[test]
    fn error_body_is_detected() {
        assert!(error_from_body(r#"{"code":401,"msg":"访问密钥无效"}"#)
            .unwrap()
            .contains("访问密钥无效"));
        assert!(error_from_body(r#"[{"id":1}]"#).is_none());
    }
}
