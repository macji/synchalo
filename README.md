# SyncHalo

SyncHalo 是一个本地优先的局域网粘贴板与文件同步工具。桌面端使用 Tauri 2 + React/TypeScript，发现、配对、加密存储和文件数据面全部由 Rust 实现。

## 当前实现

- 左右分栏桌面 UI：粘贴板、同步文件、设置。
- 跟随系统浅色/深色主题，键盘导航和可撤销单条删除。
- SQLite 3.53 bundled 存储；Rust 生成的 KEK 保存在权限为 `0600` 的 `synchalo.key`，SQLite 保存 wrapped DEK 与加密设备身份，粘贴板正文使用 XChaCha20-Poly1305 字段加密。
- macOS 与 Linux 文本粘贴板监听和远端回声抑制。
- mDNS 局域网发现。
- 60 秒一次性同步码、SPAKE2 密码认证密钥交换。
- 新设备通过同步码后，已有设备仍需确认设备名称和平台才能授权。
- 自签名 TLS 证书固定与 Ed25519 设备挑战，可信会话使用 QUIC/TLS 1.3。
- 签名文本事件同步。
- 文件流传输、连续 offset 续传、BLAKE3 校验、临时文件和原子提交。
- 同步文件页采用设备、发送、历史三分区，支持同步码、页面内 `Ctrl/Cmd + V` 自动同步、历史再次同步、持久化收藏和每页 100 条后端分页。
- Tauri 状态栏/托盘图标、点击激活窗口、开机启动设置和系统文件选择器。
- macOS ARM64 与 Ubuntu ARM64 原生 CI 构建。

## 直接运行编译产物

已编译的产物按平台放在 [`release/`](release/)：

```text
release/
├── macos-arm64/             # 已生成 SyncHalo.app 和 ZIP
└── ubuntu-24.04-arm64/      # Ubuntu ARM64 原生构建脚本与说明
```

macOS Apple Silicon：

```bash
open release/macos-arm64/SyncHalo.app
```

详细说明与 Ubuntu ARM64 构建/运行方法见 [`release/README.md`](release/README.md)。

## 开发环境

需要：

- Node.js 22 或更高版本；推荐 Node.js 24。
- Rust 1.88 或更高版本。
- macOS 13+，或 Ubuntu 24.04 ARM64。

Ubuntu 依赖：

```bash
sudo apt-get update
sudo apt-get install -y \
  file \
  libappindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev \
  patchelf \
  xdg-utils
```

安装与运行：

```bash
npm ci
npm run tauri -- dev
```

仅在本地调试时可使用临时内存密钥：

```bash
SYNCHALO_EPHEMERAL_KEYS=1 npm run tauri -- dev
```

该模式不会把粘贴板历史或设备信任写入磁盘，不能用于发布包。

只运行 Web UI（使用内置演示数据）：

```bash
npm run dev
```

## 验证

```bash
npm run build
npm run lint
npm test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

本地浏览器交互与截图：

```bash
mkdir -p artifacts/ui
npm run dev
# 在另一个终端运行：
python3 tests/e2e/ui_smoke.py
```

## 构建

macOS ARM64：

```bash
npm run tauri -- build --target aarch64-apple-darwin --bundles app,dmg
```

Ubuntu ARM64 必须在 ARM64 Linux 主机或 runner 上构建：

```bash
npm run tauri -- build --target aarch64-unknown-linux-gnu --bundles deb,appimage
```

macOS 不能直接链接 Ubuntu 的 WebKitGTK、GTK 和 AppIndicator 运行库，因此仓库使用 `ubuntu-24.04-arm` 原生 runner，而不是在 Mac 上伪交叉构建 Linux 安装包。

## 安全边界

- WebView 不接收文件字节、私钥或数据库句柄。
- 生产页面只加载随应用打包的静态资源，CSP 禁止远程脚本。
- KEK 保存在当前用户独占的 `synchalo.key`；SQLite 只保存 wrapped DEK 和加密后的传输身份。正常启动不访问 Keychain/Secret Service。
- 从旧版本升级时会最后读取一次 Keychain/Secret Service；验证迁移并生成 `synchalo.keychain-migration-backup.db` 后删除旧项，后续启动完全绕过系统钥匙串。
- 安全存储不可用时，应用使用内存数据库，不把敏感历史以弱保护方式写盘。

完整产品定义见 [PRD.md](PRD.md)，UI 规范见 [UI-DESIGN.md](UI-DESIGN.md)，阶段计划见 [IMPLEMENTATION-PLAN.md](IMPLEMENTATION-PLAN.md)。

## 自动发布

版本 Tag 会通过 GitHub Actions 构建 Ubuntu Desktop ARM64 和 Windows x64，并发布到 GitHub Releases。macOS 继续在授权 Mac 上进行 Developer ID 签名和 Apple 公证。产物列表、可选 Windows 签名和发版命令见 [RELEASING.md](RELEASING.md)。
