//! 应用级设置：关闭行为、数据/日志目录、开机自启动。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use tauri::Manager;

// 启动时解析一次的生效路径：修改存储位置后需重启才切换，避免运行中数据分裂
static ACTIVE_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static ACTIVE_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    /// 关闭窗口时的行为：`ask` 每次询问 / `tray` 后台运行 / `quit` 直接退出
    pub close_behavior: String,
    /// 自定义数据目录（为空表示使用默认 app_data_dir）
    #[serde(default)]
    pub data_dir: Option<String>,
    /// 自定义日志目录（为空表示使用 `{数据目录}/logs`）
    #[serde(default)]
    pub log_dir: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_behavior: "ask".to_string(),
            data_dir: None,
            log_dir: None,
        }
    }
}

impl AppSettings {
    fn sanitize(mut self) -> Self {
        if !matches!(self.close_behavior.as_str(), "ask" | "tray" | "quit") {
            self.close_behavior = "ask".to_string();
        }
        self.data_dir = self
            .data_dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self.log_dir = self
            .log_dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        self
    }
}

/// 固定的应用配置目录（settings.json 始终留在这里，避免自举问题）
pub fn config_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok()
}

fn path(app: &tauri::AppHandle) -> Option<PathBuf> {
    config_dir(app).map(|d| d.join("settings.json"))
}

pub fn load(app: &tauri::AppHandle) -> AppSettings {
    let Some(path) = path(app) else {
        return AppSettings::default();
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return AppSettings::default();
    };
    serde_json::from_str::<AppSettings>(&text)
        .unwrap_or_default()
        .sanitize()
}

pub fn save(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let Some(path) = path(app) else {
        return Err("无法定位应用配置目录".to_string());
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

/// 默认数据目录：`%APPDATA%/top.hn3.nettool`
pub fn default_data_dir(app: &tauri::AppHandle) -> PathBuf {
    config_dir(app).unwrap_or_else(|| PathBuf::from("."))
}

/// 用户期望的数据目录（保存设置后即反映，重启生效）
pub fn desired_data_dir(app: &tauri::AppHandle) -> PathBuf {
    load(app)
        .data_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| default_data_dir(app))
}

/// 用户期望的日志目录
pub fn desired_log_dir(app: &tauri::AppHandle) -> PathBuf {
    load(app)
        .log_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| desired_data_dir(app).join("logs"))
}

/// 程序启动时调用一次：锁定本次运行使用的数据/日志目录
pub fn init_paths(app: &tauri::AppHandle) {
    let _ = ACTIVE_DATA_DIR.set(desired_data_dir(app));
    let _ = ACTIVE_LOG_DIR.set(desired_log_dir(app));
}

/// 本次运行生效的数据目录
pub fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    ACTIVE_DATA_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| desired_data_dir(app))
}

/// 本次运行生效的日志目录
pub fn log_dir(app: &tauri::AppHandle) -> PathBuf {
    ACTIVE_LOG_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| desired_log_dir(app))
}

/// 追加一行日志到日志文件（失败静默，不影响主流程）
pub fn append_log_file(app: &tauri::AppHandle, level: &str, timestamp: &str, message: &str) {
    let dir = log_dir(app);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let date = date_part(timestamp);
    let file = dir.join(format!("frpc-{date}.log"));
    let line = format!("[{timestamp}] [{}] {message}\n", level.to_uppercase());
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(file)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// `12:34:56` → 当天日期（YYYY-MM-DD）；其它情况返回 "unknown"
fn date_part(hms: &str) -> String {
    if !hms.contains(':') {
        return "unknown".to_string();
    }
    // 以系统当前日期为准
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 8 * 3600;
    let days = secs / 86_400;
    // 简化的 civil_from_days
    let z = days as i64 + 719_468;
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub default_data_dir: String,
    pub default_log_dir: String,
    pub data_dir: String,
    pub log_dir: String,
}

#[tauri::command]
pub fn get_app_settings(app: tauri::AppHandle) -> AppSettings {
    load(&app)
}

#[tauri::command]
pub fn save_app_settings(app: tauri::AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let settings = settings.clone().sanitize();
    save(&app, &settings)?;
    Ok(settings)
}

#[tauri::command]
pub fn get_storage_info(app: tauri::AppHandle) -> StorageInfo {
    let default_data = default_data_dir(&app);
    StorageInfo {
        default_data_dir: default_data.to_string_lossy().to_string(),
        default_log_dir: default_data.join("logs").to_string_lossy().to_string(),
        data_dir: desired_data_dir(&app).to_string_lossy().to_string(),
        log_dir: desired_log_dir(&app).to_string_lossy().to_string(),
    }
}

/// 选择文件夹（原生对话框）
#[tauri::command]
pub async fn pick_directory(default_path: Option<String>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = rfd::FileDialog::new();
        if let Some(p) = default_path.filter(|p| !p.trim().is_empty()) {
            if Path::new(&p).exists() {
                dialog = dialog.set_directory(p);
            }
        }
        Ok(dialog
            .pick_folder()
            .map(|p| p.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

fn copy_dir_merge(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_merge(&from, &to)?;
        } else if !to.exists() {
            let _ = std::fs::copy(&from, &to);
        }
    }
    Ok(())
}

/// 应用存储位置设置；`migrate` 为 true 时把现有文件复制到新目录
#[tauri::command]
pub async fn apply_storage_settings(
    app: tauri::AppHandle,
    data_dir: Option<String>,
    log_dir: Option<String>,
    migrate: bool,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut settings = load(&app);
        let old_data = crate::store::data_dir(&app).unwrap_or_else(|_| default_data_dir(&app));
        let old_log = crate::settings::log_dir(&app);

        let new_data = data_dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| default_data_dir(&app));
        let new_log = log_dir
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| new_data.join("logs"));

        // 简单校验：不能指向已存在的文件
        for dir in [&new_data, &new_log] {
            if dir.exists() && !dir.is_dir() {
                return Err(format!("目标路径不是文件夹：{}", dir.display()));
            }
        }

        if migrate {
            if old_data != new_data && old_data.exists() {
                copy_dir_merge(&old_data, &new_data).map_err(|e| format!("迁移数据失败: {e}"))?;
            }
            if old_log != new_log && old_log.exists() {
                copy_dir_merge(&old_log, &new_log).map_err(|e| format!("迁移日志失败: {e}"))?;
            }
        }
        std::fs::create_dir_all(&new_data).map_err(|e| format!("创建数据目录失败: {e}"))?;
        std::fs::create_dir_all(&new_log).map_err(|e| format!("创建日志目录失败: {e}"))?;

        settings.data_dir = if new_data == default_data_dir(&app) {
            None
        } else {
            Some(new_data.to_string_lossy().to_string())
        };
        let default_log = new_data.join("logs");
        settings.log_dir = if new_log == default_log {
            None
        } else {
            Some(new_log.to_string_lossy().to_string())
        };
        save(&app, &settings)?;
        Ok("存储位置已保存，重启应用后生效".to_string())
    })
    .await
    .map_err(|e| format!("任务调度失败: {e}"))?
}

// ============ 开机自启动（Windows 注册表 Run） ============

#[cfg(windows)]
fn autostart_key() -> &'static str {
    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run"
}

#[tauri::command]
pub fn get_autostart() -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("reg")
            .args(["query", autostart_key(), "/v", "NetTool"])
            .creation_flags(0x08000000)
            .output();
        return out.map(|o| o.status.success()).unwrap_or(false);
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[tauri::command]
pub fn set_autostart(enabled: bool) -> Result<String, String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        if enabled {
            let exe = std::env::current_exe().map_err(|e| e.to_string())?;
            let value = format!("\"{}\"", exe.to_string_lossy());
            let out = std::process::Command::new("reg")
                .args([
                    "add",
                    autostart_key(),
                    "/v",
                    "NetTool",
                    "/t",
                    "REG_SZ",
                    "/d",
                    &value,
                    "/f",
                ])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;
            if !out.status.success() {
                return Err(String::from_utf8_lossy(&out.stderr).to_string());
            }
            Ok("已开启开机自启动".to_string())
        } else {
            let out = std::process::Command::new("reg")
                .args(["delete", autostart_key(), "/v", "NetTool", "/f"])
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| e.to_string())?;
            // 值不存在时也视为成功
            let _ = out;
            Ok("已关闭开机自启动".to_string())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("当前平台暂不支持在应用内设置开机自启动".to_string())
    }
}


