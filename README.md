# NetTool — FRP 内网穿透管理工具

基于 **Tauri 2.x + Vue 3 + Vite** 的跨平台桌面应用，用于 FRP（fast reverse proxy）内网穿透的连接管理。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2.x |
| 前端 | Vue 3 + TypeScript + Vite |
| 图表 | ECharts |
| 后端 | Rust |

## 功能特性

- **控制面板**：配置选择、连接/断开、实时上下行流量图表
- **隧道管理**：导入 TOML / JSON 配置、新建/编辑/删除/启停隧道、复制为 TOML
- **核心下载**：选择 FRP 版本与平台架构，真实下载并解压 frpc 核心
- **设置**：服务器连接、日志、Web 控制台等 FRP 配置项
- **日志**：实时展示运行时日志，支持级别筛选与导出
- **用户中心**：账号信息、偏好设置（主题/语言/自启动等）
- **关于**：应用版本与开源许可信息
- 可折叠侧边栏、亮/暗主题切换、响应式布局

## 环境要求

- Node.js ≥ 18
- pnpm（或 npm）
- Rust 工具链（stable）
- 平台对应的 Tauri 系统依赖（见 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)）

## 开发

```bash
# 安装依赖
pnpm install

# 启动开发模式（同时启动 Vite 与 Tauri）
pnpm tauri dev

# 仅启动前端（浏览器调试）
pnpm dev

# 构建生产版本
pnpm tauri build
```

## 使用说明

### 1. 下载 FRP 核心

进入「核心下载」页面，选择 FRP 版本、操作系统与架构，点击「下载」。程序会：

1. 从 GitHub Releases 下载对应压缩包（失败时自动尝试国内镜像站）
2. 自动解压到应用数据目录 `{app_data_dir}/frp/`
3. 定位并记录 `frpc` 可执行文件路径

> 首次运行需联网。若 GitHub 直连不可用，程序会自动回退到镜像源。

### 2. 配置隧道

进入「隧道管理」页面：

- **导入**：粘贴完整的 `frpc.toml`（含 `[[proxies]]` 段落）或 JSON 配置，自动解析出隧道列表与服务器连接信息
- **新建/编辑**：手动配置隧道（类型、本地/远程端口、域名、`transport` 加密压缩等）
- **复制**：将单条隧道转换为标准 TOML 片段复制到剪贴板

### 3. 启动连接

进入「控制面板」，点击「连接」：

1. 根据「设置」中的服务器配置与已启用的隧道，生成 `frpc.toml`
2. 写入应用数据目录
3. 调用已下载的 `frpc` 核心启动进程，标准输出实时转发到「日志」页

## 配置文件示例

```toml
serverAddr = '1.1.1.1'
serverPort = 2333
user = '2a55d6f9a5b123efwfc36dc92dfbc6b'

[auth]
method = 'token'
token = 'ServerToken'

[[proxies]]
name = 'frpTest'
type = 'tcp'
localIP = '10.0.0.1'
localPort = 80
remotePort = 46438

[proxies.transport]
useEncryption = true
useCompression = false
```

## 项目结构

```
netTool/
├── src/                      # 前端
│   ├── App.vue               # 主框架（侧边栏 + 视图路由）
│   ├── api.ts                # Tauri 命令封装 / 状态持久化
│   ├── types.ts              # TypeScript 类型定义
│   ├── components/
│   │   ├── Sidebar.vue       # 可折叠侧边导航
│   │   └── TrafficChart.vue  # 流量图表
│   └── views/                # 7 个功能模块视图
├── src-tauri/                # Rust 后端
│   ├── src/lib.rs            # 命令定义（解析/下载/进程管理）
│   ├── Cargo.toml
│   └── tauri.conf.json
└── index.html
```

## 许可
