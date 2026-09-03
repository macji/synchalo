# Desktop Release Deployment

SyncHalo publishes Linux and Windows packages through `.github/workflows/release.yml`. A pushed version tag is resolved to an immutable commit SHA, then shared source verification and both platform builds run in parallel. macOS is built, signed, and notarized on the authorized Mac, then uploaded to the same GitHub Release by `scripts/release-macos.sh`. All three platforms must resolve to the same tag commit.

Manual workflow runs accept a source `ref` and a `target` choice. The ref can be a branch, tag, or commit for `linux`, `windows`, and `validate-only`; `all` requires the matching version tag. Single-platform builds retain the result as a seven-day Actions artifact, while `validate-only` runs only the shared source checks. Tag pushes always behave as `all`.

## Release Outputs

| Platform             | Runner             | Assets                    |
| -------------------- | ------------------ | ------------------------- |
| Ubuntu Desktop ARM64 | `ubuntu-24.04-arm` | `.deb`, `.AppImage`       |
| Windows x64          | `windows-latest`   | `.msi`, NSIS `-setup.exe` |
| macOS ARM64          | Authorized Mac     | notarized `.app.zip`      |

Linux does not require a certificate for these packages. Windows signing is optional; without its certificate secrets, the workflow publishes unsigned installers and records that status in the release notes. Unsigned Windows builds can trigger an “Unknown publisher” or Microsoft Defender SmartScreen warning.

## Required GitHub Secrets

Open **Settings → Secrets and variables → Actions → New repository secret**.

Signed updater artifacts require both of these repository Actions secrets. They are a dedicated minisign key and are not Apple signing material:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

For optional Windows Authenticode signing, configure both:

- `WINDOWS_CERTIFICATE`: base64 of a code-signing `.pfx`.
- `WINDOWS_CERTIFICATE_PASSWORD`: the `.pfx` export password.

In PowerShell, encode the certificate with:

```powershell
[Convert]::ToBase64String(
  [IO.File]::ReadAllBytes("certificate.pfx")
) | Set-Clipboard
```

Never commit certificates, private keys, or passwords.

## macOS Local Release

macOS is intentionally excluded from GitHub Actions. `scripts/release-macos.sh` uses the installed `Developer ID Application` identity, the `SyncHaloNotary` keychain profile, and the encrypted updater key at `~/.config/synchalo/updater-signing.key`. It verifies, staples, and stages the canonical local bundle before uploading versioned macOS assets and the three-platform `latest.json`. Apple credentials and certificates stay out of GitHub and Git history.

## Create a Release

Keep these versions identical before tagging:

- `Cargo.toml`
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/tauri.conf.json`
- `package-lock.json`

Commit and push the version change, then validate without creating a tag:

```bash
scripts/release.sh 0.1.2 --dry-run
```

Trigger the release:

```bash
scripts/release-all.sh 0.1.2
```

For a faster non-publishing platform build, open **Actions → Publish Linux and Windows Release → Run workflow**, set `ref` to `main` (or another branch, tag, or commit), then choose `linux`, `windows`, or `validate-only`. Choose `all` only with a matching version tag when the manual run should create or update the complete GitHub Release.

The scripts require a clean `main` branch that exactly matches `origin/main`. `release-all.sh` creates and pushes the annotated version tag, starts the Linux/Windows workflow, builds macOS locally in parallel, waits for GitHub to succeed, uploads macOS, and verifies remote asset digests. Alternatively, run **Publish Linux and Windows Release** from the Actions page and provide a source ref plus target.

## Automatic Updates

The public repository exposes `https://github.com/macji/synchalo/releases/latest/download/latest.json`. Production builds check this manifest five seconds after startup. Updates are verified with the embedded public updater key before installation; macOS and Windows restart automatically, while Linux automatic installation is enabled only for AppImage launches. DEB installations continue to update through APT or manual package installation.

References: [GitHub Releases access](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases), [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/).
