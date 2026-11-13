use crate::model::{now_ms, Profile, ProfileStore};
use std::path::PathBuf;

/// 应用数据目录（用户可在设置中自定义，默认 `%APPDATA%/top.hn3.nettool`）
pub fn data_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(crate::settings::data_dir(app))
}

/// 旧的 bundle identifier（改名 top.hn3.nettool 之前）
const LEGACY_IDENTIFIER: &str = "com.jlhan.nettool";

/// 首次以新标识符启动时，把旧目录的配置/核心/凭据迁移过来
pub fn migrate_legacy_data(app: &tauri::AppHandle) {
    let Ok(new_dir) = data_dir(app) else {
        return;
    };
    if new_dir.join("profiles.json").exists() {
        return;
    }
    let Some(parent) = new_dir.parent() else {
        return;
    };
    let old_dir = parent.join(LEGACY_IDENTIFIER);
    if old_dir == new_dir || !old_dir.exists() || !old_dir.join("profiles.json").exists() {
        return;
    }
    let _ = copy_dir(&old_dir, &new_dir);
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)?.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else if !to.exists() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn store_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("profiles.json"))
}

pub fn core_meta_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("core.json"))
}

pub fn frp_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(data_dir(app)?.join("frp"))
}

/// 每套配置独立的运行目录（多配置互不干扰）
pub fn profile_dir(app: &tauri::AppHandle, profile_id: &str) -> Result<PathBuf, String> {
    let safe: String = profile_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.is_empty() { "default".to_string() } else { safe };
    Ok(data_dir(app)?.join("profiles").join(safe))
}

pub fn read_store(app: &tauri::AppHandle) -> ProfileStore {
    let Ok(path) = store_path(app) else {
        return ProfileStore::default();
    };
    let Ok(text) = std::fs::read_to_string(&path) else {
        return ProfileStore::default();
    };
    serde_json::from_str::<ProfileStore>(&text).unwrap_or_default()
}

pub fn write_store(app: &tauri::AppHandle, store: &ProfileStore) -> Result<(), String> {
    let path = store_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

/// 规范化配置：补齐 id / 时间戳 / 默认值，保证落盘数据自洽
pub fn normalize_profile(mut p: Profile) -> Profile {
    if p.id.trim().is_empty() {
        p.id = gen_id();
    }
    if p.name.trim().is_empty() {
        p.name = "未命名配置".to_string();
    }
    if p.server_port == 0 {
        p.server_port = 7000;
    }
    if p.transport.is_empty() {
        p.transport = "tcp".to_string();
    }
    if p.log_level.is_empty() {
        p.log_level = "info".to_string();
    }
    for t in p.tunnels.iter_mut() {
        if t.id.trim().is_empty() {
            t.id = gen_id();
        }
        if t.local_ip.trim().is_empty() {
            t.local_ip = "127.0.0.1".to_string();
        }
        if t.proxy_type.is_empty() {
            t.proxy_type = "tcp".to_string();
        }
    }
    let now = now_ms();
    if p.created_at == 0 {
        p.created_at = now;
    }
    p.updated_at = now;
    p
}

pub fn gen_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand = (nanos % 0xffff_ffff) as u32;
    format!("{:x}{:04x}", nanos / 1000, rand & 0xffff)
}

// ============ Tauri 命令 ============

#[tauri::command]
pub fn load_profiles(app: tauri::AppHandle) -> Result<ProfileStore, String> {
    Ok(read_store(&app))
}

#[tauri::command]
pub fn save_profiles(app: tauri::AppHandle, store: ProfileStore) -> Result<ProfileStore, String> {
    let mut data = store;
    data.profiles = data.profiles.into_iter().map(normalize_profile).collect();
    if data.active_id.is_empty() {
        if let Some(first) = data.profiles.first() {
            data.active_id = first.id.clone();
        }
    }
    write_store(&app, &data)?;
    Ok(data)
}

#[tauri::command]
pub fn get_app_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    Ok(data_dir(&app)?.to_string_lossy().to_string())
}

