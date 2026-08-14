use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};

#[derive(Default)]
pub struct FrpState {
    running: Mutex<bool>,
    process: Mutex<Option<u32>>,
}

#[derive(Debug, Serialize)]
pub struct FrpProcessInfo {
    pid: u32,
    running: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigFile {
    path: String,
    content: String,
}

/// 获取应用数据目录
#[tauri::command]
fn get_app_data_dir(app: tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir)
}

/// 读取 FRP 配置文件（TOML）
#[tauri::command]
fn read_config(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

/// 写入 FRP 配置文件
#[tauri::command]
fn write_config(path: String, content: String) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

/// 解析 TOML 配置为 JSON（用于前端展示）
#[tauri::command]
fn parse_toml(content: String) -> Result<Value, String> {
    let value: toml::Value = toml::from_str(&content).map_err(|e| e.to_string())?;
    let json = toml_to_json(&value);
    Ok(json)
}

fn toml_to_json(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(*f).map(Value::Number).unwrap_or(Value::Null)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(arr) => {
            Value::Array(arr.iter().map(toml_to_json).collect())
        }
        toml::Value::Table(map) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in map {
                obj.insert(k.clone(), toml_to_json(v));
            }
            Value::Object(obj)
        }
    }
}

/// 将 JSON 隧道配置转换为 TOML
#[tauri::command]
fn json_to_toml(content: String) -> Result<String, String> {
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let toml_val = json_to_toml_value(&v);
    let table = match toml_val {
        toml::Value::Table(t) => t,
        _ => return Err("根节点必须是对象".to_string()),
    };
    let rendered = toml::to_string(&toml::Value::Table(table)).map_err(|e| e.to_string())?;
    Ok(rendered)
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
        Value::Array(arr) => {
            toml::Value::Array(arr.iter().map(json_to_toml_value).collect())
        }
        Value::Object(obj) => {
            let mut map = toml::map::Map::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_toml_value(v));
            }
            toml::Value::Table(map)
        }
    }
}

/// 列出目录下的文件
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

/// 启动 FRP 进程（模拟，返回 PID 占位）
#[tauri::command]
fn start_frp(app: tauri::AppHandle, state: State<'_, FrpState>) -> Result<FrpProcessInfo, String> {
    let mut running = state.running.lock().map_err(|e| e.to_string())?;
    let mut process = state.process.lock().map_err(|e| e.to_string())?;
    if *running {
        return Err("FRP 已在运行中".to_string());
    }
    let pid = std::process::id();
    *process = Some(pid);
    *running = true;

    let info = FrpProcessInfo { pid, running: true };
    let _ = app.emit("frp-status", &info);
    Ok(info)
}

/// 停止 FRP 进程
#[tauri::command]
fn stop_frp(app: tauri::AppHandle, state: State<'_, FrpState>) -> Result<FrpProcessInfo, String> {
    let mut running = state.running.lock().map_err(|e| e.to_string())?;
    let mut process = state.process.lock().map_err(|e| e.to_string())?;
    *running = false;
    *process = None;

    let info = FrpProcessInfo { pid: 0, running: false };
    let _ = app.emit("frp-status", &info);
    Ok(info)
}

/// 获取当前 FRP 运行状态
#[tauri::command]
fn get_frp_status(state: State<'_, FrpState>) -> Result<FrpProcessInfo, String> {
    let running = state.running.lock().map_err(|e| e.to_string())?;
    let process = state.process.lock().map_err(|e| e.to_string())?;
    Ok(FrpProcessInfo {
        pid: process.unwrap_or(0),
        running: *running,
    })
}

/// 发送一条日志（用于日志模块演示）
#[tauri::command]
fn emit_log(app: tauri::AppHandle, level: String, message: String) -> Result<(), String> {
    let payload = serde_json::json!({
        "level": level,
        "message": message,
        "timestamp": chrono_now(),
    });
    let _ = app.emit("frp-log", &payload);
    Ok(())
}

fn chrono_now() -> String {
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
            json_to_toml,
            list_dir_files,
            start_frp,
            stop_frp,
            get_frp_status,
            emit_log,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
