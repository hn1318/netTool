use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

// ============ 状态 ============
pub struct FrpState {
    running: Mutex<bool>,
    child: Mutex<Option<u32>>, // frpc 子进程 PID
}

impl Default for FrpState {
    fn default() -> Self {
        Self {
            running: Mutex::new(false),
            child: Mutex::new(None),
        }
    }
}

// ============ 结构体 ============
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    #[serde(rename = "localIP")]
    pub local_ip: String,
    #[serde(rename = "localPort")]
    pub local_port: i64,
    #[serde(rename = "remotePort")]
    pub remote_port: i64,
    #[serde(rename = "customDomains")]
    pub custom_domains: Option<String>,
    pub subdomain: Option<String>,
    #[serde(rename = "useEncryption")]
    pub use_encryption: Option<bool>,
    #[serde(rename = "useCompression")]
    pub use_compression: Option<bool>,
    #[serde(rename = "bandwidthLimit")]
    pub bandwidth_limit: Option<String>,
    pub enabled: bool,
    pub source: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FrpProcessInfo {
    pid: u32,
    running: bool,
}

#[derive(Debug, Serialize)]
pub struct ParseResult {
    server_addr: Option<String>,
    server_port: Option<i64>,
    user: Option<String>,
    auth_method: Option<String>,
    auth_token: Option<String>,
    tunnels: Vec<TunnelConfig>,
}

#[derive(Debug, Serialize)]
pub struct DownloadResult {
    path: String,
    version: String,
}

// ============ 工具 ============
fn now_str() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 86400;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let sec = secs % 60;
    format!("{:02}:{:02}:{:02}", hours, mins, sec)
}

fn emit_log_event(app: &tauri::AppHandle, level: &str, message: &str) {
    let payload = serde_json::json!({
        "level": level,
        "message": message,
        "timestamp": now_str(),
    });
    let _ = app.emit("frp-log", &payload);
}

fn frp_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join("frp"))
}

// ============ 基础命令 ============
#[tauri::command]
fn get_app_data_dir(app: tauri::AppHandle) -> Result<PathBuf, String> {
    app.path().app_data_dir().map_err(|e| e.to_string())
}

#[tauri::command]
fn read_config(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn write_config(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_dir_files(path: String) -> Result<Vec<String>, String> {
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

// ============ 正确的 TOML 隧道解析 ============
#[tauri::command]
fn parse_toml_tunnels(content: String) -> Result<ParseResult, String> {
    let value: toml::Value = toml::from_str(&content).map_err(|e| format!("TOML 解析失败: {e}"))?;

    let table = value.as_table().ok_or("配置根节点必须是表")?;

    let server_addr = table.get("serverAddr").and_then(|v| v.as_str()).map(String::from);
    let server_port = table.get("serverPort").and_then(|v| v.as_integer());
    let user = table.get("user").and_then(|v| v.as_str()).map(String::from);

    let auth = table.get("auth").and_then(|v| v.as_table());
    let auth_method = auth.and_then(|a| a.get("method")).and_then(|v| v.as_str()).map(String::from);
    let auth_token = auth.and_then(|a| a.get("token")).and_then(|v| v.as_str()).map(String::from);

    let mut tunnels = Vec::new();
    if let Some(proxies) = table.get("proxies").and_then(|v| v.as_array()) {
        for p in proxies {
            if let Some(pt) = p.as_table() {
                let name = pt.get("name").and_then(|v| v.as_str()).unwrap_or("proxy").to_string();
                let proxy_type = pt.get("type").and_then(|v| v.as_str()).unwrap_or("tcp").to_string();
                let local_ip = pt.get("localIP").and_then(|v| v.as_str()).unwrap_or("127.0.0.1").to_string();
                let local_port = pt.get("localPort").and_then(|v| v.as_integer()).unwrap_or(0);
                let remote_port = pt.get("remotePort").and_then(|v| v.as_integer()).unwrap_or(0);
                let custom_domains = pt.get("customDomains").and_then(|v| v.as_str()).map(String::from);
                let subdomain = pt.get("subdomain").and_then(|v| v.as_str()).map(String::from);
                let bandwidth_limit = pt.get("bandwidthLimit").and_then(|v| v.as_str()).map(String::from);

                // transport 子表
                let transport = pt.get("transport").and_then(|v| v.as_table());
                let use_encryption = transport.and_then(|t| t.get("useEncryption")).and_then(|v| v.as_bool());
                let use_compression = transport.and_then(|t| t.get("useCompression")).and_then(|v| v.as_bool());

                tunnels.push(TunnelConfig {
                    id: uuid_str(),
                    name,
                    proxy_type,
                    local_ip,
                    local_port,
                    remote_port,
                    custom_domains,
                    subdomain,
                    use_encryption,
                    use_compression,
                    bandwidth_limit,
                    enabled: true,
                    source: Some("toml".to_string()),
                });
            }
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

fn uuid_str() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}", nanos)
}

/// 将隧道列表 + 服务器配置渲染为 frpc.toml
#[tauri::command]
fn tunnels_to_toml(
    server_addr: String,
    server_port: i64,
    user: String,
    auth_token: String,
    tunnels: Vec<TunnelConfig>,
) -> Result<String, String> {
    let mut out = String::new();
    out.push_str(&format!("serverAddr = '{}'\n", escape_toml(&server_addr)));
    out.push_str(&format!("serverPort = {}\n", server_port));
    if !user.is_empty() {
        out.push_str(&format!("user = '{}'\n", escape_toml(&user)));
    }
    if !auth_token.is_empty() {
        out.push_str("\n[auth]\n");
        out.push_str("method = 'token'\n");
        out.push_str(&format!("token = '{}'\n", escape_toml(&auth_token)));
    }
    out.push('\n');

    for t in &tunnels {
        out.push_str("[[proxies]]\n");
        out.push_str(&format!("name = '{}'\n", escape_toml(&t.name)));
        out.push_str(&format!("type = '{}'\n", escape_toml(&t.proxy_type)));
        out.push_str(&format!("localIP = '{}'\n", escape_toml(&t.local_ip)));
        out.push_str(&format!("localPort = {}\n", t.local_port));
        out.push_str(&format!("remotePort = {}\n", t.remote_port));
        if let Some(cd) = &t.custom_domains {
            if !cd.is_empty() {
                out.push_str(&format!("customDomains = '{}'\n", escape_toml(cd)));
            }
        }
        if let Some(sd) = &t.subdomain {
            if !sd.is_empty() {
                out.push_str(&format!("subdomain = '{}'\n", escape_toml(sd)));
            }
        }
        if let Some(bl) = &t.bandwidth_limit {
            if !bl.is_empty() {
                out.push_str(&format!("bandwidthLimit = '{}'\n", escape_toml(bl)));
            }
        }
        // transport 子表
        if t.use_encryption.is_some() || t.use_compression.is_some() {
            out.push_str("\n[proxies.transport]\n");
            if let Some(ue) = t.use_encryption {
                out.push_str(&format!("useEncryption = {}\n", ue));
            }
            if let Some(uc) = t.use_compression {
                out.push_str(&format!("useCompression = {}\n", uc));
            }
        }
        out.push('\n');
    }
    Ok(out)
}

fn escape_toml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\'', "\\'")
}

// 兼容旧的 parse_toml / json_to_toml 命令
#[tauri::command]
fn parse_toml(content: String) -> Result<Value, String> {
    let value: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
    Ok(toml_to_json(&value))
}

fn toml_to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => serde_json::Number::from_f64(*f).map(Value::Number).unwrap_or(Value::Null),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), toml_to_json(v));
            }
            Value::Object(obj)
        }
    }
}

#[tauri::command]
fn json_to_toml(content: String) -> Result<String, String> {
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let toml_val = json_to_toml_value(&v);
    let table = match toml_val {
        toml::Value::Table(t) => t,
        _ => return Err("根节点必须是对象".to_string()),
    };
    toml::to_string(&toml::Value::Table(table)).map_err(|e| e.to_string())
}

fn json_to_toml_value(value: &Value) -> toml::Value {
    match value {
        Value::String(s) => toml::Value::String(s.clone()),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::Integer(0)
            }
        }
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Null => toml::Value::String(String::new()),
        Value::Array(arr) => toml::Value::Array(arr.iter().map(json_to_toml_value).collect()),
        Value::Object(obj) => {
            let mut map = toml::map::Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_toml_value(v));
            }
            toml::Value::Table(map)
        }
    }
}

// ============ 真实下载 FRP 核心 ============
#[tauri::command]
fn download_frp(
    app: tauri::AppHandle,
    version: String,
    os: String,
    arch: String,
) -> Result<DownloadResult, String> {
    let tag = if version.starts_with('v') { version.clone() } else { format!("v{version}") };
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    let file_name = format!("frp_{}_{}_{}.{}", tag.trim_start_matches('v'), os, arch, ext);

    // 镜像站列表，可配置多个，依次尝试
    let mirrors = vec![
        format!("https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://ghproxy.com/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://gh-proxy.com/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://mirror.ghproxy.com/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
    ];

    let dir = frp_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let archive_path = dir.join(&file_name);
    let extract_dir = dir.join(format!("frp_{}_{}_{}", tag.trim_start_matches('v'), os, arch));

    // 若已解压则直接返回
    let frpc_path = frpc_binary_path(&extract_dir, &os);
    if frpc_path.exists() {
        return Ok(DownloadResult { path: frpc_path.to_string_lossy().to_string(), version: tag });
    }

    emit_log_event(&app, "info", &format!("开始下载 FRP {} ...", tag));

    // 依次尝试镜像
    let mut last_err = String::new();
    let mut downloaded = false;
    for (i, url) in mirrors.iter().enumerate() {
        if i > 0 {
            emit_log_event(&app, "warn", &format!("主源下载失败，尝试镜像 {}", url));
        }
        match download_to_file(url, &archive_path) {
            Ok(_) => {
                downloaded = true;
                break;
            }
            Err(e) => {
                last_err = e;
                let _ = std::fs::remove_file(&archive_path);
            }
        }
    }
    if !downloaded {
        return Err(format!("下载失败: {last_err}"));
    }

    emit_log_event(&app, "info", "下载完成，开始解压...");

    // 解压
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    if ext == "zip" {
        extract_zip(&archive_path, &extract_dir)?;
    } else {
        extract_tar_gz(&archive_path, &extract_dir)?;
    }

    let _ = std::fs::remove_file(&archive_path);

    let frpc_path = frpc_binary_path(&extract_dir, &os);
    if !frpc_path.exists() {
        return Err("解压完成但未找到 frpc 可执行文件".to_string());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&frpc_path, std::fs::Permissions::from_mode(0o755));
    }

    emit_log_event(&app, "info", "FRP 核心下载完成");
    Ok(DownloadResult { path: frpc_path.to_string_lossy().to_string(), version: tag })
}

fn frpc_binary_path(extract_dir: &Path, os: &str) -> PathBuf {
    if os == "windows" {
        extract_dir.join("frpc.exe")
    } else {
        extract_dir.join("frpc")
    }
}

fn download_to_file(url: &str, dest: &Path) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    std::fs::write(dest, bytes).map_err(|e| e.to_string())
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.mangled_name().to_string_lossy().to_string();
        // 去掉顶层目录
        let rel = name.splitn(2, '/').nth(1).unwrap_or(&name);
        if rel.is_empty() {
            continue;
        }
        let out_path = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    let entries = tar.entries().map_err(|e| e.to_string())?;
    for entry in entries {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.to_string_lossy().to_string();
        let rel = path.splitn(2, '/').nth(1).unwrap_or(&path);
        if rel.is_empty() {
            continue;
        }
        let out_path = dest.join(rel);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        entry.unpack(&out_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ============ 真实启动 / 停止 frpc ============
#[tauri::command]
fn run_frp(app: tauri::AppHandle, state: State<'_, FrpState>, frpc_path: String, config_path: String) -> Result<FrpProcessInfo, String> {
    {
        let running = state.running.lock().map_err(|e| e.to_string())?;
        if *running {
            return Err("FRP 已在运行中".to_string());
        }
    }

    if !Path::new(&frpc_path).exists() {
        return Err("frpc 可执行文件不存在，请先在「核心下载」中下载".to_string());
    }
    if !Path::new(&config_path).exists() {
        return Err("配置文件不存在".to_string());
    }

    emit_log_event(&app, "info", &format!("启动 frpc: {} -c {}", frpc_path, config_path));

    let mut cmd = std::process::Command::new(&frpc_path);
    cmd.arg("-c").arg(&config_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| format!("启动失败: {e}"))?;
    let pid = child.id();

    // 读取 stdout/stderr 转发为日志
    if let Some(stdout) = child.stdout.take() {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let level = classify_log_level(&line);
                emit_log_event(&app_clone, level, &line);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                emit_log_event(&app_clone, "warn", &line);
            }
        });
    }

    // 后台监控子进程退出
    {
        let app_clone = app.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            if let Ok(s) = status {
                emit_log_event(&app_clone, "info", &format!("frpc 进程已退出，退出码: {:?}", s.code()));
            }
            let state = app_clone.state::<FrpState>();
            if let Ok(mut running) = state.running.lock() {
                *running = false;
            }
            if let Ok(mut c) = state.child.lock() {
                *c = None;
            }
            let _ = app_clone.emit("frp-status", &FrpProcessInfo { pid: 0, running: false });
        });
    }

    {
        let mut running = state.running.lock().map_err(|e| e.to_string())?;
        let mut child_pid = state.child.lock().map_err(|e| e.to_string())?;
        *running = true;
        *child_pid = Some(pid);
    }

    let info = FrpProcessInfo { pid, running: true };
    let _ = app.emit("frp-status", &info);
    Ok(info)
}

fn classify_log_level(line: &str) -> &'static str {
    let lower = line.to_lowercase();
    if lower.contains("error") || lower.contains("fatal") {
        "error"
    } else if lower.contains("warn") {
        "warn"
    } else if lower.contains("debug") {
        "debug"
    } else if lower.contains("trace") {
        "trace"
    } else {
        "info"
    }
}

#[tauri::command]
fn stop_frp(app: tauri::AppHandle, state: State<'_, FrpState>) -> Result<FrpProcessInfo, String> {
    let pid = {
        let child = state.child.lock().map_err(|e| e.to_string())?;
        *child
    };

    if let Some(pid) = pid {
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        #[cfg(not(windows))]
        {
            unsafe {
                libc_kill(pid);
            }
        }
    }

    {
        let mut running = state.running.lock().map_err(|e| e.to_string())?;
        let mut child = state.child.lock().map_err(|e| e.to_string())?;
        *running = false;
        *child = None;
    }

    emit_log_event(&app, "info", "frpc 已停止");
    let info = FrpProcessInfo { pid: 0, running: false };
    let _ = app.emit("frp-status", &info);
    Ok(info)
}

#[cfg(not(windows))]
unsafe fn libc_kill(pid: u32) {
    let _ = std::process::Command::new("kill")
        .arg(pid.to_string())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

#[tauri::command]
fn get_frp_status(state: State<'_, FrpState>) -> Result<FrpProcessInfo, String> {
    let running = state.running.lock().map_err(|e| e.to_string())?;
    let child = state.child.lock().map_err(|e| e.to_string())?;
    Ok(FrpProcessInfo { pid: child.unwrap_or(0), running: *running })
}

#[tauri::command]
fn emit_log(app: tauri::AppHandle, level: String, message: String) -> Result<(), String> {
    emit_log_event(&app, &level, &message);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(FrpState::default())
        .invoke_handler(tauri::generate_handler![
            get_app_data_dir,
            read_config,
            write_config,
            parse_toml,
            parse_toml_tunnels,
            json_to_toml,
            tunnels_to_toml,
            list_dir_files,
            download_frp,
            run_frp,
            stop_frp,
            get_frp_status,
            emit_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
