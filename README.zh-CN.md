# SyncHalo

[English](README.md)

SyncHalo 是一款本地优先的局域网剪贴板与文件同步工具。它不需要账号、云存储或公网中继，只在用户明确配对的可信设备之间传输数据。

[下载最新版本](https://github.com/macji/synchalo/releases/latest) · [APT 软件源](https://macji.github.io/synchalo/apt) · [安全策略](SECURITY.md) · [产品规格](PRD.md)

## 使用 AI 快速安装

把下面的提示词复制到 Codex、Claude Code 或其他可信的本地编程 Agent 中。Agent 应当识别你的操作系统和处理器架构，只使用 SyncHalo 官方下载，并在安装前验证软件包。

```text
请在这台电脑上安装 https://github.com/macji/synchalo/releases/latest 提供的最新版稳定版 SyncHalo。先识别操作系统和 CPU 架构；只允许使用 macji/synchalo 官方 Release 资产或官方签名 APT 软件源；下载后必须根据 Release 中的 SHA-256 校验文件验证安装包；不得绕过 Gatekeeper、SmartScreen、软件包签名或其他系统安全检查。macOS 请把已公证的 Apple Silicon 应用安装到 /Applications；Ubuntu 请用 APT 安装匹配架构的官方 DEB，使 SyncHalo 签名软件源同时完成注册，以便后续更新；Windows 请使用 x64 setup 安装程序，如果无法验证发布者签名，继续前必须先告诉我。安装完成后尽可能启动 SyncHalo，并报告已安装版本、软件包来源和校验结果。
```

## 功能

- 通过 mDNS 自动发现同一局域网内的设备，也可以手动刷新发现与重连状态。
- 使用 60 秒一次性同步码配对；新设备加入前仍需已有设备确认。
- 在可信设备间实时同步纯文本剪贴板，并抑制远端写入造成的同步回环。
- 支持拖放、原生文件选择器和页面内粘贴发送文件；未指定目标时发送给全部在线设备。
- 文件传输支持流式传输、断点续传、BLAKE3 完整性校验、临时文件和原子提交。
- 提供剪贴板历史、文件历史、搜索、收藏、再次同步和后端分页。
- 删除同步与收藏同步可独立开启，默认均为关闭。
- 支持英语、简体中文、繁体中文、日语和韩语，默认跟随系统语言，也可以在设置中即时切换。
- 支持系统托盘驻留、暂停同步、开机启动、自定义接收目录和设备管理。
- 启动约 5 秒后检查更新，之后每 30 分钟检查一次，也可以手动检查。
- 忽略版本只会停止启动和定时提醒；手动检查仍会显示该版本。
- macOS 和 Windows 可以在后台下载并验证更新，随后请求安装确认；Ubuntu 会在确认后请求管理员授权，通过签名 APT 软件源安装并重启。

## 安全与隐私

- 剪贴板内容和文件不会上传到 SyncHalo 服务器；数据链路仅限局域网内的可信设备。
- 配对使用 SPAKE2 密码认证密钥交换；可信连接使用 QUIC、TLS 1.3、证书固定和设备签名挑战。
- 剪贴板正文以 XChaCha20-Poly1305 加密后保存在本机 SQLite 中。
- 本地生成的 KEK 保存在权限为 `0600` 的 `synchalo.key` 中；数据库只保存包装后的数据密钥和加密身份。
- 私钥、文件字节、数据库句柄和密码学操作不会进入 WebView。
- 生产页面只加载应用内静态资源，日志不会记录剪贴板或文件内容。
- Ubuntu 管理员 helper 只接受受限版本号，并且只能通过固定签名源升级 `sync-halo` 软件包；应用本身不会获得 root 权限。

## 下载与安装

当前正式发布产物支持：

| 平台 | 架构 | 安装包 | 更新方式 |
| --- | --- | --- | --- |
| macOS 13+ | Apple Silicon（ARM64） | ZIP 中的 `.app` | 应用内签名更新 |
| Ubuntu 24.04 | ARM64、x86_64（amd64） | `.deb` | 应用提醒、Polkit 授权、签名 APT 安装 |
| Windows 10/11 | x64 | NSIS `.exe`、`.msi` | 应用内签名更新 |

请从 [GitHub Releases](https://github.com/macji/synchalo/releases/latest) 下载适用于当前平台的最新软件包。

### macOS

下载 `SyncHalo_<版本>_macos-arm64.zip`，解压后把 `SyncHalo.app` 移到“应用程序”目录并打开。正式软件包经过 Developer ID 签名和 Apple 公证。

如果 macOS 仍然阻止首次启动，请在 Finder 中按住 Control 键点按 SyncHalo，选择“打开”并确认。

### Ubuntu：DEB（推荐）

先检查设备架构：

```bash
dpkg --print-architecture
```

安装包支持 `arm64` 和 `amd64`。下载与输出架构匹配的 DEB，然后运行：

```bash
cd ~/Downloads
sudo apt install ./SyncHalo_*_ubuntu-*.deb
sudo apt update
```

DEB 会安装 SyncHalo 的公开 APT 签名密钥、Deb822 软件源、受限更新 helper 和 Polkit 策略。发现更新时，SyncHalo 会显示版本和发布说明。选择“立即更新”后会打开 Ubuntu 管理员授权窗口；APT 将验证并安装指定版本，然后 SyncHalo 自动重启。

你也可以通过 Ubuntu 软件更新器更新，或运行：

```bash
sudo apt update
sudo apt install --only-upgrade sync-halo
```

如果不想先下载 DEB，也可以直接添加软件源并安装：

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://macji.github.io/synchalo/apt/synchalo-archive-keyring.asc \
  | sudo tee /etc/apt/keyrings/synchalo-archive-keyring.asc >/dev/null
printf '%s\n' \
  'Types: deb' \
  'URIs: https://macji.github.io/synchalo/apt' \
  'Suites: stable' \
  'Components: main' \
  'Architectures: amd64 arm64' \
  'Signed-By: /etc/apt/keyrings/synchalo-archive-keyring.asc' \
  | sudo tee /etc/apt/sources.list.d/synchalo.sources >/dev/null
sudo apt update
sudo apt install sync-halo
```

图形安装器可能会把首次单独下载的 DEB 标记为未知来源。软件源注册完成后，后续所有仓库元数据和软件包摘要都会由 SyncHalo APT 密钥认证。

### Windows

运行 `SyncHalo_<版本>_windows-x64-setup.exe`，也可以使用 MSI 进行集中部署。

在 SignPath Foundation 审核完成前，Windows 正式软件包暂时没有 Authenticode 签名，Windows 可能显示未知发布者或 SmartScreen 警告。每个 Release 都包含 SHA-256 校验文件；请只从本仓库的 Releases 页面下载安装程序。

## 第一次使用

1. 确保两台设备处于同一局域网，并且防火墙允许 SyncHalo 的本地网络通信。
2. 在已有设备的“设置”或“同步文件”页面生成一次性同步码。
3. 在另一台设备上选择“加入”，输入同步码。
4. 返回已有设备，确认新设备的名称和平台。
5. 配对完成后，文本会自动同步。文件必须通过选择、拖放或在“同步文件”页面粘贴来显式发送。

Wayland 对后台全局剪贴板访问有更严格的限制。Ubuntu 上的实际能力取决于桌面 compositor 是否支持 data-control。不支持时，SyncHalo 会降级为仅窗口活跃时同步或手动同步；文件同步不受影响。

## 从源码开发

### 通用环境

- Git
- Node.js 22 或更高版本，推荐 Node.js 24
- Rust 1.88 或更高版本及 Cargo
- 当前平台对应的 Tauri 2 系统依赖

克隆仓库并安装锁定依赖：

```bash
git clone https://github.com/macji/synchalo.git
cd synchalo
npm ci
```

使用演示数据运行 Web UI：

```bash
npm run dev
```

运行包含 Rust 后端、真实设备发现和系统集成的完整桌面应用：

```bash
npm run tauri -- dev
```

仅限本地调试时，可以启用临时内存密钥：

```bash
SYNCHALO_EPHEMERAL_KEYS=1 npm run tauri -- dev
```

该模式不会持久化剪贴板历史或设备信任，禁止用于发布构建。

### macOS 源码构建

安装 Xcode Command Line Tools：

```bash
xcode-select --install
rustup target add aarch64-apple-darwin
```

没有 Developer ID 证书时，可以生成仅供本机测试的 ad-hoc 软件包：

```bash
APPLE_SIGNING_IDENTITY=- npm run tauri -- build \
  --target aarch64-apple-darwin \
  --bundles app,dmg \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

该构建没有经过 Apple 公证，不得作为正式版本分发。

### Ubuntu 源码构建

请在 Ubuntu 24.04 ARM64 或 x86_64 主机上安装依赖：

```bash
sudo apt update
sudo apt install -y \
  libappindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  patchelf \
  xdg-utils

npm ci
npm run tauri -- build --bundles deb
```

GitHub Actions 分别使用原生 `ubuntu-24.04-arm` 和 `ubuntu-24.04` runner 构建 ARM64 与 x86_64 DEB，不使用跨架构模拟。

### Windows x64 源码构建

需要安装 Visual Studio 2022 Build Tools（包含“使用 C++ 的桌面开发”）、WebView2、Node.js 和 Rust MSVC 工具链。然后在 Developer PowerShell 中运行：

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis,msi --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

本地构建的安装包不包含 SyncHalo 官方的 Authenticode 或 Tauri 更新签名。

## 测试与验证

提交前运行：

```bash
npm run build
npm run lint
npm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash -n scripts/*.sh tests/release/*.sh
tests/release/deb_apt_bootstrap_smoke.sh
tests/release/apt_repository_smoke.sh
```

浏览器 UI 烟测需要先启动 Vite：

```bash
npm run dev
```

然后在另一个终端运行：

```bash
python3 tests/e2e/ui_smoke.py
```

## 项目结构

```text
apps/desktop/src/        React 与 TypeScript 界面
apps/desktop/src-tauri/  Tauri command 与桌面运行时
crates/core/             领域模型与事件语义
crates/network/          mDNS、配对、QUIC 与可信连接
crates/platform/         系统剪贴板与平台适配
crates/storage/          SQLite、迁移与本地加密
crates/transfer/         文件分块、续传与完整性校验
tests/e2e/               浏览器交互测试
tests/release/           发布与软件包仓库测试
packaging/               Linux 软件源与安装包资源
```

完整的产品与发布文档见 [PRD.md](PRD.md) 和 [RELEASING.md](RELEASING.md)。

## 发布

正式标签会触发 GitHub Actions，并行构建 Ubuntu ARM64、Ubuntu x86_64 和 Windows x64 软件包，同时发布签名的双架构 APT 软件源。macOS ARM64 在授权 Mac 上构建，使用 Developer ID 签名并由 Apple 公证，生成 Tauri 更新签名后上传到同一个 GitHub Release。

构建和仓库中只保存公开验证密钥。APT 私钥、Tauri 更新私钥、Apple 证书和公证凭据不会进入源码管理或 Git 历史。完整流程见 [RELEASING.md](RELEASING.md)。

## 许可证

MIT。详见 [LICENSE](LICENSE)。
