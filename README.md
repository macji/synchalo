# SyncHalo

SyncHalo 是一款本地优先的局域网剪贴板与文件同步工具。它不需要账号、云盘或公网中继，只在用户明确配对的可信设备之间传输数据。

[下载最新版本](https://github.com/macji/synchalo/releases/latest) · [APT 软件源](https://macji.github.io/synchalo/apt) · [安全策略](SECURITY.md) · [产品设计](PRD.md)

## 产品功能

- 在同一局域网内通过 mDNS 自动发现设备，并可随时刷新发现和重连状态。
- 使用 60 秒一次性同步码配对；新设备仍需由已有设备确认后才能加入。
- 在可信设备之间实时同步纯文本剪贴板，并抑制远端写入造成的同步回环。
- 支持拖放、文件选择和页面内粘贴发送文件；未指定目标时发送给全部在线设备。
- 文件传输支持流式传输、断点续传、BLAKE3 完整性校验、临时文件和原子提交。
- 提供剪贴板历史、文件历史、搜索、收藏、再次同步和后端分页。
- 删除同步与收藏同步可独立开启，默认关闭。
- 支持托盘驻留、暂停同步、开机启动、自定义接收目录和设备管理。
- macOS 和 Windows 启动约 5 秒后检查更新，之后每 30 分钟检查一次，也可手动检查。
- macOS 和 Windows 关闭自动更新时显示新版说明，由用户选择立即更新或忽略该版本；开启后后台下载并验签，安装前仍会请求确认。Ubuntu 由签名 APT 源更新。

## 安全与隐私

- 剪贴板和文件不上传到 SyncHalo 服务器，数据面只在局域网可信设备之间传输。
- 配对使用 SPAKE2 密码认证密钥交换；可信连接使用 QUIC、TLS 1.3、证书固定和设备签名挑战。
- 剪贴板正文以 XChaCha20-Poly1305 加密后保存在本机 SQLite 中。
- Rust 生成的本地 KEK 保存在权限为 `0600` 的 `synchalo.key`；数据库只保存包装后的数据密钥和加密身份。
- 私钥、文件字节、数据库句柄和密码学操作不会进入 WebView。
- 生产页面只加载应用内静态资源，日志不会记录剪贴板或文件内容。

## 下载与安装

当前正式产物支持：

| 平台 | 架构 | 安装包 | 更新方式 |
| --- | --- | --- | --- |
| macOS 13+ | Apple Silicon（ARM64） | ZIP 中的 `.app` | 应用内签名更新 |
| Ubuntu 24.04 | ARM64 | `.deb` | 签名 APT 系统更新 |
| Windows 10/11 | x64 | NSIS `.exe`、`.msi` | 应用内签名更新 |

请先在 [GitHub Releases](https://github.com/macji/synchalo/releases/latest) 下载当前平台的最新版。

### macOS

下载 `SyncHalo_<版本>_macos-arm64.zip`，解压后把 `SyncHalo.app` 移到“应用程序”目录并打开。正式包经过 Developer ID 签名和 Apple 公证。

如果系统仍阻止首次打开，请在 Finder 中右键 SyncHalo，选择“打开”并确认。

### Ubuntu：DEB（推荐）

先确认设备架构：

```bash
dpkg --print-architecture
```

当前安装包要求输出为 `arm64`。下载 DEB 后运行：

```bash
cd ~/Downloads
sudo apt install ./SyncHalo_*_ubuntu-arm64.deb
sudo apt update
```

DEB 会自动安装 SyncHalo 的公开 APT 签名密钥和 Deb822 软件源配置。以后 Ubuntu 软件更新器或以下命令会从签名源获取新版：

```bash
sudo apt update
sudo apt install --only-upgrade sync-halo
```

也可以不先下载 DEB，手动添加软件源后安装：

```bash
sudo install -d -m 0755 /etc/apt/keyrings
curl -fsSL https://macji.github.io/synchalo/apt/synchalo-archive-keyring.asc \
  | sudo tee /etc/apt/keyrings/synchalo-archive-keyring.asc >/dev/null
printf '%s\n' \
  'Types: deb' \
  'URIs: https://macji.github.io/synchalo/apt' \
  'Suites: stable' \
  'Components: main' \
  'Architectures: arm64' \
  'Signed-By: /etc/apt/keyrings/synchalo-archive-keyring.asc' \
  | sudo tee /etc/apt/sources.list.d/synchalo.sources >/dev/null
sudo apt update
sudo apt install sync-halo
```

首次单独安装下载的 DEB 时，图形安装器仍可能把它标记为未知来源；配置完成后，后续 APT 仓库元数据和软件包摘要均由 SyncHalo APT 密钥认证。

### Windows

下载 `SyncHalo_<版本>_windows-x64-setup.exe` 运行安装，或使用 MSI 进行系统部署。

SignPath Foundation 审核完成前，Windows 正式包暂未进行 Authenticode 签名，系统可能显示“未知发布者”或 SmartScreen 提醒。每个 Release 都提供 SHA-256 校验文件；请只从本仓库的 Releases 页面下载。

## 第一次使用

1. 确保两台设备处于同一局域网，且防火墙允许 SyncHalo 的本地网络通信。
2. 在已有设备的“设置”或“同步文件”页面生成一次性同步码。
3. 在另一台设备选择“加入”，输入同步码。
4. 回到已有设备确认新设备名称和平台。
5. 配对成功后即可同步文本；文件必须通过选择、拖放或在同步文件页面粘贴来显式发送。

Wayland 对后台全局剪贴板有更严格的限制。Ubuntu 上的实际能力取决于桌面 compositor 是否支持 data-control；能力不足时，应用会降级为仅窗口活跃或手动同步，文件同步不受此限制。

## 从源码开发

### 通用环境

- Git
- Node.js 22 或更高版本，推荐 Node.js 24
- Rust 1.88 或更高版本及 Cargo
- 当前平台对应的 Tauri 2 系统依赖

克隆并安装锁定依赖：

```bash
git clone https://github.com/macji/synchalo.git
cd synchalo
npm ci
```

运行使用演示数据的 Web UI：

```bash
npm run dev
```

运行包含 Rust 后端、真实发现和系统集成的桌面应用：

```bash
npm run tauri -- dev
```

仅限本地调试时可启用临时内存密钥：

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

没有 Developer ID 证书时可生成仅供本机测试的 ad-hoc 包：

```bash
APPLE_SIGNING_IDENTITY=- npm run tauri -- build \
  --target aarch64-apple-darwin \
  --bundles app,dmg \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

该构建没有 Apple 公证，不能作为正式发布包分发。

### Ubuntu ARM64 源码构建

请在 ARM64 Ubuntu 主机或 ARM64 GitHub runner 上安装依赖：

```bash
sudo apt update
sudo apt install -y \
  libappindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  patchelf \
  xdg-utils

rustup target add aarch64-unknown-linux-gnu
npm ci
npm run tauri -- build \
  --target aarch64-unknown-linux-gnu \
  --bundles deb \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

### Windows x64 源码构建

需要 Visual Studio 2022 Build Tools（Desktop development with C++）、WebView2、Node.js 和 Rust MSVC 工具链。在 Developer PowerShell 中运行：

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis,msi --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

自行构建的安装包不会包含 SyncHalo 的正式 Authenticode 或 Tauri 更新签名。

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

浏览器 UI 冒烟测试需要先启动 Vite：

```bash
npm run dev
```

然后在另一个终端运行：

```bash
python3 tests/e2e/ui_smoke.py
```

## 项目结构

```text
apps/desktop/src/        React / TypeScript 界面
apps/desktop/src-tauri/  Tauri commands 与桌面运行时
crates/core/             领域模型与事件语义
crates/network/          mDNS、配对、QUIC 与可信连接
crates/platform/         系统剪贴板与平台适配
crates/storage/          SQLite、迁移和本地加密
crates/transfer/         文件分块、续传与完整性校验
tests/e2e/               浏览器交互测试
tests/release/           发布与软件源测试
packaging/               Linux 软件源和安装包资源
```

更完整的产品、界面和发布说明见 [PRD.md](PRD.md)、[UI-DESIGN.md](UI-DESIGN.md) 和 [RELEASING.md](RELEASING.md)。

## 发布说明

正式标签通过 GitHub Actions 构建 Ubuntu ARM64 与 Windows x64，生成并发布签名 APT 仓库。macOS ARM64 在授权 Mac 上完成 Developer ID 签名、Apple 公证和 Tauri 更新签名，然后上传到同一个 GitHub Release。

构建和仓库中只包含公开验证密钥；APT 私钥、Tauri 更新私钥、Apple 证书及公证凭据不会进入源码或 Git 历史。完整流程见 [RELEASING.md](RELEASING.md)。
