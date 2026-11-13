use crate::model::{Profile, TunnelConfig};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub server_addr: Option<String>,
    pub server_port: Option<u16>,
    pub user: Option<String>,
    pub auth_method: Option<String>,
    pub auth_token: Option<String>,
    pub tunnels: Vec<TunnelConfig>,
}

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// 将配置渲染为 frpc.toml
///
/// - `local_ports`：隧道 id -> 网关端口（流量统计网关），缺省时使用隧道自身 localPort
/// - `web_port`：frpc 本地管理接口端口，0 表示不启用
pub fn render_config(profile: &Profile, local_ports: &HashMap<String, u16>, web_port: u16) -> String {
    let mut out = String::new();

    out.push_str(&format!("serverAddr = \"{}\"\n", esc(profile.server_addr.trim())));
    out.push_str(&format!("serverPort = {}\n", profile.server_port));
    if !profile.user.trim().is_empty() {
        out.push_str(&format!("user = \"{}\"\n", esc(profile.user.trim())));
    }
    out.push_str(&format!(
        "transport.protocol = \"{}\"\n",
        esc(if profile.transport.trim().is_empty() { "tcp" } else { profile.transport.trim() })
    ));
    if profile.tls_enable {
        out.push_str("transport.tls.enable = true\n");
    }

    // 日志走 stdout，由应用统一收集，避免文件锁与重复写入
    out.push_str("log.to = \"console\"\n");
    out.push_str(&format!(
        "log.level = \"{}\"\n",
        esc(if profile.log_level.trim().is_empty() { "info" } else { profile.log_level.trim() })
    ));
    out.push_str("log.maxDays = 3\n");

    if web_port > 0 {
        out.push_str("webServer.addr = \"127.0.0.1\"\n");
        out.push_str(&format!("webServer.port = {}\n", web_port));
    }

    if !profile.auth_token.trim().is_empty() {
        out.push_str("\n[auth]\nmethod = \"token\"\n");
        out.push_str(&format!("token = \"{}\"\n", esc(profile.auth_token.trim())));
    }

    for t in profile.tunnels.iter().filter(|t| t.enabled) {
        out.push('\n');
        out.push_str("[[proxies]]\n");
        out.push_str(&format!("name = \"{}\"\n", esc(t.name.trim())));
        out.push_str(&format!("type = \"{}\"\n", esc(&t.proxy_type)));
        out.push_str(&format!("localIP = \"{}\"\n", esc(t.local_ip.trim())));

        let port = local_ports.get(&t.id).copied().unwrap_or(t.local_port);
        out.push_str(&format!("localPort = {}\n", port));

        match t.proxy_type.as_str() {
            "tcp" | "udp" => {
                if t.remote_port > 0 {
                    out.push_str(&format!("remotePort = {}\n", t.remote_port));
                }
            }
            "http" | "https" | "tcpmux" => {
                if let Some(domains) = non_empty(&t.custom_domains) {
                    let list: Vec<String> = domains
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(|d| format!("\"{}\"", esc(d)))
                        .collect();
                    if !list.is_empty() {
                        out.push_str(&format!("customDomains = [{}]\n", list.join(", ")));
                    }
                }
                if let Some(sub) = non_empty(&t.subdomain) {
                    out.push_str(&format!("subdomain = \"{}\"\n", esc(sub)));
                }
            }
            "stcp" | "xtcp" => {
                if let Some(key) = non_empty(&t.secret_key) {
                    out.push_str(&format!("secretKey = \"{}\"\n", esc(key)));
                }
            }
            _ => {}
        }

        if let Some(bw) = non_empty(&t.bandwidth_limit) {
            out.push_str(&format!("transport.bandwidthLimit = \"{}\"\n", esc(bw)));
        }
        if let Some(v) = t.use_encryption {
            out.push_str(&format!("transport.useEncryption = {}\n", v));
        }
        if let Some(v) = t.use_compression {
            out.push_str(&format!("transport.useCompression = {}\n", v));
        }
    }

    out
}

/// 解析 frpc.toml：既支持新格式（dotted keys），也兼容旧格式（[common] 段）
pub fn parse_toml(content: &str) -> Result<ParseResult, String> {
    let value: toml::Value = toml::from_str(content).map_err(|e| format!("TOML 解析失败: {e}"))?;
    let table = value.as_table().ok_or("配置根节点必须是表")?;

    // 兼容 frp 旧版本 [common] 段
    let root = if let Some(common) = table.get("common").and_then(|v| v.as_table()) {
        common
    } else {
        table
    };

    let server_addr = root
        .get("serverAddr")
        .or_else(|| root.get("server_addr"))
        .and_then(|v| v.as_str())
        .map(String::from);
    let server_port = root
        .get("serverPort")
        .or_else(|| root.get("server_port"))
        .and_then(|v| v.as_integer())
        .map(|v| v as u16);
    let user = root.get("user").and_then(|v| v.as_str()).map(String::from);

    // auth 既可能是表，也可能是 dotted key
    let auth = root.get("auth").and_then(|v| v.as_table());
    let auth_method = auth
        .and_then(|a| a.get("method"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| root.get("auth.method").and_then(|v| v.as_str()).map(String::from));
    let auth_token = auth
        .and_then(|a| a.get("token"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| root.get("auth.token").and_then(|v| v.as_str()).map(String::from))
        .or_else(|| {
            root.get("token")
                .and_then(|v| v.as_str())
                .map(String::from)
        });

    let mut tunnels = Vec::new();
    if let Some(proxies) = table.get("proxies").and_then(|v| v.as_array()) {
        for p in proxies {
            let Some(pt) = p.as_table() else { continue };
            let name = pt.get("name").and_then(|v| v.as_str()).unwrap_or("proxy").to_string();
            let proxy_type = pt.get("type").and_then(|v| v.as_str()).unwrap_or("tcp").to_string();
            let local_ip = pt
                .get("localIP")
                .or_else(|| pt.get("localIp"))
                .or_else(|| pt.get("local_ip"))
                .and_then(|v| v.as_str())
                .unwrap_or("127.0.0.1")
                .to_string();
            let local_port = pt.get("localPort").and_then(|v| v.as_integer()).unwrap_or(0) as u16;
            let remote_port = pt.get("remotePort").and_then(|v| v.as_integer()).unwrap_or(0) as u16;
            let custom_domains = pt
                .get("customDomains")
                .and_then(|v| match v {
                    toml::Value::Array(arr) => Some(
                        arr.iter()
                            .filter_map(|x| x.as_str())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    toml::Value::String(s) => Some(s.clone()),
                    _ => None,
                })
                .filter(|s| !s.is_empty());
            let subdomain = pt.get("subdomain").and_then(|v| v.as_str()).map(String::from);
            let secret_key = pt.get("secretKey").and_then(|v| v.as_str()).map(String::from);
            let bandwidth_limit = pt
                .get("bandwidthLimit")
                .and_then(|v| v.as_str())
                .map(String::from)
                .or_else(|| {
                    pt.get("transport")
                        .and_then(|v| v.as_table())
                        .and_then(|t| t.get("bandwidthLimit"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                });

            // transport 子表（新）或平铺字段（旧）
            let transport = pt.get("transport").and_then(|v| v.as_table());
            let use_encryption = transport
                .and_then(|t| t.get("useEncryption"))
                .and_then(|v| v.as_bool())
                .or_else(|| pt.get("useEncryption").and_then(|v| v.as_bool()));
            let use_compression = transport
                .and_then(|t| t.get("useCompression"))
                .and_then(|v| v.as_bool())
                .or_else(|| pt.get("useCompression").and_then(|v| v.as_bool()));

            tunnels.push(TunnelConfig {
                id: crate::store::gen_id(),
                name,
                proxy_type,
                local_ip,
                local_port,
                remote_port,
                custom_domains,
                subdomain,
                secret_key,
                use_encryption,
                use_compression,
                bandwidth_limit,
                enabled: true,
                source: Some("toml".to_string()),
            });
        }
    }

    Ok(ParseResult {
        server_addr,
        server_port,
        user,
        auth_method,
        auth_token,
        tunnels,
    })
}

// ============ Tauri 命令 ============

/// 通用配置解析：先按 TOML 解析，若得到的是旧版 INI（[common] + [隧道名]）则回退 INI 解析器
pub fn parse_config_text(text: &str) -> Result<ParseResult, String> {
    if let Ok(parsed) = parse_toml(text) {
        if !parsed.tunnels.is_empty() {
            return Ok(parsed);
        }
        // 形如 [common] 的旧版配置在 TOML 里也是合法表，但 proxies 为空
        if let Ok(ini) = parse_legacy_ini(text) {
            if !ini.tunnels.is_empty() {
                return Ok(ini);
            }
        }
        if parsed.server_addr.is_some() {
            return Ok(parsed);
        }
    }
    parse_legacy_ini(text)
}

/// 解析 frp 旧版 INI 配置：`[common]` 段为服务器参数，其余段每条一个隧道
fn parse_legacy_ini(content: &str) -> Result<ParseResult, String> {
    use std::collections::HashMap;

    let mut order: Vec<String> = Vec::new();
    let mut sections: HashMap<String, HashMap<String, String>> = HashMap::new();
    let mut current = String::new();

    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].trim().to_string();
            if !sections.contains_key(&current) {
                order.push(current.clone());
                sections.insert(current.clone(), HashMap::new());
            }
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if current.is_empty() {
                continue;
            }
            let value = v.trim().trim_matches('"').trim_matches('\'').to_string();
            sections
                .entry(current.clone())
                .or_default()
                .insert(k.trim().to_lowercase(), value);
        }
    }

    if sections.is_empty() {
        return Err("无法识别的配置格式（既不是 TOML 也不是 frp INI）".to_string());
    }

    let empty = HashMap::new();
    let common = sections.get("common").unwrap_or(&empty);
    let pick = |m: &HashMap<String, String>, keys: &[&str]| -> Option<String> {
        keys.iter().find_map(|k| m.get(*k).cloned())
    };

    let server_addr = pick(common, &["server_addr", "serveraddr"]);
    let server_port = pick(common, &["server_port", "serverport"]).and_then(|v| v.parse::<u16>().ok());
    let user = pick(common, &["user", "user_name"]);
    let auth_token = pick(common, &["token", "auth_token"]);

    let mut tunnels = Vec::new();
    for name in order.iter().filter(|n| n.as_str() != "common") {
        let sec = &sections[name];
        let proxy_type = pick(sec, &["type"]).unwrap_or_else(|| "tcp".to_string());
        let is_web = matches!(proxy_type.as_str(), "http" | "https");
        let custom_domains = if is_web {
            pick(sec, &["custom_domains", "customdomains"])
        } else {
            None
        };
        let subdomain = if is_web { pick(sec, &["subdomain"]) } else { None };
        let use_encryption = pick(sec, &["use_encryption"]).map(|v| v == "true");
        let use_compression = pick(sec, &["use_compression"]).map(|v| v == "true");

        tunnels.push(TunnelConfig {
            id: crate::store::gen_id(),
            name: name.clone(),
            proxy_type,
            local_ip: pick(sec, &["local_ip", "localip"]).unwrap_or_else(|| "127.0.0.1".to_string()),
            local_port: pick(sec, &["local_port", "localport"])
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(0),
            remote_port: pick(sec, &["remote_port", "remoteport"])
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(0),
            custom_domains,
            subdomain,
            secret_key: pick(sec, &["sk", "secret_key"]),
            use_encryption,
            use_compression,
            bandwidth_limit: pick(sec, &["bandwidth_limit"]),
            enabled: true,
            source: Some("ini".to_string()),
        });
    }

    Ok(ParseResult {
        server_addr,
        server_port,
        user,
        auth_method: Some("token".to_string()),
        auth_token,
        tunnels,
    })
}

#[tauri::command]
pub fn parse_toml_tunnels(content: String) -> Result<ParseResult, String> {
    parse_config_text(&content)
}

/// 预览/导出配置：使用隧道真实本地端口，不启用网关与管理接口
#[tauri::command]
pub fn preview_config(profile: Profile) -> Result<String, String> {
    Ok(render_config(&profile, &HashMap::new(), 0))
}

/// 兼容旧接口：仅渲染 proxies（服务器参数由调用方给出）
#[tauri::command]
pub fn tunnels_to_toml(
    server_addr: String,
    server_port: u16,
    user: String,
    auth_token: String,
    tunnels: Vec<TunnelConfig>,
) -> Result<String, String> {
    let profile = Profile {
        id: "preview".into(),
        name: "preview".into(),
        server_addr,
        server_port,
        user,
        auth_token,
        transport: "tcp".into(),
        tls_enable: true,
        log_level: "info".into(),
        tunnels,
        ..Default::default()
    };
    Ok(render_config(&profile, &HashMap::new(), 0))
}

#[tauri::command]
pub fn read_config(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_config(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_dir_files(path: String) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(&path).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        if entry.path().is_file() {
            if let Some(name) = entry.file_name().to_str() {
                files.push(name.to_string());
            }
        }
    }
    files.sort();
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Profile;

    fn profile_with(tunnels: Vec<TunnelConfig>) -> Profile {
        Profile {
            id: "p1".into(),
            name: "test".into(),
            server_addr: "frp.example.com".into(),
            server_port: 7000,
            user: "jack".into(),
            auth_token: "secret".into(),
            transport: "websocket".into(),
            tls_enable: true,
            log_level: "info".into(),
            tunnels,
            ..Default::default()
        }
    }

    fn tcp_tunnel() -> TunnelConfig {
        TunnelConfig {
            id: "t1".into(),
            name: "ssh".into(),
            proxy_type: "tcp".into(),
            local_ip: "127.0.0.1".into(),
            local_port: 22,
            remote_port: 6000,
            use_encryption: Some(true),
            ..Default::default()
        }
    }

    #[test]
    fn renders_full_config_with_gateway_port() {
        let mut ports = HashMap::new();
        ports.insert("t1".to_string(), 54321u16);
        let out = render_config(&profile_with(vec![tcp_tunnel()]), &ports, 7401);
        assert!(out.contains("localPort = 54321"), "{out}");
        assert!(!out.contains("localPort = 22"), "应使用网关端口: {out}");
        assert!(out.contains("transport.protocol = \"websocket\""));
        assert!(out.contains("transport.tls.enable = true"));
        assert!(out.contains("[auth]"));
        assert!(out.contains("token = \"secret\""));
        assert!(out.contains("webServer.port = 7401"));
        assert!(out.contains("remotePort = 6000"));
        assert!(out.contains("transport.useEncryption = true"));
    }

    #[test]
    fn http_tunnel_uses_domain_not_remote_port() {
        let t = TunnelConfig {
            id: "t2".into(),
            name: "web".into(),
            proxy_type: "http".into(),
            local_ip: "127.0.0.1".into(),
            local_port: 8080,
            remote_port: 80,
            custom_domains: Some("a.example.com, b.example.com".into()),
            ..Default::default()
        };
        let out = render_config(&profile_with(vec![t]), &HashMap::new(), 0);
        assert!(
            out.contains("customDomains = [\"a.example.com\", \"b.example.com\"]"),
            "{out}"
        );
        assert!(!out.contains("remotePort"), "http 隧道不应写 remotePort: {out}");
        assert!(!out.contains("webServer"), "web_port=0 时不应写 webServer: {out}");
    }

    #[test]
    fn parses_new_and_legacy_style() {
        let new_style = r#"
serverAddr = "1.2.3.4"
serverPort = 7000
user = "u"
transport.protocol = "kcp"
[auth]
method = "token"
token = "abc"

[[proxies]]
name = "web"
type = "http"
localIP = "127.0.0.1"
localPort = 8080
customDomains = ["x.example.com"]
"#;
        let r = parse_toml(new_style).unwrap();
        assert_eq!(r.server_addr.as_deref(), Some("1.2.3.4"));
        assert_eq!(r.server_port, Some(7000));
        assert_eq!(r.auth_token.as_deref(), Some("abc"));
        assert_eq!(r.tunnels.len(), 1);
        assert_eq!(r.tunnels[0].custom_domains.as_deref(), Some("x.example.com"));

        let legacy = r#"
[common]
server_addr = "5.6.7.8"
server_port = 7001
token = "old"

[ssh]
type = "tcp"
local_ip = "127.0.0.1"
local_port = 22
remote_port = 6001
"#;
        let r2 = parse_toml(legacy).unwrap();
        assert_eq!(r2.server_addr.as_deref(), Some("5.6.7.8"));
        assert_eq!(r2.auth_token.as_deref(), Some("old"));
    }

    #[test]
    fn round_trip_keeps_tunnel_fields() {
        let out = render_config(&profile_with(vec![tcp_tunnel()]), &HashMap::new(), 0);
        let parsed = parse_toml(&out).unwrap();
        assert_eq!(parsed.tunnels.len(), 1);
        assert_eq!(parsed.tunnels[0].local_port, 22);
        assert_eq!(parsed.tunnels[0].remote_port, 6000);
        assert_eq!(parsed.tunnels[0].local_ip, "127.0.0.1");
        assert_eq!(parsed.tunnels[0].use_encryption, Some(true));
    }

    #[test]
    fn http_tunnel_with_subdomain_round_trip() {
        let t = TunnelConfig {
            id: "t3".into(),
            name: "site".into(),
            proxy_type: "https".into(),
            local_ip: "127.0.0.1".into(),
            local_port: 3000,
            remote_port: 0,
            subdomain: Some("demo".into()),
            ..Default::default()
        };
        let out = render_config(&profile_with(vec![t]), &HashMap::new(), 0);
        assert!(out.contains("type = \"https\""), "{out}");
        assert!(out.contains("subdomain = \"demo\""), "{out}");
        assert!(!out.contains("customDomains"), "{out}");
        assert!(!out.contains("remotePort"), "https 隧道不应写 remotePort: {out}");

        let parsed = parse_toml(&out).unwrap();
        assert_eq!(parsed.tunnels.len(), 1);
        assert_eq!(parsed.tunnels[0].proxy_type, "https");
        assert_eq!(parsed.tunnels[0].subdomain.as_deref(), Some("demo"));
        assert_eq!(parsed.tunnels[0].remote_port, 0);
    }

    #[test]
    fn parses_http_proxy_from_external_config() {
        // 兼容第三方服务商生成的 http 配置（域名数组 + transport 子表）
        let cfg = r#"
serverAddr = "node.example.com"
serverPort = 7000

[auth]
method = "token"
token = "tk"

[[proxies]]
name = "blog"
type = "http"
localIP = "127.0.0.1"
localPort = 8080
customDomains = ["blog.example.com", "www.blog.example.com"]

[proxies.transport]
useEncryption = true
useCompression = false
"#;
        let r = parse_toml(cfg).unwrap();
        assert_eq!(r.server_addr.as_deref(), Some("node.example.com"));
        assert_eq!(r.auth_token.as_deref(), Some("tk"));
        assert_eq!(r.tunnels.len(), 1);
        let t = &r.tunnels[0];
        assert_eq!(t.proxy_type, "http");
        assert_eq!(
            t.custom_domains.as_deref(),
            Some("blog.example.com,www.blog.example.com")
        );
        assert_eq!(t.use_encryption, Some(true));
        assert_eq!(t.use_compression, Some(false));
    }

    #[test]
    fn accepts_frontend_local_ip_casing() {
        // 前端历史上使用 localIp，TOML 风格为 localIP，二者都必须能反序列化
        let json = r#"{"id":"a","name":"n","type":"tcp","localIp":"127.0.0.1","localPort":22,"remotePort":6000,"enabled":true}"#;
        let a: TunnelConfig = serde_json::from_str(json).unwrap();
        assert_eq!(a.local_port, 22);
        let json2 = r#"{"id":"a","name":"n","type":"tcp","localIP":"10.0.0.2","localPort":22,"remotePort":6000,"enabled":true}"#;
        let b: TunnelConfig = serde_json::from_str(json2).unwrap();
        assert_eq!(b.local_ip, "10.0.0.2");
    }

}
