# SyncHalo 中文介绍

[English README](README.md)

SyncHalo 是一款本地优先的局域网剪贴板与文件同步工具。它不需要账号、云盘或公网中继，只在用户明确配对的可信设备之间传输数据。

[下载最新版本](https://github.com/macji/synchalo/releases/latest) · [APT 软件源](https://macji.github.io/synchalo/apt) · [安全策略](SECURITY.md) · [产品规格](PRD.md)

## 使用 AI 快速安装

把下面的提示词直接复制给 Codex、Claude Code 或其他可信的本地编程 Agent。它会先识别系统与处理器架构，再从 SyncHalo 官方发布页选择并校验正确的安装包。

```text
请在这台电脑上安装 https://github.com/macji/synchalo/releases/latest 提供的最新版稳定版 SyncHalo。先识别操作系统和 CPU 架构；只允许使用 macji/synchalo 官方 Release 资产或官方签名 APT 软件源；下载后必须根据 Release 中的 SHA-256 校验文件验证安装包；不得绕过 Gatekeeper、SmartScreen、软件包签名或其他系统安全检查。macOS 请把已公证的 Apple Silicon 应用安装到 /Applications；Ubuntu 请用 APT 安装匹配架构的官方 DEB，使 SyncHalo 签名软件源同时完成注册，以便后续更新；Windows 请使用 x64 setup 安装程序，如果无法验证发布者签名，继续前必须先告诉我。安装完成后尽可能启动 SyncHalo，并报告已安装版本、软件包来源和校验结果。
```

## 主要功能

- 通过 mDNS 自动发现同一局域网内的设备，也可以手动刷新发现与重连状态。
- 使用 60 秒一次性同步码配对，新设备加入前仍需已有设备确认。
- 在可信设备间实时同步纯文本剪贴板，并抑制远端写入造成的同步回环。
- 支持拖放、原生文件选择器和页面内粘贴发送文件；未指定目标时发送给全部在线设备。
- 文件传输支持流式传输、断点续传、BLAKE3 完整性校验、临时文件和原子提交。
- 提供剪贴板历史、文件历史、搜索、收藏、再次同步和后端分页。
- 支持英语、简体中文、繁体中文、日语和韩语，默认跟随系统语言，也可在设置中即时切换。
- 支持系统托盘、暂停同步、开机启动、自定义接收目录和可信设备管理。
- 启动约 5 秒后检查更新，之后每 30 分钟检查一次，也可手动检查。

## 下载与安装

| 平台 | 架构 | 安装包 | 更新方式 |
| --- | --- | --- | --- |
| macOS 13+ | Apple Silicon（ARM64） | ZIP 中的 `.app` | 应用内签名更新 |
| Ubuntu 24.04 | ARM64、x86_64（amd64） | `.deb` | 应用提醒、Polkit 授权、签名 APT 安装 |
| Windows 10/11 | x64 | NSIS `.exe`、`.msi` | 应用内更新 |

### macOS

从 [GitHub Releases](https://github.com/macji/synchalo/releases/latest) 下载 `SyncHalo_<版本>_macos-arm64.zip`，解压后把 `SyncHalo.app` 移到“应用程序”目录。正式包经过 Developer ID 签名和 Apple 公证。

### Ubuntu

先检查设备架构：

```bash
dpkg --print-architecture
```

下载对应的 `arm64` 或 `amd64` DEB，然后安装：

```bash
cd ~/Downloads
sudo apt install ./SyncHalo_*_ubuntu-*.deb
sudo apt update
```

首次安装会自动注册 SyncHalo 的公开 APT 签名密钥、Deb822 软件源、受限更新 helper 和 Polkit 策略。之后应用会自动发现新版；用户点击“立即更新”并完成 Ubuntu 管理员授权后，APT 会验证并安装指定版本，再自动重启 SyncHalo。

### Windows

下载并运行 `SyncHalo_<版本>_windows-x64-setup.exe`，也可以使用 MSI 进行系统部署。安装前请核对 Release 中的 SHA-256 校验值和发布者签名状态。

## 第一次使用

1. 确保两台设备处于同一局域网，且防火墙允许 SyncHalo 的本地网络通信。
2. 在已有设备的“设置”或“同步文件”页面生成一次性同步码。
3. 在另一台设备选择“加入”，输入同步码。
4. 回到已有设备确认新设备名称和平台。
5. 配对成功后即可自动同步文本；文件需要通过选择、拖放或页面内粘贴显式发送。

## 安全与隐私

- 剪贴板和文件不会上传到 SyncHalo 服务器，数据只在局域网可信设备之间传输。
- 配对使用 SPAKE2；可信连接使用 QUIC、TLS 1.3、证书固定和设备签名挑战。
- 剪贴板正文以 XChaCha20-Poly1305 加密后保存在本机 SQLite 中。
- 本地 KEK 保存在权限为 `0600` 的 `synchalo.key`，数据库只保存包装后的数据密钥和加密身份。
- 私钥、文件字节、数据库句柄和密码学操作不会进入 WebView。

## 从源码运行

需要 Node.js 22+（推荐 24）、Rust 1.88+ 和当前平台对应的 Tauri 2 系统依赖：

```bash
git clone https://github.com/macji/synchalo.git
cd synchalo
npm ci
npm run tauri -- dev
```

完整开发、验证与发布说明见 [英文 README](README.md)、[PRD.md](PRD.md) 和 [RELEASING.md](RELEASING.md)。

## 许可证

MIT，详见 [LICENSE](LICENSE)。
