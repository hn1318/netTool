# 发布流程

## 分支与手动触发

| 动作 | 触发内容 |
| --- | --- |
| 推送到 `master` / `main`（改动涉及 `src`、`src-tauri`、`package.json`、`pnpm-lock.yaml`） | `ci.yml`：类型检查 → 前端构建 → 后端单测 → release 构建（预热缓存） |
| Pull Request | `ci.yml` 同上（不写缓存） |
| 推送 `v*` tag（例如 `v0.1.1`） | `release.yml`：校验 tag 与版本号一致 → 构建 `.msi` + `-setup.exe` → 创建 GitHub Release |
| Actions 页面手动运行 release | `release.yml`：用输入的版本号构建，Release 默认为 **草稿** |

## 发布新版本（推荐流程）

版本号必须同时出现在三处：

1. `package.json` → `"version"`
2. `src-tauri/tauri.conf.json` → `"version"`
3. `src-tauri/Cargo.toml` → `version`（`Cargo.lock` 会在构建时自动更新）

然后：

```bash
# 1. 改上面三处版本号，例如 0.1.0 -> 0.1.1
git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml
git commit -m "chore(release): v0.1.1"
git push origin master

# 2. 打 tag 并推送，触发发布流水线
git tag v0.1.1
git push origin v0.1.1
```

推送 tag 后，`release.yml` 会：

- 校验 `v0.1.1` 与 `tauri.conf.json`、`package.json` 的版本号一致（不一致直接失败）
- 从 `ci.yml` 预热的 rust 缓存恢复，显著缩短编译时间
- 打包 NSIS（`-setup.exe`）与 MSI（`.msi`）
- 创建 GitHub Release 并上传安装包

## 可选：最小化脚本

```bash
# 用法：./scripts/bump.sh 0.1.2
npm version <version> --no-git-tag-version   # 改 package.json（不建 tag）
```

随后手动同步 `src-tauri/tauri.conf.json` 与 `src-tauri/Cargo.toml`，再按上面的方式打 tag。
（`npm version` 没有直接覆盖 TOML 的能力，因此这里不强制脚本化。）

## 常见失败原因

| 现象 | 原因 / 处理 |
| --- | --- |
| `Tag vX.Y.Z does not match tauri.conf.json version` | 三处版本号没同步，改完重新打 tag |
| `Port 1420 is already in use` | 仅出现在本地 `pnpm tauri dev` 多开，与流水线无关 |
| Release 没有产物 | 查看 Actions 中 `Build and publish` 步骤日志，通常是 bundler 报错 |
| 需要自动更新（updater） | 需在仓库 Secrets 配置 `TAURI_SIGNING_PRIVATE_KEY` 与 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`；未配置时安装包照常构建，仅不产出签名更新清单 |
