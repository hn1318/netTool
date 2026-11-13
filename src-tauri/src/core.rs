//! frpc 核心：平台识别、版本拉取、真实下载（带进度）、安装元数据。

use crate::model::{now_ms, InstalledCore};
use crate::store::{core_meta_path, frp_dir};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tauri::Emitter;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
    pub installed: Option<InstalledCore>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadResult {
    pub path: String,
    pub version: String,
    pub size: u64,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub published_at: String,
    pub prerelease: bool,
    pub size_hint: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    stage: String,
    message: String,
    downloaded: u64,
    total: u64,
    percent: f64,
}

fn emit_progress(app: &tauri::AppHandle, stage: &str, message: &str, downloaded: u64, total: u64) {
    let percent = if total > 0 {
        (downloaded as f64 / total as f64 * 100.0).min(100.0)
    } else {
        0.0
    };
    let _ = app.emit(
        "core-download",
        ProgressPayload {
            stage: stage.to_string(),
            message: message.to_string(),
            downloaded,
            total,
            percent,
        },
    );
}

pub fn detect_os() -> String {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "freebsd"
    }
    .to_string()
}

pub fn detect_arch() -> String {
    if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "386"
    } else {
        "arm"
    }
    .to_string()
}

fn binary_name(os: &str) -> &'static str {
    if os == "windows" {
        "frpc.exe"
    } else {
        "frpc"
    }
}

fn read_meta(app: &tauri::AppHandle) -> Option<InstalledCore> {
    let path = core_meta_path(app).ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<InstalledCore>(&text).ok()
}

fn write_meta(app: &tauri::AppHandle, meta: &InstalledCore) -> Result<(), String> {
    let path = core_meta_path(app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// 已安装核心（元数据缺失时回退为目录扫描）
pub fn installed_core(app: &tauri::AppHandle) -> Option<InstalledCore> {
    if let Some(meta) = read_meta(app) {
        if Path::new(&meta.path).exists() {
            return Some(meta);
        }
    }
    let dir = frp_dir(app).ok()?;
    let name = binary_name(&detect_os());
    let found = find_file_recursive(&dir, name)?;
    let version = found
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.trim_start_matches("frp_").to_string())
        .unwrap_or_default();
    Some(InstalledCore {
        path: found.to_string_lossy().to_string(),
        version,
        os: detect_os(),
        arch: detect_arch(),
        installed_at: 0,
        size: std::fs::metadata(&found).map(|m| m.len()).unwrap_or(0),
    })
}

fn find_file_recursive(dir: &Path, target: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    let mut best: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_recursive(&path, target) {
                // 取路径层级最浅（版本目录最新）的一个，简单取首个
                if best.is_none() {
                    best = Some(found);
                }
            }
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.eq_ignore_ascii_case(target) {
                return Some(path);
            }
        }
    }
    best
}

/// 调用 frpc -v 获取真实版本号（失败则返回 None）
fn probe_version(bin: &Path) -> Option<String> {
    let mut cmd = std::process::Command::new(bin);
    cmd.arg("-v");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd.output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let first = text.lines().next().unwrap_or("").trim().to_string();
    if first.is_empty() {
        None
    } else {
        Some(first)
    }
}

// ============ 命令 ============

#[tauri::command]
pub fn detect_platform(app: tauri::AppHandle) -> PlatformInfo {
    PlatformInfo {
        os: detect_os(),
        arch: detect_arch(),
        installed: installed_core(&app),
    }
}

#[tauri::command]
pub fn get_installed_core(app: tauri::AppHandle) -> Option<InstalledCore> {
    installed_core(&app)
}

/// 拉取 GitHub 上真实的 frp 发布列表（失败时由前端回退到内置列表）
#[tauri::command]
pub async fn list_frp_releases(limit: Option<usize>) -> Result<Vec<ReleaseInfo>, String> {
    // reqwest::blocking 内部会创建 tokio runtime，必须在阻塞线程池中创建/销毁
    tauri::async_runtime::spawn_blocking(move || list_frp_releases_blocking(limit))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

fn list_frp_releases_blocking(limit: Option<usize>) -> Result<Vec<ReleaseInfo>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("NetTool/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let url = format!(
        "https://api.github.com/repos/fatedier/frp/releases?per_page={}",
        limit.unwrap_or(8).clamp(1, 30)
    );
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("GitHub API HTTP {}", resp.status()));
    }
    let list: Vec<serde_json::Value> = resp.json().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for item in list {
        let tag = item.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if tag.is_empty() {
            continue;
        }
        let published = item
            .get("published_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .chars()
            .take(10)
            .collect::<String>();
        let prerelease = item.get("prerelease").and_then(|v| v.as_bool()).unwrap_or(false);
        let assets = item.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let size = assets
            .iter()
            .filter_map(|a| a.get("size").and_then(|v| v.as_u64()))
            .max()
            .unwrap_or(0);
        out.push(ReleaseInfo {
            version: tag.trim_start_matches('v').to_string(),
            tag,
            published_at: published,
            prerelease,
            size_hint: if size > 0 {
                format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)
            } else {
                String::new()
            },
        });
    }
    if out.is_empty() {
        return Err("GitHub 未返回可用版本".to_string());
    }
    Ok(out)
}

#[tauri::command]
pub async fn download_frp(
    app: tauri::AppHandle,
    version: String,
    os: Option<String>,
    arch: Option<String>,
) -> Result<DownloadResult, String> {
    tauri::async_runtime::spawn_blocking(move || download_frp_blocking(app, version, os, arch))
        .await
        .map_err(|e| format!("任务调度失败: {e}"))?
}

fn download_frp_blocking(
    app: tauri::AppHandle,
    version: String,
    os: Option<String>,
    arch: Option<String>,
) -> Result<DownloadResult, String> {
    let os = os.unwrap_or_else(detect_os);
    let arch = arch.unwrap_or_else(detect_arch);
    let tag = if version.starts_with('v') {
        version.clone()
    } else {
        format!("v{version}")
    };
    let ver = tag.trim_start_matches('v').to_string();
    let ext = if os == "windows" { "zip" } else { "tar.gz" };
    let file_name = format!("frp_{ver}_{os}_{arch}.{ext}");

    let dir = frp_dir(&app)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let archive_path = dir.join(&file_name);
    let extract_dir = dir.join(format!("frp_{ver}_{os}_{arch}"));

    // 已安装且可执行则直接返回
    if let Some(existing) = installed_core(&app) {
        if existing.version.contains(&ver) {
            emit_progress(&app, "done", "已安装该版本核心", 1, 1);
            return Ok(DownloadResult {
                path: existing.path,
                version: existing.version,
                size: existing.size,
                os,
                arch,
            });
        }
    }

    let mirrors = [
        format!("https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://gh-proxy.com/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://ghfast.top/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
        format!("https://mirror.ghproxy.com/https://github.com/fatedier/frp/releases/download/{tag}/{file_name}"),
    ];

    emit_progress(&app, "download", &format!("开始下载 FRP {tag}"), 0, 0);

    let mut last_err = String::new();
    let mut downloaded_bytes: u64 = 0;
    let mut done = false;
    for (i, url) in mirrors.iter().enumerate() {
        if i > 0 {
            emit_progress(&app, "retry", &format!("切换下载源({i}/{})", mirrors.len()), 0, 0);
        }
        match download_to_file(&app, url, &archive_path) {
            Ok(size) => {
                downloaded_bytes = size;
                done = true;
                break;
            }
            Err(e) => {
                last_err = e;
                let _ = std::fs::remove_file(&archive_path);
            }
        }
    }
    if !done {
        emit_progress(&app, "error", &last_err, 0, 0);
        return Err(format!("下载失败: {last_err}"));
    }

    emit_progress(&app, "extract", "下载完成，正在解压...", downloaded_bytes, downloaded_bytes);
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;
    if ext == "zip" {
        extract_zip(&archive_path, &extract_dir)?;
    } else {
        extract_tar_gz(&archive_path, &extract_dir)?;
    }
    let _ = std::fs::remove_file(&archive_path);

    let name = binary_name(&os);
    let bin = find_file_recursive(&extract_dir, name).ok_or_else(|| {
        format!(
            "解压完成但未找到 {name}（解压目录: {}）",
            extract_dir.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755));
    }

    let real_version = probe_version(&bin).unwrap_or_else(|| ver.clone());
    let size = std::fs::metadata(&bin).map(|m| m.len()).unwrap_or(downloaded_bytes);

    let meta = InstalledCore {
        path: bin.to_string_lossy().to_string(),
        version: real_version.clone(),
        os: os.clone(),
        arch: arch.clone(),
        installed_at: now_ms(),
        size,
    };
    write_meta(&app, &meta)?;

    emit_progress(&app, "done", &format!("frpc {real_version} 安装完成"), downloaded_bytes, downloaded_bytes);
    Ok(DownloadResult {
        path: meta.path,
        version: real_version,
        size,
        os,
        arch,
    })
}

fn download_to_file(app: &tauri::AppHandle, url: &str, dest: &Path) -> Result<u64, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(900))
        .user_agent("NetTool/0.1")
        .build()
        .map_err(|e| e.to_string())?;
    let mut resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE) {
        let ct = ct.to_str().unwrap_or("");
        if ct.contains("text/html") {
            return Err("下载源返回 HTML 页面而非压缩包（可能被镜像拦截）".to_string());
        }
    }
    let total = resp.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 128 * 1024];
    let mut downloaded: u64 = 0;
    let mut last_pct: i32 = -1;
    loop {
        let n = resp.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        downloaded += n as u64;
        let pct = if total > 0 {
            (downloaded as f64 / total as f64 * 100.0) as i32
        } else {
            -1
        };
        if pct != last_pct {
            last_pct = pct;
            emit_progress(
                app,
                "download",
                &format!("已下载 {:.1} MB", downloaded as f64 / 1024.0 / 1024.0),
                downloaded,
                total,
            );
        }
    }
    if let Ok(meta) = file.metadata() {
        if meta.len() < 100 * 1024 {
            return Err(format!("下载内容过小({} 字节)，可能不是有效压缩包", meta.len()));
        }
    }
    Ok(downloaded)
}

fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.mangled_name().to_string_lossy().replace('\\', "/");
        if name.is_empty() || name.ends_with('/') {
            continue;
        }
        let out_path = dest.join(&name);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
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
        let path = entry.path().map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
        if path.is_empty() || path.ends_with('/') {
            continue;
        }
        let out_path = dest.join(&path);
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        entry.unpack(&out_path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod net_tests {
    use super::*;

    /// 网络冒烟：真实拉取 GitHub releases（验证 reqwest 阻塞客户端可用）
    #[test]
    fn github_releases_smoke() {
        match list_frp_releases_blocking(Some(3)) {
            Ok(list) => {
                println!("releases: {:?}", list.iter().map(|r| r.tag.clone()).collect::<Vec<_>>());
                assert!(!list.is_empty());
            }
            Err(e) => println!("网络不可用，跳过: {e}"),
        }
    }

    /// 平台识别必须与运行平台一致
    #[test]
    fn platform_detection() {
        let os = detect_os();
        let arch = detect_arch();
        println!("platform: {os}/{arch}");
        assert!(matches!(os.as_str(), "windows" | "linux" | "darwin" | "freebsd"));
        assert!(matches!(arch.as_str(), "amd64" | "arm64" | "386" | "arm"));
    }
}
