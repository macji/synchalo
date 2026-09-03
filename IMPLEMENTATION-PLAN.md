# SyncHalo MVP 实施计划

| 项目 | 内容 |
| --- | --- |
| 版本 | v0.2 |
| 首发平台 | macOS ARM64/x64、Ubuntu ARM64 |
| 桌面架构 | Tauri 2 + React/TypeScript/Vite + Rust |
| 当前阶段 | M4：双平台验证与发布硬化 |

## v0.1.7 发布计划

1. Ubuntu DEB 与 macOS/Windows 一样执行启动约 5 秒、每 30 分钟和手动更新检查，发现新版后显示版本与发布说明。
2. 用户确认安装后通过 Polkit 弹出管理员授权，受限 helper 只允许 APT 从固定签名源安装精确的 `sync-halo` 版本，成功后重启应用。
3. 忽略版本只抑制自动提醒；手动检查仍显示同一版本。完成 DEB 解包安全检查、三平台构建和 `v0.1.7` 发布。

## v0.1.6 发布计划

1. Ubuntu ARM64 仅发布 DEB，所有默认、CI、手动和正式发布构建均停止生成 AppImage。
2. DEB 内置公开 APT 公钥和软件源配置，首次安装后由 Ubuntu 系统更新器或 APT 获取后续版本。
3. 自动校验 DEB 中的源、公钥指纹和旧配置迁移脚本，更新开源 README 后完成三平台 `v0.1.6` 正式发布。

## v0.1.5 发布计划

1. 在同步文件和设置页的“我的设备”标题右侧加入统一刷新按钮，触发 Rust 重启 mDNS 发布/浏览并恢复可信设备连接。
2. 自动更新关闭时仍执行启动后约 5 秒和每 30 分钟检查；新版提醒提供“立即更新 / 忽略此版本”，忽略值持久化并仅抑制同一版本。
3. 验证两个刷新入口、在线状态回写、更新提醒与忽略版本迁移，然后完成 macOS 公证和三平台 `v0.1.5` 正式发布。

## v0.1.4 开发计划

1. 保持启动约 5 秒和每 30 分钟的更新检查，自动更新开关仅控制是否预先下载，不再关闭版本提醒。
2. 关闭自动更新时展示版本、有限长度的发布说明和“立即更新 / 取消”；开启时先下载验签到私有临时文件，再展示“安装并重启 / 稍后”。
3. 检查、下载和安装使用同一互斥状态；安装前再次校验缓存摘要，更新包字节与路径不进入 WebView。
4. 完成前端、Rust、UI 烟测和 macOS Developer ID 签名、公证构建；随后按独立发布请求完成 `v0.1.4`。

## v0.1.3 发布计划

1. **P0 稳定性门禁**：恢复普通 CI 的无密钥原生打包检查；正式发布在构建前校验全部 updater、SignPath 和 APT 凭据；任何签名、版本、架构或发布者校验失败都不得创建 GitHub Release。
2. **Windows 可信发布者**：GitHub-hosted Windows runner 生成 MSI/NSIS；SignPath 审核期间 `v0.1.3` 使用明确披露的临时 unsigned 模式完成自动更新验证。审核通过后切换变量，SignPath Foundation 对标签构建执行 Authenticode 签名，工作流验证发布者后重新生成 Tauri updater 签名。手动单平台产物保持非正式、未签名状态。
3. **Ubuntu 签名 APT 源**：GitHub Actions 校验 ARM64 DEB，生成 `stable/main` 索引，用专用 GPG 密钥签署 `InRelease` 和 `Release.gpg`，验证后部署 GitHub Pages。
4. **完整发版**：所有测试通过后，在授权 Mac 上完成 Developer ID 签名、Apple 公证和 staple；创建 `v0.1.3` 标签，等待 SignPath 人工批准，核验 APT、Windows、Linux、macOS 和 updater 清单后完成 Release。

## 1. MVP 交付边界

首个可用版本必须完成：

1. 左侧三项导航和粘贴板、同步文件、设置三个完整页面。
2. 跟随系统浅色/深色主题，支持键盘、空状态、错误状态与撤销删除。
3. Rust 持有权威状态；WebView 只消费 ViewModel 和节流后的进度。
4. SQLite 持久化设备、加密粘贴板历史、文件任务和设置。
5. macOS 与 Ubuntu ARM64 监听纯文本剪贴板，并抑制远端写入回声。
6. 使用 mDNS 发现局域网设备；一次性同步码完成设备配对。
7. 已配对在线设备实时同步文本；离线旧文本不覆盖当前剪贴板。
8. 显式粘贴、选择或拖放文件；文件流由 Rust 直接传输、校验和提交。
9. 托盘驻留、暂停同步、接收目录和开机启动设置。
10. macOS 与 Ubuntu ARM64 原生 CI 构建、单元测试和前端交互测试。

## 2. 分阶段实现

### M0：工程骨架

- Cargo workspace、Tauri 壳、React SPA、CSS tokens。
- 领域 ViewModel、HLC、错误模型和 Tauri command/event 边界。
- macOS 与 Ubuntu ARM64 构建矩阵。

### M1：本地纵向链路

- SQLite schema、字段加密和设置存储。
- 剪贴板轮询、回写与历史增删查。
- 文件选择、拖放、任务历史与本机校验流程。

### M2：局域网同步

- mDNS 发布/浏览与在线状态。
- 一次性同步码、配对确认、身份与设备撤销。
- QUIC 控制连接、文本事件幂等与回声抑制。

### M3：可靠文件传输

- QUIC 文件流、BLAKE3 校验、临时文件和原子提交。
- 每设备状态、断线恢复、连续 offset 断点续传。
- 速度、剩余时间、取消、重试和重启恢复。

### M4：发布硬化

- 托盘、开机启动、通知、权限与 Wayland 能力提示。
- CSP/capabilities、日志脱敏、性能预算和安装包。
- macOS 签名/notarize 与 Ubuntu ARM64 DEB。

## 3. 当前完成度

| 阶段 | 状态 | 已落地内容 |
| --- | --- | --- |
| M0 | 完成 | Cargo workspace、Tauri/React、CSS tokens、typed command/event 边界、CI 骨架。 |
| M1 | 完成 | bundled SQLite、加密历史、系统设置、macOS/Linux 粘贴板、文件选择/粘贴/拖放。 |
| M2 | 完成 | mDNS、60 秒同步码、SPAKE2、证书固定、Ed25519 挑战、QUIC 文本同步与去重。 |
| M3 | 完成纵向版本 | BLAKE3、临时文件、原子提交、连续 offset 续传、多设备状态、重启恢复。 |
| M4 | 进行中 | macOS `.app` 已本地构建和启动；Ubuntu ARM64 原生 workflow 已配置，待 runner/真机验证。 |

Beta 前仍需完成：

- 在 Ubuntu 24.04 ARM64 的 X11 与 Wayland 真机分别跑安装、托盘、本地 `0600` 密钥文件和文件粘贴板测试。
- 使用真实 Apple Developer 身份完成 macOS 签名与 notarize。
- 跑 1 GB、10 GB、50 GB 传输与中断恢复基准。
- 在冻结 Protocol v1 前决定把当前长度前缀 JSON 控制帧迁移到 Protobuf，或正式修订 PRD 的编码决策。

## 4. 平台策略

| 能力 | macOS | Ubuntu ARM64 |
| --- | --- | --- |
| WebView | WKWebView | WebKitGTK 4.1 |
| 剪贴板 | NSPasteboard/arboard | X11 或 Wayland data-control/arboard |
| 密钥 | `synchalo.key`（0600）保存 KEK，SQLite 保存 wrapped DEK 与加密设备身份 | 同一方案；旧 Keychain/Secret Service 仅在一次性迁移时读取 |
| 托盘 | Tauri 状态栏图标，左键直接激活 | AppIndicator 菜单首项激活 |
| 构建 target | `aarch64-apple-darwin`、`x86_64-apple-darwin` | `aarch64-unknown-linux-gnu` |

macOS 不能可靠地直接交叉链接 Ubuntu WebKitGTK 应用，因此 Ubuntu ARM64 必须在原生 ARM64 Linux runner 或目标机上构建和验证。

## 5. 完成定义

- `cargo test --workspace` 通过。
- `npm test`、`npm run lint`、`npm run build` 通过。
- macOS 上 `npm run tauri -- dev` 可启动并完成本地功能链路。
- Ubuntu ARM64 CI 可生成安装产物。
- 两台已配对设备能双向同步文本，并能完成至少 1 GB 文件的校验传输。
- 文件传输期间 WebView 不接收文件字节，UI 保持可交互。
