# NetTool — FRP 内网穿透管理工具

基于 **Tauri 2.x + Vue 3 + Vite** 的跨平台桌面应用，用于 FRP（fast reverse proxy）内网穿透的配置管理、核心安装与连接运维。

## 技术栈

| 层 | 技术 |
| --- | --- |
| 桌面框架 | Tauri 2.x |
| 前端 | Vue 3 + TypeScript + Vite |
| 图表 | ECharts |
| 后端 | Rust（reqwest / toml / zip / flate2 / tar） |

## 功能特性

- **多配置管理**：每套配置独立保存服务器参数、认证令牌与隧道列表，支持新建 / 编辑 / 复制 / 删除 / 切换 / 导入 TOML / 导出 TOML
- **第三方服务**：已接入 ME Frp、OpenFrp、ChmlFrp、SakuraFrp、MSL Frp 五家，支持密钥 / 密码 / 网页授权登录（含人机验证），读取用户信息与隧道列表、预览与获取配置、导入为本地配置并一键启动；四家共用同一套「登录 → 隧道列表 → 配置 → 导入启动」抽象，新增服务商只需实现一个子模块
- **加载反馈**：启动、控制面板、服务商、核心下载、设置等模块都有骨架屏 / 旋转指示器，异步操作不再先显示占位或示例数据；首次启动也不会自动创建示例隧道
- **真实流量统计**：按隧道统计真实的上下行速率、累计流量与连接数（本地计量网关逐字节计数，非模拟数据），并展示 frpc 管理接口回报的代理真实状态
- **真实核心下载**：从 GitHub Releases 拉取版本列表并下载对应平台/架构的 frpc，带进度事件、多镜像回退、解压与版本校验（`frpc -v`）
- **真实内网穿透**：按当前配置生成标准 `frpc.toml`（transport / TLS / auth / log / webServer），启动并托管 frpc 进程，日志实时回传
- **隧道管理**：TOML / JSON 导入、按类型（tcp/udp/http/https/stcp/xtcp/tcpmux）自适应字段、HTTP(S) 隧道自动给出可访问地址、复制为 TOML；支持批量启用 / 禁用 / 删除
- **敏感字段显隐**：认证令牌、服务商密码等支持一键切换明文/密文
- **批量配置管理**：配置卡片可多选，支持批量导出（逐套 frpc.toml）与批量删除
- **日志**：实时回传 frpc 运行日志（自动去除 ANSI 颜色码并解析时间戳 / I W E 级别），支持级别筛选、搜索、自动滚动与导出
- **后台运行**：关闭窗口时可选择「后台运行（托盘）」或「直接关闭」，支持记住选择；托盘图标可重新打开主界面或退出；设置页可随时修改该行为
- **软件设置**：外观（主题 / 自定义背景图 / 圆角 0–24px / 磨砂玻璃与模糊强度）、存储位置（数据与日志目录，可迁移现有文件）、开机自启动、关闭行为与运行环境；连接参数统一在「配置管理」中维护\n- **通知提醒**：所有关键操作结果以右下角轻提示（成功 / 错误 / 警告 / 信息）弹出，自动堆叠与倒计时收起\n- **开机自启动**：可在设置中一键写入 Windows 登录自启动项
- 可折叠侧边栏、亮/暗主题、响应式布局；默认窗口 1920×1080（最小 1200×720）

## 环境要求

- Node.js ≥ 18
- pnpm（或 npm）
- Rust 工具链（stable）
- 平台对应的 Tauri 系统依赖（见 [Tauri 前置要求](https://v2.tauri.app/start/prerequisites/)）

## 开发

```bash
pnpm install
pnpm tauri dev      # 同时启动 Vite 与 Tauri
pnpm dev            # 仅前端（浏览器调试，Tauri API 不可用）
pnpm tauri build    # 生产构建

cd src-tauri && cargo test    # 后端单元测试
```

## 使用说明

### 1. 安装 FRP 核心

「核心下载」页会自动识别当前操作系统与架构（windows/linux/darwin × amd64/arm64/…），版本列表来自 GitHub API（失败时回退内置列表）。下载流程：

1. 依次尝试 GitHub 直链与镜像源，过程中通过 `core-download` 事件回传实时进度
2. 解压到 `{app_data_dir}/frp/frp_{version}_{os}_{arch}/`
3. 用 `frpc -v` 校验并写入 `core.json` 安装元数据

### 2. 管理配置（多配置）

「配置管理」页负责多套配置的增删改与切换：

- 每套配置拥有独立的 `serverAddr / serverPort / auth.token / user / transport / tls / 隧道列表`
- 运行时生成的配置文件写入 `{app_data_dir}/profiles/{profile_id}/frpc.toml`，多配置互不覆盖
- 支持从 TOML 导入为新配置、把任意配置导出为 TOML

### 3. 配置隧道

「隧道管理」页作用于当前配置：

- 导入：粘贴完整 `frpc.toml`（含 `[[proxies]]`）或 JSON，自动解析服务器信息与隧道
- 新建/编辑：按类型显示对应字段（tcp/udp 用 remotePort；http/https 用 customDomains/subdomain；stcp/xtcp 用 secretKey）
- 保存后需在控制面板重新连接才会生效

### 4. 连接与流量统计

「控制面板」点击「连接」后，后端会：

1. 为每条 TCP 类隧道启动**本地计量网关**（`127.0.0.1` 随机端口），把隧道 `localPort` 指向网关端口，网关再转发到真实的 `localIP:localPort`
2. 生成 `frpc.toml`（含 `webServer` 管理端口）并拉起 frpc 进程
3. 每秒从网关读取双向字节数，计算真实速率与累计值，通过 `frp-traffic` 事件推送前端；每 2 秒读取 frpc `/api/status` 获取每条代理的真实状态与远端地址

数据路径：

```
外网用户 → frps → frpc → 本地计量网关（计数） → 真实本地服务
```

> UDP 隧道的流量方向无法在 TCP 层计数，UI 中会标注「无字节统计」。

### 5. 第三方服务（多服务商）

「第三方服务」页已接入四家 FRP 服务商，登录后可直接读取远端隧道、预览配置、导入为本地配置并启动：

| 服务商 | 登录方式 | 配置来源 |
| --- | --- | --- |
| [ME Frp](https://www.mefrp.com/) | 访问密钥 / 密码（需人机验证） | `POST /auth/proxy/config[/multiple]` 官方生成 |
| [OpenFrp](https://www.openfrp.net/) | **网页授权（Remote Login，推荐）** / 旧版 Authorization | 由隧道列表 + 节点信息本地生成 |
| [ChmlFrp](https://www.chmlfrp.net/) | 访问令牌（access_token） | `GET /tunnel_config`（INI，自动转为 TOML） |
| [SakuraFrp](https://www.natfrp.com/) | 访问密钥 | `POST /v4/tunnel/config` 服务端下发 |
| [MSL Frp](https://user.mslmc.net/) | 用户 Token（`Bearer`） | `GET /api/frp/getTunnelConfig?id=` 服务端返回 TOML |

统一流程：**登录 → 读取用户信息与隧道列表 → 预览/导出配置 → 导入为本地配置（可同时启动 frpc）**。凭据保存在本地 `providers.json`，可分别退出登录。

各服务商接口约定：

| 能力 | ME Frp | OpenFrp | ChmlFrp | SakuraFrp |
| --- | --- | --- | --- | --- |
| 鉴权 | `Bearer <token>` | `Authorization: <网页授权令牌>`（旧版为 `OPENFRP<jwt>`） | `?access_token=` | `Bearer <访问密钥>` |
| 登录 | `POST /public/login`（+captchaToken） | 网页授权（argoAccess requestLogin / pollLogin） | — | — |
| 用户信息 | `GET /auth/user/info` | `POST /frp/api/getUserInfo` | `GET /userinfo` | `GET /v4/user/info` |
| 隧道列表 | `GET /auth/proxy/list` | `POST /frp/api/getUserProxies` | `GET /tunnel` | `GET /v4/tunnels` |
| 节点 | 隧道列表内返回 | `POST /frp/api/getNodeList` | `GET /node` | `GET /v4/nodes` |
| 配置 | `POST /auth/proxy/config` | 本地组装 | `GET /tunnel_config` | `POST /v4/tunnel/config` |

> - ME Frp 密码登录的人机验证：打开 `https://www.mefrp.com/3rdparty/captcha?client=nettool`，把页面给出的 Base64 验证码粘贴回来（程序会自动解码取 `token`）。
> - OpenFrp 面板已移除旧版「第三方客户端安全登录」的 Authorization 复制入口，面板里的 32 位「用户访问密钥」只能用于 frpc 的 `-u` 启动，不能用于 API 鉴权。程序按 [OPENAPI 文档](https://github.com/ZGIT-Network/OPENFRP-APIDOC) 实现推荐的 **Remote Login**：申请授权 → 打开官方授权页 → 每 5 秒轮询 → 用 Curve25519 + XSalsa20-Poly1305 解密出 Authorization。新版令牌是 56 位原始字符串，程序原样放入 `Authorization` 头；只有旧版三段式 JWT 才会补 `OPENFRP` 前缀。旧账号已保存的 `OPENFRP...` 仍可走「密钥登录」。
> - ChmlFrp 文档标注为 `http://`，实际服务仅 HTTPS 可用，程序已使用 `https://cf-v2.uapis.cn`；v2 接口字段为全小写（`localip` / `nport` / `dorp` / `usergroup`），程序同时兼容 v1 驼峰字段，避免登录成功却读不到隧道。
> - 服务端返回的配置可能是 TOML 或旧版 INI（`[common]` + `[隧道名]`），程序统一解析后重新渲染为标准 `frpc.toml`。
> - 同一批次导入的隧道必须属于同一节点（服务端限制），跨节点会提示分批导入。
> - ME Frp 支持「签到领流量」：点签到后在浏览器完成人机验证，把 Base64 验证码粘贴回来提交。
> - 流量单位各不相同（ME Frp 为 **MB**、SakuraFrp 为字节、OpenFrp 为 MiB），程序统一按各服务商面板口径换算展示；ME Frp 的带宽字段单位为 `1/128 Mbps`（如 `8320` → `65 Mbps`）。
> - 隧道表格的「速率」列：服务端未提供逐隧道限速时，展示账号带宽作为参考。
> - MSL Frp 接口基址为 `https://user.mslmc.net/api`：`/frp/userInfo`、`/frp/nodeList`、`/frp/getTunnelList`、`/frp/getTunnelConfig?id=`；Token 在 MSL 用户中心复制（请求头 `Authorization: Bearer <Token>`）。\n> - 应用标识符为 `top.hn3.nettool`，默认数据目录 `%APPDATA%/top.hn3.nettool`；可在「设置 → 存储位置」自定义数据目录与日志目录（重启生效，可选迁移现有文件）。首次启动会自动从旧目录 `com.jlhan.nettool` 迁移配置、核心与登录凭据。

## 配置文件示例



程序生成/使用的标准格式（`frpc verify` 校验通过，兼容 frp 0.61 / 0.72）：

```toml
serverAddr = "1.1.1.1"
serverPort = 7000
user = "2a55d6f9a5b123efwfc36dc92dfbc6b"
transport.protocol = "tcp"
transport.tls.enable = true
log.to = "console"
log.level = "info"
log.maxDays = 3
webServer.addr = "127.0.0.1"
webServer.port = 7401

[auth]
method = "token"
token = "ServerToken"

[[proxies]]
name = "frpTest"
type = "tcp"
localIP = "10.0.0.1"
localPort = 12345        # 运行时替换为计量网关端口
remotePort = 46438
transport.useEncryption = true
transport.useCompression = false
```

## 项目结构

```
netTool/
├── src/                        # 前端
│   ├── App.vue                 # 框架 + 多配置状态管理（增删改/切换/落盘）
│   ├── api.ts                  # Tauri 命令封装、事件、格式化工具
│   ├── types.ts                # Profile / TunnelConfig / TrafficSnapshot / MeProxy 等类型
│   ├── components/
│   │   ├── Sidebar.vue
│   │   ├── PasswordField.vue
│   │   ├── TrafficChart.vue
│   │   ├── ToastHost.vue       # 右下角通知承载\n│   │   ├── Spinner.vue         # 旋转加载指示器
│   │   ├── Skeleton.vue        # 骨架屏色块
│   │   ├── PageLoader.vue      # 整页加载动画
│   │   └── EmptyState.vue      # 统一空状态
│   └── views/
│       ├── Dashboard.vue       # 连接控制 + 真实流量面板
│       ├── Tunnels.vue         # 当前配置的隧道管理（含 HTTP(S) 访问地址）
│       ├── Profiles.vue        # 多配置管理
│       ├── Providers.vue       # 第三方服务（多服务商登录/隧道/配置/启动）
│       ├── CoreDownload.vue    # 核心下载（进度/平台识别）
│       ├── Settings.vue        # 当前配置参数 + 配置预览
│       ├── Logs.vue / UserCenter.vue / About.vue
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs              # AppState、命令注册、窗口关闭回收
│   │   ├── model.rs            # Profile / ProfileStore / TunnelConfig
│   │   ├── store.rs            # profiles.json 持久化
│   │   ├── settings.rs         # 应用设置（关闭行为）与托盘
│   │   ├── config.rs           # frpc.toml 渲染与解析（含单元测试）
│   │   ├── core.rs             # 平台识别、GitHub releases、下载解压
│   │   ├── traffic.rs          # 计量网关、字节统计、/api/status 解析
│   │   ├── providers/          # 第三方服务商
│   │   │   ├── mod.rs          # 通用引擎（注册表 / 账号 / 命令 / 配置装配）
│   │   │   ├── mefrp.rs        # ME Frp
│   │   │   ├── openfrp.rs      # OpenFrp（网页授权 + 密钥登录）
│   │   │   ├── chmlfrp.rs      # ChmlFrp（兼容 v1/v2 字段）
│   │   │   └── sakura.rs       # SakuraFrp
│   │   └── runner.rs           # frpc 编排（网关→配置→进程→采样）
│   ├── Cargo.toml
│   └── tauri.conf.json
└── index.html
```

## 数据目录

```
{app_data_dir}/
├── profiles.json               # 多配置存储（activeId + profiles）
├── providers.json              # 第三方服务登录凭据
├── settings.json               # 应用设置（关闭窗口行为）
├── core.json                   # 已安装核心元数据
├── frp/                        # 下载的 frpc 核心
└── profiles/{profile_id}/
    └── frpc.toml               # 该配置生成的运行配置
```

## 许可


