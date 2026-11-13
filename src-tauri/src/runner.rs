//! frpc 进程编排：按配置生成 frpc.toml、启动计量网关、拉起 frpc、秒级采样真实流量。

use crate::config::render_config;
use crate::core::{detect_arch, detect_os, installed_core};
use crate::model::{now_ms, Profile};
use crate::store::{gen_id, normalize_profile, profile_dir, read_store};
use crate::traffic::{fetch_proxy_status, Gateway, TrafficSnapshot, TunnelTraffic};
use crate::{emit_log_event, emit_log_event_at, AppState, RuntimeState};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{Emitter, Manager, State};

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FrpProcessInfo {
    pub pid: u32,
    pub running: bool,
    pub profile_id: String,
    pub profile_name: String,
    pub config_path: String,
    pub web_port: u16,
    pub started_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayBinding {
    pub tunnel_id: String,
    pub name: String,
    pub port: u16,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartResult {
    pub pid: u32,
    pub profile_id: String,
    pub profile_name: String,
    pub config_path: String,
    pub web_port: u16,
    pub gateways: Vec<GatewayBinding>,
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0u16))
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
        .unwrap_or(7401)
}

fn classify_log_level(line: &str) -> &'static str {
    let lower = line.to_lowercase();
    if lower.contains("[e]") || lower.contains("error") || lower.contains("fatal") {
        "error"
    } else if lower.contains("[w]") || lower.contains("warn") {
        "warn"
    } else if lower.contains("[d]") || lower.contains("debug") {
        "debug"
    } else if lower.contains("[t]") || lower.contains("trace") {
        "trace"
    } else {
        "info"
    }
}

/// 去掉 ANSI 颜色/控制序列（frpc 的日志默认带颜色码）
fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                // CSI 序列以 0x40..=0x7E 的字符结束
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
                continue;
            }
            // 其它 ESC 序列直接丢弃
            continue;
        }
        if c == '\r' {
            continue;
        }
        out.push(c);
    }
    out
}

/// 解析 frpc 日志行：`2026-11-08 23:53:29.978 [I] [client/admin_api.go:168] Http request ...`
///
/// 返回（时间戳 HH:MM:SS、级别、正文）
fn parse_frpc_log(raw: &str) -> (Option<String>, &'static str, String) {
    let cleaned = strip_ansi(raw);
    let line = cleaned.trim();
    if line.is_empty() {
        return (None, "info", String::new());
    }

    // 1. 提取时间戳（前 19 个字符形如 YYYY-MM-DD HH:MM:SS）
    let mut ts: Option<String> = None;
    let mut rest = line;
    if line.len() >= 19 {
        let head = &line[..19];
        let b = head.as_bytes();
        let is_ts = b[4] == b'-'
            && b[7] == b'-'
            && b[10] == b' '
            && b[13] == b':'
            && b[16] == b':'
            && b[..4].iter().all(u8::is_ascii_digit)
            && b[5..7].iter().all(u8::is_ascii_digit)
            && b[8..10].iter().all(u8::is_ascii_digit)
            && b[11..13].iter().all(u8::is_ascii_digit)
            && b[14..16].iter().all(u8::is_ascii_digit)
            && b[17..19].iter().all(u8::is_ascii_digit);
        if is_ts {
            ts = Some(head[11..].to_string());
            rest = line[19..].trim_start();
            // 跳过毫秒部分 .123
            if let Some(after_dot) = rest.strip_prefix('.') {
                let digits = after_dot
                    .char_indices()
                    .take_while(|(_, c)| c.is_ascii_digit())
                    .count();
                rest = after_dot[digits..].trim_start();
            }
        }
    }

    // 2. 提取级别标记 [I] / [W] / [E] / [D] / [T]
    let (level, rest): (&'static str, &str) =
        if rest.len() >= 3 && rest.starts_with('[') && rest.as_bytes()[2] == b']' {
            let lv = match rest.as_bytes()[1].to_ascii_uppercase() {
                b'W' => "warn",
                b'E' => "error",
                b'D' => "debug",
                b'T' => "trace",
                _ => "info",
            };
            (lv, rest[3..].trim_start())
        } else {
            (classify_log_level(rest), rest)
        };

    let message = if rest.is_empty() {
        line.to_string()
    } else {
        rest.to_string()
    };
    (ts, level, message)
}

fn kill_process_tree(pid: u32) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .creation_flags(0x08000000)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("kill")
            .arg("-9")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

/// 停止一切运行态资源（进程 + 计量网关 + 采样线程）
pub fn cleanup_runtime(app: &tauri::AppHandle) {
    let state = app.state::<AppState>();
    let (pid, gateways) = {
        let mut guard = match state.inner.lock() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        guard.running = false;
        if let Some(stop) = guard.monitor_stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        let pid = guard.pid.take();
        let gateways = std::mem::take(&mut guard.gateways);
        guard.snapshot.running = false;
        guard.snapshot.up_speed = 0;
        guard.snapshot.down_speed = 0;
        (pid, gateways)
    };
    for gw in gateways {
        gw.shutdown();
    }
    if let Some(pid) = pid {
        kill_process_tree(pid);
    }
    let info = FrpProcessInfo::default();
    let _ = app.emit("frp-status", &info);
}

fn current_info(state: &RuntimeState) -> FrpProcessInfo {
    FrpProcessInfo {
        pid: state.pid.unwrap_or(0),
        running: state.running,
        profile_id: state.profile_id.clone(),
        profile_name: state.profile_name.clone(),
        config_path: state.config_path.clone(),
        web_port: state.web_port,
        started_at: state.started_at,
    }
}

#[tauri::command]
pub fn get_frp_status(state: State<'_, AppState>) -> Result<FrpProcessInfo, String> {
    let guard = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(current_info(&guard))
}

#[tauri::command]
pub fn get_traffic(state: State<'_, AppState>) -> Result<TrafficSnapshot, String> {
    let guard = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(guard.snapshot.clone())
}

#[tauri::command]
pub fn reset_traffic(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
    for gw in guard.gateways.iter() {
        gw.stat.reset();
    }
    guard.snapshot.up_total = 0;
    guard.snapshot.down_total = 0;
    guard.snapshot.up_speed = 0;
    guard.snapshot.down_speed = 0;
    for t in guard.snapshot.tunnels.iter_mut() {
        t.up_total = 0;
        t.down_total = 0;
        t.conn_total = 0;
    }
    guard.started_at = now_ms();
    guard.snapshot.started_at = guard.started_at;
    Ok(())
}

/// 启动 frpc
///
/// - `profile`：前端传入的当前配置（优先使用，避免与落盘存在时序差）
/// - `profile_id`：未传 profile 时按 id 从 profiles.json 读取
#[tauri::command]
pub async fn start_frpc(
    app: tauri::AppHandle,
    profile: Option<Profile>,
    profile_id: Option<String>,
    frpc_path: Option<String>,
) -> Result<StartResult, String> {
    let worker = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let state = worker.state::<AppState>();
        start_frpc_inner(&worker, &state, profile, profile_id, frpc_path)
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

fn start_frpc_inner(
    app: &tauri::AppHandle,
    state: &AppState,
    profile: Option<Profile>,
    profile_id: Option<String>,
    frpc_path: Option<String>,
) -> Result<StartResult, String> {
    {
        let guard = state.inner.lock().map_err(|e| e.to_string())?;
        if guard.running {
            return Err("frpc 已在运行中，请先断开连接".to_string());
        }
    }

    let profile = match profile.filter(|p| !p.id.trim().is_empty()) {
        Some(p) => normalize_profile(p),
        None => {
            let store = read_store(app);
            match profile_id.as_ref().filter(|s| !s.is_empty()) {
                Some(id) => store.find(id).cloned(),
                None => store.active().cloned(),
            }
            .ok_or_else(|| "未找到可用的配置，请先在「配置管理」中创建".to_string())?
        }
    };

    if profile.server_addr.trim().is_empty() {
        return Err("服务器地址 (serverAddr) 不能为空".to_string());
    }
    if profile.server_port == 0 {
        return Err("服务器端口 (serverPort) 不能为 0".to_string());
    }
    let enabled = profile.enabled_tunnels();
    if enabled.is_empty() {
        return Err("当前配置没有启用任何隧道".to_string());
    }
    for t in &enabled {
        if t.name.trim().is_empty() {
            return Err("存在未命名的隧道，请先补全隧道名称".to_string());
        }
        if t.local_port == 0 {
            return Err(format!("隧道「{}」的本地端口未填写", t.name));
        }
        if matches!(t.proxy_type.as_str(), "http" | "https" | "tcpmux") {
            let has_domain = t
                .custom_domains
                .as_deref()
                .map(str::trim)
                .is_some_and(|s| !s.is_empty())
                || t.subdomain
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|s| !s.is_empty());
            if !has_domain {
                return Err(format!(
                    "隧道「{}」为 {} 类型，需要填写自定义域名或子域名",
                    t.name, t.proxy_type
                ));
            }
        }
    }

    // 定位 frpc 可执行文件
    let bin = match frpc_path.filter(|p| !p.trim().is_empty()) {
        Some(p) => {
            if !Path::new(&p).exists() {
                return Err(format!("frpc 不存在: {p}"));
            }
            p
        }
        None => installed_core(app)
            .map(|c| c.path)
            .ok_or_else(|| "尚未安装 frpc 核心，请先到「核心下载」安装".to_string())?,
    };

    // 1. 启动计量网关（TCP 类隧道才能统计方向流量）
    let mut gateways: Vec<Gateway> = Vec::new();
    let mut ports: HashMap<String, u16> = HashMap::new();
    let mut bindings: Vec<GatewayBinding> = Vec::new();
    for t in &enabled {
        if !t.is_tcp_based() {
            emit_log_event(
                app,
                "warn",
                &format!(
                    "隧道「{}」类型为 {}，暂不支持字节级流量统计",
                    t.name, t.proxy_type
                ),
            );
            continue;
        }
        match Gateway::start(t) {
            Ok(gw) => {
                ports.insert(t.id.clone(), gw.port);
                bindings.push(GatewayBinding {
                    tunnel_id: t.id.clone(),
                    name: t.name.clone(),
                    port: gw.port,
                    target: gw.target.clone(),
                });
                gateways.push(gw);
            }
            Err(e) => {
                for gw in &gateways {
                    gw.shutdown();
                }
                return Err(format!("隧道「{}」计量网关启动失败: {e}", t.name));
            }
        }
    }

    // 2. 生成配置并落盘（每套配置独立目录，多配置互不覆盖）
    let web_port = free_port();
    let toml = render_config(&profile, &ports, web_port);
    let dir = profile_dir(app, &profile.id)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let config_path = dir.join("frpc.toml");
    std::fs::write(&config_path, &toml).map_err(|e| e.to_string())?;
    let config_path_str = config_path.to_string_lossy().to_string();

    emit_log_event(
        app,
        "info",
        &format!(
            "启动 frpc：{} -c {}（{} 条隧道，管理接口 {}）",
            bin,
            config_path_str,
            enabled.len(),
            web_port
        ),
    );

    // 3. 拉起进程
    let mut cmd = std::process::Command::new(&bin);
    cmd.arg("-c").arg(&config_path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|e| {
        for gw in &gateways {
            gw.shutdown();
        }
        format!("启动 frpc 失败: {e}")
    })?;
    let pid = child.id();

    if let Some(stdout) = child.stdout.take() {
        let app_clone = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                let (ts, level, message) = parse_frpc_log(&line);
                if message.is_empty() {
                    continue;
                }
                emit_log_event_at(&app_clone, level, &message, ts);
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let app_clone = app.clone();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let (ts, level, message) = parse_frpc_log(&line);
                if message.is_empty() {
                    continue;
                }
                let level = if level == "info" { "warn" } else { level };
                emit_log_event_at(&app_clone, level, &message, ts);
            }
        });
    }

    // 4. 记录运行态
    let started_at = now_ms();
    let monitor_stop = Arc::new(AtomicBool::new(false));
    {
        let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
        guard.running = true;
        guard.pid = Some(pid);
        guard.profile_id = profile.id.clone();
        guard.profile_name = profile.name.clone();
        guard.config_path = config_path_str.clone();
        guard.web_port = web_port;
        guard.started_at = started_at;
        guard.gateways = gateways;
        guard.monitor_stop = Some(monitor_stop.clone());
        guard.snapshot = TrafficSnapshot {
            running: true,
            profile_id: profile.id.clone(),
            profile_name: profile.name.clone(),
            started_at,
            updated_at: started_at,
            ..Default::default()
        };
    }

    let info = {
        let guard = state.inner.lock().map_err(|e| e.to_string())?;
        current_info(&guard)
    };
    let _ = app.emit("frp-status", &info);

    // 5. 子进程退出监视
    {
        let app_clone = app.clone();
        thread::spawn(move || {
            let status = child.wait();
            if let Ok(s) = status {
                emit_log_event(
                    &app_clone,
                    "info",
                    &format!("frpc 进程已退出（退出码 {:?}）", s.code()),
                );
            }
            cleanup_runtime(&app_clone);
        });
    }

    // 6. 流量采样线程
    start_monitor(
        app.clone(),
        monitor_stop,
        profile.id.clone(),
        profile.name.clone(),
        web_port,
        started_at,
    );

    Ok(StartResult {
        pid,
        profile_id: profile.id,
        profile_name: profile.name,
        config_path: config_path_str,
        web_port,
        gateways: bindings,
    })
}

fn start_monitor(
    app: tauri::AppHandle,
    stop: Arc<AtomicBool>,
    profile_id: String,
    profile_name: String,
    web_port: u16,
    started_at: u64,
) {
    thread::spawn(move || {
        let state = app.state::<AppState>();
        let mut prev: HashMap<String, (u64, u64)> = HashMap::new();
        let mut tick: u32 = 0;
        let mut status_map: HashMap<String, crate::traffic::ProxyStatus> = HashMap::new();

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            let gateways: Vec<(
                String,
                String,
                String,
                String,
                u16,
                Arc<crate::traffic::TunnelStat>,
            )> = {
                match state.inner.lock() {
                    Ok(g) => g
                        .gateways
                        .iter()
                        .map(|gw| {
                            (
                                gw.tunnel_id.clone(),
                                gw.tunnel_name.clone(),
                                gw.proxy_type.clone(),
                                gw.target.clone(),
                                gw.port,
                                gw.stat.clone(),
                            )
                        })
                        .collect(),
                    Err(_) => break,
                }
            };

            // 每 2 秒读取一次 frpc 管理接口，拿到真实代理状态
            if tick % 2 == 0 {
                status_map = fetch_proxy_status(web_port);
            }
            tick = tick.wrapping_add(1);

            let mut tunnels: Vec<TunnelTraffic> = Vec::new();
            let (mut up_total, mut down_total) = (0u64, 0u64);
            let (mut up_speed, mut down_speed) = (0u64, 0u64);
            let mut conns_total = 0i64;

            for (id, name, ptype, target, port, stat) in gateways {
                let up = stat.up.load(Ordering::Relaxed);
                let down = stat.down.load(Ordering::Relaxed);
                let (pu, pd) = prev.get(&id).copied().unwrap_or((up, down));
                let up_sp = up.saturating_sub(pu);
                let down_sp = down.saturating_sub(pd);
                prev.insert(id.clone(), (up, down));

                let status = status_map.get(&name);
                let conns = stat.conns.load(Ordering::Relaxed);

                up_total += up;
                down_total += down;
                up_speed += up_sp;
                down_speed += down_sp;
                conns_total += conns;

                tunnels.push(TunnelTraffic {
                    id,
                    name,
                    proxy_type: ptype,
                    target,
                    gateway_port: port,
                    up_speed: up_sp,
                    down_speed: down_sp,
                    up_total: up,
                    down_total: down,
                    conns,
                    conn_total: stat.conn_total.load(Ordering::Relaxed),
                    status: status.map(|s| s.status.clone()).unwrap_or_default(),
                    err: status.map(|s| s.err.clone()).unwrap_or_default(),
                    remote_addr: status.map(|s| s.remote_addr.clone()).unwrap_or_default(),
                });
            }

            let snapshot = {
                match state.inner.lock() {
                    Ok(mut g) => {
                        if !g.running {
                            break;
                        }
                        g.snapshot = TrafficSnapshot {
                            running: true,
                            up_speed,
                            down_speed,
                            up_total,
                            down_total,
                            conns: conns_total,
                            profile_id: profile_id.clone(),
                            profile_name: profile_name.clone(),
                            started_at,
                            tunnels,
                            updated_at: now_ms(),
                        };
                        g.snapshot.clone()
                    }
                    Err(_) => break,
                }
            };

            let _ = app.emit("frp-traffic", &snapshot);

            for _ in 0..5 {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                thread::sleep(Duration::from_millis(200));
            }
        }
    });
}

#[tauri::command]
pub async fn stop_frpc(app: tauri::AppHandle) -> Result<FrpProcessInfo, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        {
            let guard = state.inner.lock().map_err(|e| e.to_string())?;
            if !guard.running {
                return Ok(current_info(&guard));
            }
        }
        cleanup_runtime(&app);
        emit_log_event(&app, "info", "frpc 已停止");
        Ok(FrpProcessInfo::default())
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

/// 校验核心可用性，返回真实版本号
#[tauri::command]
pub async fn verify_core(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || verify_core_blocking(path))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

fn verify_core_blocking(path: String) -> Result<String, String> {
    if !Path::new(&path).exists() {
        return Err("文件不存在".to_string());
    }
    let mut cmd = std::process::Command::new(&path);
    cmd.arg("-v");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().map_err(|e| format!("无法执行 frpc: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        return Err("frpc 未返回版本信息".to_string());
    }
    Ok(text)
}

/// 平台信息（供前端展示当前运行环境）
#[tauri::command]
pub fn get_platform() -> (String, String) {
    (detect_os(), detect_arch())
}

/// 为新增配置生成 id（前端新建时使用，保证与后端一致）
#[tauri::command]
pub fn new_profile_id() -> String {
    gen_id()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_ansi_and_parses_frpc_log() {
        let raw = "\u{1b}[0m \u{1b}[1;34m2026-11-08 23:53:29.978\u{1b}[0m [I] [client/admin_api.go:168] Http request [/api/status]";
        let (ts, level, msg) = parse_frpc_log(raw);
        assert_eq!(ts.as_deref(), Some("23:53:29"));
        assert_eq!(level, "info");
        assert_eq!(msg, "[client/admin_api.go:168] Http request [/api/status]");
        assert!(!msg.contains('\u{1b}'));
    }

    #[test]
    fn parses_warn_and_error_levels() {
        let (_, level, msg) =
            parse_frpc_log("2026-11-08 10:00:00.001 [W] [client/service.go:120] connect failed");
        assert_eq!(level, "warn");
        assert!(msg.contains("connect failed"));

        let (_, level, _) = parse_frpc_log("2026-11-08 10:00:00.002 [E] login to server failed");
        assert_eq!(level, "error");
    }

    #[test]
    fn handles_plain_lines_without_timestamp() {
        let (ts, level, msg) = parse_frpc_log("start proxy success");
        assert!(ts.is_none());
        assert_eq!(level, "info");
        assert_eq!(msg, "start proxy success");

        let (_, level, _) = parse_frpc_log("WARN something odd");
        assert_eq!(level, "warn");
    }
}
