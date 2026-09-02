# Desktop Release Deployment

SyncHalo publishes Linux and Windows packages through `.github/workflows/release.yml`. A pushed version tag is resolved to an immutable commit SHA, then shared source verification and both platform builds run in parallel. Publishing waits for every job to pass, verifies the artifacts, generates `SHA256SUMS.txt`, and creates or updates the matching GitHub Release. macOS remains a local Developer ID build and notarization workflow.

## Release Outputs

| Platform             | Runner             | Assets                    |
| -------------------- | ------------------ | ------------------------- |
| Ubuntu Desktop ARM64 | `ubuntu-24.04-arm` | `.deb`, `.AppImage`       |
| Windows x64          | `windows-latest`   | `.msi`, NSIS `-setup.exe` |

Linux does not require a certificate for these packages. Windows signing is optional; without its certificate secrets, the workflow publishes unsigned installers and records that status in the release notes. Unsigned Windows builds can trigger an “Unknown publisher” or Microsoft Defender SmartScreen warning.

## Required GitHub Secrets

Open **Settings → Secrets and variables → Actions → New repository secret**.

No Secrets are required to build Linux or unsigned Windows packages. For optional Windows Authenticode signing, configure both:

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

macOS is intentionally excluded from GitHub Actions. Build it on the authorized Mac with the installed `Developer ID Application` identity, submit it through the `SyncHaloNotary` keychain profile, staple the accepted ticket, and replace the Git-ignored local output under `release/macos-arm64/` according to `AGENTS.md`. Apple credentials, certificates, and generated release packages stay out of Git history.

## Create a Release

Keep these versions identical before tagging:

- `Cargo.toml`
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/tauri.conf.json`

Commit and push the version change, then validate without creating a tag:

```bash
scripts/release.sh 0.1.0 --dry-run
```

Trigger the release:

```bash
scripts/release.sh 0.1.0
```

The script requires a clean `main` branch that exactly matches `origin/main`, creates an annotated `v0.1.0` tag, and pushes it. Alternatively, run **Publish Desktop Release** from the Actions page and provide an existing version tag.

## Private Repository Access

GitHub Releases inherit repository visibility. Because `macji/synchalo` is private, only authenticated users who already have repository access can view or download its releases. There is no setting that makes only a release public while keeping its repository private.

For public downloads while retaining private source code, publish the same assets to a separate public repository such as `macji/synchalo-downloads`, or upload them to object storage/CDN. Doing that from Actions requires a narrowly scoped token for the destination repository; the built-in `GITHUB_TOKEN` only has access to its current repository.

References: [GitHub Releases access](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases), [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/).
