mod config;
mod core;
mod model;
mod providers;
mod runner;
mod settings;
mod store;
mod traffic;

use std::sync::Mutex;
use tauri::{Emitter, Manager};

pub struct RuntimeState {
    pub running: bool,
    pub pid: Option<u32>,
    pub profile_id: String,
    pub profile_name: String,
    pub config_path: String,
    pub web_port: u16,
    pub started_at: u64,
    pub gateways: Vec<traffic::Gateway>,
    pub monitor_stop: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pub snapshot: traffic::TrafficSnapshot,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            running: false,
            pid: None,
            profile_id: String::new(),
            profile_name: String::new(),
            config_path: String::new(),
            web_port: 0,
            started_at: 0,
            gateways: Vec::new(),
            monitor_stop: None,
            snapshot: traffic::TrafficSnapshot::idle(),
        }
    }
}

pub struct AppState {
    pub inner: Mutex<RuntimeState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: Mutex::new(RuntimeState::default()),
        }
    }
}

/// 统一的日志事件（前端「日志」页消费）
pub fn emit_log_event(app: &tauri::AppHandle, level: &str, message: &str) {
    emit_log_event_at(app, level, message, None);
}

/// 带时间戳的日志事件：frpc 自身日志已包含准确时间，避免被采集时刻覆盖
pub fn emit_log_event_at(
    app: &tauri::AppHandle,
    level: &str,
    message: &str,
    timestamp: Option<String>,
) {
    let payload = serde_json::json!({
        "level": level,
        "message": message,
        "timestamp": timestamp.unwrap_or_else(model::now_hms),
    });
    // 同步落盘一份到日志文件（可在设置中自定义日志目录）
    if let Some(ts) = payload.get("timestamp").and_then(|v| v.as_str()) {
        settings::append_log_file(app, level, ts, message);
    }
    let _ = app.emit("frp-log", &payload);
}

#[tauri::command]
fn emit_log(app: tauri::AppHandle, level: String, message: String) -> Result<(), String> {
    emit_log_event(&app, &level, &message);
    Ok(())
}

/// 在文件管理器中打开指定路径（不依赖 opener 插件的能力声明）
#[tauri::command]
fn reveal_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let target = if !path.trim().is_empty() && std::path::Path::new(&path).exists() {
        path
    } else {
        store::data_dir(&app)?.to_string_lossy().to_string()
    };

    #[cfg(windows)]
    {
        std::process::Command::new("explorer")
            .arg(&target)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开目录失败: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&target)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开目录失败: {e}"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&target)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("打开目录失败: {e}"))
    }
}

/// 显示并聚焦主窗口（托盘菜单 / 托盘双击）
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 退出程序：回收 frpc 与计量网关后结束进程
fn quit_app(app: &tauri::AppHandle) {
    runner::cleanup_runtime(app);
    app.exit(0);
}

fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .tooltip("NetTool")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => quit_app(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// 前端弹窗选择关闭方式后回调
#[tauri::command]
fn close_action(
    app: tauri::AppHandle,
    action: String,
    remember: bool,
) -> Result<(), String> {
    if remember && matches!(action.as_str(), "tray" | "quit") {
        let mut current = settings::load(&app);
        current.close_behavior = action.clone();
        settings::save(&app, &current)?;
    }
    match action.as_str() {
        "tray" => {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
            }
        }
        "quit" => quit_app(&app),
        _ => {}
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .setup(|app| {
            // 先锁定本次运行的数据/日志目录（设置中修改后需重启生效）
            settings::init_paths(app.handle());
            // 应用标识符改为 top.hn3.nettool 后，首次启动迁移旧目录数据
            store::migrate_legacy_data(app.handle());
            if let Err(e) = setup_tray(app.handle()) {
                eprintln!("托盘初始化失败: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            emit_log,
            reveal_path,
            store::get_app_data_dir,
            store::load_profiles,
            store::save_profiles,
            config::parse_toml_tunnels,
            config::preview_config,
            config::tunnels_to_toml,
            config::read_config,
            config::write_config,
            config::list_dir_files,
            core::detect_platform,
            core::get_installed_core,
            core::list_frp_releases,
            core::download_frp,
            runner::start_frpc,
            runner::stop_frpc,
            runner::get_frp_status,
            runner::get_traffic,
            runner::reset_traffic,
            runner::verify_core,
            runner::get_platform,
            runner::new_profile_id,
            providers::provider_list,
            providers::provider_accounts,
            providers::provider_login_key,
            providers::provider_login_password,
            providers::provider_login_code,
            providers::provider_auth_url,
            providers::provider_web_login_start,
            providers::provider_web_login_poll,
            providers::provider_web_login_cancel,
            providers::provider_sign,
            providers::provider_logout,
            providers::provider_user,
            providers::provider_tunnels,
            providers::provider_config,
            providers::provider_import,
            providers::provider_kick,
            providers::provider_toggle,
            providers::provider_captcha_info,
            providers::provider_decode_captcha,
            providers::provider_parse_config,
            settings::get_app_settings,
            settings::save_app_settings,
            settings::get_storage_info,
            settings::pick_directory,
            settings::apply_storage_settings,
            settings::get_autostart,
            settings::set_autostart,
            close_action,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                match settings::load(app).close_behavior.as_str() {
                    // 后台运行：隐藏窗口，frpc 继续跑
                    "tray" => {
                        api.prevent_close();
                        let _ = window.hide();
                        emit_log_event(app, "info", "已最小化到托盘，隧道保持运行");
                    }
                    // 直接退出：回收 frpc 后关闭
                    "quit" => {
                        runner::cleanup_runtime(app);
                    }
                    // 每次询问：交给前端弹窗决定
                    _ => {
                        api.prevent_close();
                        let _ = app.emit("app-close-request", ());
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


