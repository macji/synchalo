# SyncHalo

[中文介绍](README.zh-CN.md)

SyncHalo is a local-first clipboard and file synchronization tool for devices on the same local network. It requires no account, cloud storage, or public relay and transfers data only between devices that the user explicitly trusts.

[Download the latest release](https://github.com/macji/synchalo/releases/latest) · [APT repository](https://macji.github.io/synchalo/apt) · [Security policy](SECURITY.md) · [Product specification](PRD.md)

## Quick Install with AI

Copy the prompt below into Codex, Claude Code, or another trusted local coding agent. The agent should detect your operating system and architecture, use only official SyncHalo downloads, and verify the package before installation.

```text
Install the latest stable SyncHalo release from https://github.com/macji/synchalo/releases/latest on this computer. Detect the operating system and CPU architecture first. Use only assets or the signed APT repository published by macji/synchalo, verify the downloaded file against the release SHA-256 checksums, and do not bypass Gatekeeper, SmartScreen, package signatures, or other operating-system security checks. On macOS, install the notarized Apple Silicon app in /Applications. On Ubuntu, install the matching official DEB with APT so the signed SyncHalo repository is enrolled for future updates. On Windows, use the x64 setup executable and tell me before proceeding if its publisher signature cannot be verified. After installation, launch SyncHalo when possible and report the installed version, package source, and verification result.
```

## Features

- Automatically discovers devices on the local network through mDNS, with manual discovery and reconnection refresh.
- Pairs devices using a 60-second one-time sync code; an existing device must still approve each new device.
- Synchronizes plain-text clipboard content between trusted devices in real time while suppressing remote-write sync loops.
- Sends files through drag and drop, a native file picker, or in-page paste; when no destination is selected, all online devices receive the files.
- Supports streaming 4 MiB chunks with per-chunk BLAKE3 verification, verified-prefix resume, whole-file verification, peer-aware retry and cancellation, temporary files, and atomic commits.
- Provides clipboard and file history with search, favorites, repeat sync, and backend pagination.
- Lets users enable deletion sync and favorite sync independently; both are disabled by default.
- Supports English, Simplified Chinese, Traditional Chinese, Japanese, and Korean, follows the system language by default, and provides an immediate language switch in Settings.
- Includes tray residency, sync pause, launch at startup, a configurable receive folder, and device management.
- Checks for updates about five seconds after launch, every 30 minutes afterward, and on demand.
- Ignoring a version suppresses only startup and scheduled reminders; a manual check still shows it.
- macOS and Windows can download and verify updates in the background before requesting installation confirmation. Ubuntu requests administrator authorization after confirmation, then installs through the signed APT repository and restarts.

## Security and Privacy

- Clipboard content and files are never uploaded to a SyncHalo server. The data plane is limited to trusted devices on the local network.
- Pairing uses SPAKE2 password-authenticated key exchange. Trusted connections use QUIC, TLS 1.3, certificate pinning, and a device-signature challenge.
- Clipboard bodies are encrypted with XChaCha20-Poly1305 before being stored in local SQLite.
- A locally generated KEK is stored in `synchalo.key` with `0600` permissions. The database stores only wrapped data keys and encrypted identities.
- Private keys, file bytes, database handles, and cryptographic operations never enter the WebView.
- Production pages load only bundled static assets, and logs never include clipboard or file content.
- The Ubuntu administrator helper accepts only a constrained version string and can upgrade only the `sync-halo` package from the pinned signed repository; the application itself never gains root privileges.

## Download and Install

Current release artifacts support:

| Platform | Architecture | Package | Update path |
| --- | --- | --- | --- |
| macOS 13+ | Apple Silicon (ARM64) | `.app` in a ZIP archive | Signed in-app update |
| Ubuntu 24.04 | ARM64 and x86_64 (amd64) | `.deb` | In-app prompt, Polkit authorization, signed APT installation |
| Windows 10/11 | x64 | NSIS `.exe` and `.msi` | Signed in-app update |

Download the newest package for your platform from [GitHub Releases](https://github.com/macji/synchalo/releases/latest).

### macOS

Download `SyncHalo_<version>_macos-arm64.zip`, extract it, move `SyncHalo.app` to Applications, and open it. Official packages are signed with Developer ID and notarized by Apple.

If macOS still blocks the first launch, Control-click SyncHalo in Finder, select **Open**, and confirm.

### Ubuntu: DEB (recommended)

Check your architecture first:

```bash
dpkg --print-architecture
```

Packages are available for `arm64` and `amd64`. Download the matching DEB, then run:

```bash
cd ~/Downloads
sudo apt install ./SyncHalo_*_ubuntu-*.deb
sudo apt update
```

The DEB installs SyncHalo's public APT signing key, Deb822 source, constrained update helper, and Polkit policy. When an update is available, SyncHalo displays the version and release notes. Selecting **Update now** opens the Ubuntu administrator authorization dialog; APT verifies and installs the exact version before SyncHalo restarts.

You can also update through Ubuntu Software Updater or run:

```bash
sudo apt update
sudo apt install --only-upgrade sync-halo
```

To install directly from the repository without downloading a DEB first:

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

A graphical installer may identify the first standalone DEB as coming from an unknown source. After repository enrollment, SyncHalo's APT key authenticates all subsequent repository metadata and package hashes.

### Windows

Run `SyncHalo_<version>_windows-x64-setup.exe`, or use the MSI for managed deployment.

Until the SignPath Foundation review is complete, official Windows packages are temporarily unsigned and Windows may display an unknown-publisher or SmartScreen warning. Every release includes SHA-256 checksums; download installers only from this repository's Releases page.

## First Use

1. Make sure both devices are on the same local network and that the firewall allows SyncHalo's local network traffic.
2. Generate a one-time sync code from **Settings** or **File sync** on the existing device.
3. Select **Join** on the other device and enter the sync code.
4. Return to the existing device and approve the new device name and platform.
5. After pairing, text synchronizes automatically. Files must be sent explicitly by choosing, dropping, or pasting them on the File sync page.

Wayland applies stricter limits to background global clipboard access. Actual capability on Ubuntu depends on whether the desktop compositor supports data-control. When it does not, SyncHalo falls back to active-window-only or manual clipboard sync; file sync is unaffected.

## Develop from Source

### Common requirements

- Git
- Node.js 22 or newer; Node.js 24 is recommended
- Rust 1.88 or newer and Cargo
- The Tauri 2 system dependencies for your platform

Clone the repository and install locked dependencies:

```bash
git clone https://github.com/macji/synchalo.git
cd synchalo
npm ci
```

Run the Web UI with demo data:

```bash
npm run dev
```

Run the complete desktop application with the Rust backend, real discovery, and system integration:

```bash
npm run tauri -- dev
```

For local debugging only, you can enable ephemeral in-memory keys:

```bash
SYNCHALO_EPHEMERAL_KEYS=1 npm run tauri -- dev
```

This mode does not persist clipboard history or device trust and must never be used for release builds.

### macOS source build

Install Xcode Command Line Tools:

```bash
xcode-select --install
rustup target add aarch64-apple-darwin
```

Without a Developer ID certificate, you can create an ad-hoc package for local testing only:

```bash
APPLE_SIGNING_IDENTITY=- npm run tauri -- build \
  --target aarch64-apple-darwin \
  --bundles app,dmg \
  --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

This build is not notarized by Apple and must not be distributed as an official release.

### Ubuntu source build

Install dependencies on an Ubuntu 24.04 ARM64 or x86_64 host:

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

GitHub Actions uses native `ubuntu-24.04-arm` and `ubuntu-24.04` runners for ARM64 and x86_64 DEBs respectively; it does not use cross-architecture emulation.

### Windows x64 source build

Install Visual Studio 2022 Build Tools with **Desktop development with C++**, WebView2, Node.js, and the Rust MSVC toolchain. Then run in Developer PowerShell:

```powershell
rustup target add x86_64-pc-windows-msvc
npm ci
npm run tauri -- build --target x86_64-pc-windows-msvc --bundles nsis,msi --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

Locally built installers do not contain SyncHalo's official Authenticode or Tauri update signatures.

## Test and Verify

Run before committing:

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

For the browser UI smoke test, start Vite first:

```bash
npm run dev
```

Then run in another terminal:

```bash
python3 tests/e2e/ui_smoke.py
```

## Project Structure

```text
apps/desktop/src/        React and TypeScript UI
apps/desktop/src-tauri/  Tauri commands and desktop runtime
crates/core/             Domain models and event semantics
crates/network/          mDNS, pairing, QUIC, and trusted connections
crates/platform/         System clipboard and platform adapters
crates/storage/          SQLite, migrations, and local encryption
crates/transfer/         File chunking, resume, and integrity verification
tests/e2e/               Browser interaction tests
tests/release/           Release and package-repository tests
packaging/               Linux repository and package assets
```

See [PRD.md](PRD.md) and [RELEASING.md](RELEASING.md) for the full product and release documentation.

## Releases

Release tags trigger parallel GitHub Actions builds for Ubuntu ARM64, Ubuntu x86_64, and Windows x64, and publish the signed dual-architecture APT repository. macOS ARM64 is built on an authorized Mac, signed with Developer ID, notarized by Apple, signed for Tauri updates, and uploaded to the same GitHub Release.

Only public verification keys are stored in builds and the repository. APT private keys, Tauri update private keys, Apple certificates, and notarization credentials never enter source control or Git history. See [RELEASING.md](RELEASING.md) for the complete process.

## License

MIT. See [LICENSE](LICENSE).
