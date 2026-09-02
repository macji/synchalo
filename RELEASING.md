# Desktop Release Deployment

SyncHalo publishes versioned desktop packages through `.github/workflows/release.yml`. A pushed version tag builds every platform from the same commit, verifies the artifacts, generates `SHA256SUMS.txt`, and creates or updates the matching GitHub Release.

## Release Outputs

| Platform             | Runner             | Assets                       |
| -------------------- | ------------------ | ---------------------------- |
| macOS ARM64          | `macos-15`         | notarized `.app.zip`, `.dmg` |
| Ubuntu Desktop ARM64 | `ubuntu-24.04-arm` | `.deb`, `.AppImage`          |
| Windows x64          | `windows-latest`   | `.msi`, NSIS `-setup.exe`    |

macOS publishing fails closed when signing or notarization secrets are missing. Windows signing is optional; without its certificate secrets, the workflow publishes unsigned installers and records that status in the release notes.

## Required GitHub Secrets

Open **Settings → Secrets and variables → Actions → New repository secret**.

For macOS, configure all of:

- `APPLE_CERTIFICATE`: base64 of the exported Developer ID Application `.p12`.
- `APPLE_CERTIFICATE_PASSWORD`: password assigned while exporting the `.p12`.
- `APPLE_KEYCHAIN_PASSWORD`: a random temporary CI keychain password.
- `APPLE_ID`: Apple Developer account email.
- `APPLE_PASSWORD`: Apple app-specific password for notarization.
- `APPLE_TEAM_ID`: Developer Team ID, currently `39UVPY4WQL`.

Export the `.p12` from Keychain Access under **My Certificates**, then encode it on macOS:

```bash
openssl base64 -A -in DeveloperID.p12 | pbcopy
```

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
