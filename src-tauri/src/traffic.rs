//! 真实流量统计：在 frpc 与本地服务之间插入计量网关。
//!
//! 数据路径：外网用户 -> frps -> frpc -> 【本应用计量网关】 -> 本地服务
//! 网关监听 127.0.0.1:随机端口，frpc 的 localPort 指向该端口，
//! 网关再把流量转发到真实的 localIP:localPort，同时对两个方向做字节计数：
//!   - 从 frpc 侧读到的字节 = 下行（下载）
//!   - 写回 frpc 侧的字节   = 上行（上传）
//! 因此统计值是真实的、按隧道区分的，且不依赖 frps 面板与管理员权限。

use crate::model::{now_ms, TunnelConfig};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct TunnelStat {
    pub up: AtomicU64,
    pub down: AtomicU64,
    /// 当前并发连接数
    pub conns: AtomicI64,
    /// 累计连接次数
    pub conn_total: AtomicU64,
}

impl TunnelStat {
    pub fn reset(&self) {
        self.up.store(0, Ordering::Relaxed);
        self.down.store(0, Ordering::Relaxed);
        self.conn_total.store(0, Ordering::Relaxed);
    }
}

pub struct Gateway {
    pub tunnel_id: String,
    pub tunnel_name: String,
    pub proxy_type: String,
    pub target: String,
    pub port: u16,
    pub stat: Arc<TunnelStat>,
    stop: Arc<AtomicBool>,
}

impl Gateway {
    /// 为一条隧道启动计量网关，返回网关句柄（含分配到的端口）
    pub fn start(t: &TunnelConfig) -> Result<Gateway, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0u16))
            .map_err(|e| format!("计量网关端口分配失败: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| e.to_string())?
            .port();

        let stat = Arc::new(TunnelStat::default());
        let stop = Arc::new(AtomicBool::new(false));
        let target = t.target();

        let stat_loop = stat.clone();
        let stop_loop = stop.clone();
        let target_loop = target.clone();

        thread::spawn(move || {
            for incoming in listener.incoming() {
                if stop_loop.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(client) = incoming else { continue };
                let target = target_loop.clone();
                let stat = stat_loop.clone();
                thread::spawn(move || handle_conn(client, target, stat));
            }
        });

        Ok(Gateway {
            tunnel_id: t.id.clone(),
            tunnel_name: t.name.clone(),
            proxy_type: t.proxy_type.clone(),
            target,
            port,
            stat,
            stop,
        })
    }

    pub fn shutdown(&self) {
        self.stop.store(true, Ordering::Relaxed);
        // 主动连一次以唤醒阻塞在 accept 的线程
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn handle_conn(client: TcpStream, target: String, stat: Arc<TunnelStat>) {
    let upstream = match TcpStream::connect(&target) {
        Ok(s) => s,
        Err(_) => return, // 本地服务未启动，直接断开（frpc 侧会看到连接关闭）
    };
    let _ = client.set_nodelay(true);
    let _ = upstream.set_nodelay(true);

    stat.conns.fetch_add(1, Ordering::Relaxed);
    stat.conn_total.fetch_add(1, Ordering::Relaxed);

    let (Ok(c_read), Ok(u_read)) = (client.try_clone(), upstream.try_clone()) else {
        stat.conns.fetch_sub(1, Ordering::Relaxed);
        return;
    };

    let s1 = stat.clone();
    let down_pump = thread::spawn(move || pump(c_read, upstream, s1, false));
    let s2 = stat.clone();
    let up_pump = thread::spawn(move || pump(u_read, client, s2, true));

    let _ = down_pump.join();
    let _ = up_pump.join();
    stat.conns.fetch_sub(1, Ordering::Relaxed);
}

fn pump(mut r: TcpStream, mut w: TcpStream, stat: Arc<TunnelStat>, upload: bool) {
    let mut buf = vec![0u8; 32 * 1024];
    let counter = if upload { &stat.up } else { &stat.down };
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                counter.fetch_add(n as u64, Ordering::Relaxed);
                if w.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    let _ = w.shutdown(Shutdown::Write);
}

// ============ 快照结构 ============

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TunnelTraffic {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub target: String,
    pub gateway_port: u16,
    pub up_speed: u64,
    pub down_speed: u64,
    pub up_total: u64,
    pub down_total: u64,
    pub conns: i64,
    pub conn_total: u64,
    /// frpc 管理接口回报的真实状态：running / start error / 空
    pub status: String,
    pub err: String,
    pub remote_addr: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TrafficSnapshot {
    pub running: bool,
    pub up_speed: u64,
    pub down_speed: u64,
    pub up_total: u64,
    pub down_total: u64,
    pub conns: i64,
    pub profile_id: String,
    pub profile_name: String,
    pub started_at: u64,
    pub tunnels: Vec<TunnelTraffic>,
    pub updated_at: u64,
}

impl TrafficSnapshot {
    pub fn idle() -> Self {
        Self {
            updated_at: now_ms(),
            ..Default::default()
        }
    }
}

// ============ frpc 本地管理接口 ============

#[derive(Debug, Clone, Default)]
pub struct ProxyStatus {
    pub status: String,
    pub err: String,
    pub remote_addr: String,
}

/// 读取 frpc 的 /api/status，得到每条代理的真实运行状态
pub fn fetch_proxy_status(port: u16) -> HashMap<String, ProxyStatus> {
    let mut map = HashMap::new();
    if port == 0 {
        return map;
    }
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(800))
        .no_proxy()
        .build()
    {
        Ok(c) => c,
        Err(_) => return map,
    };
    let url = format!("http://127.0.0.1:{port}/api/status");
    let Ok(resp) = client.get(url).send() else {
        return map;
    };
    if !resp.status().is_success() {
        return map;
    }
    let Ok(text) = resp.text() else { return map };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return map;
    };
    let Some(obj) = value.as_object() else { return map };

    for (_kind, list) in obj {
        let Some(items) = list.as_array() else { continue };
        for item in items {
            let Some(name) = item.get("name").and_then(|v| v.as_str()) else {
                continue;
            };
            map.insert(
                name.to_string(),
                ProxyStatus {
                    status: item
                        .get("status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    err: item
                        .get("err")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    remote_addr: item
                        .get("remote_addr")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                },
            );
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    /// 计量网关必须真实统计两个方向的字节数
    #[test]
    fn gateway_counts_both_directions() {
        // 模拟本地服务：echo
        let svc = TcpListener::bind("127.0.0.1:0").unwrap();
        let svc_port = svc.local_addr().unwrap().port();
        thread::spawn(move || {
            for stream in svc.incoming() {
                let Ok(mut s) = stream else { break };
                thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = s.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        let _ = s.write_all(&buf[..n]);
                    }
                });
            }
        });

        let t = TunnelConfig {
            id: "t1".to_string(),
            name: "echo".to_string(),
            proxy_type: "tcp".to_string(),
            local_ip: "127.0.0.1".to_string(),
            local_port: svc_port,
            ..Default::default()
        };
        let gw = Gateway::start(&t).expect("网关启动失败");

        let mut client = TcpStream::connect(("127.0.0.1", gw.port)).unwrap();
        let payload = vec![7u8; 8 * 1024];
        client.write_all(&payload).unwrap();
        let mut back = vec![0u8; payload.len()];
        client.read_exact(&mut back).unwrap();
        thread::sleep(Duration::from_millis(150));

        // 客户端写入 = 下行（来自外网的数据），回写 = 上行
        assert_eq!(gw.stat.down.load(Ordering::Relaxed), payload.len() as u64);
        assert_eq!(gw.stat.up.load(Ordering::Relaxed), payload.len() as u64);
        assert!(gw.stat.conn_total.load(Ordering::Relaxed) >= 1);

        gw.shutdown();
    }
}
