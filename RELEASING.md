# Desktop Release Deployment

SyncHalo publishes Linux and Windows packages through `.github/workflows/release.yml`. A pushed version tag is resolved to an immutable commit SHA, then shared source verification and both platform builds run in parallel. Formal Windows installers must be Authenticode-signed by SignPath Foundation, and the Ubuntu DEB must be published through the signed GitHub Pages APT repository before GitHub Release publication. macOS is built, signed, and notarized on the authorized Mac, then uploaded to the same GitHub Release by `scripts/release-macos.sh`. All three platforms must resolve to the same tag commit.

Manual workflow runs accept a source `ref` and a `target` choice. The ref can be a branch, tag, or commit for `linux`, `windows`, and `validate-only`; `all` requires the matching version tag. Single-platform builds retain the result as a seven-day Actions artifact, while `validate-only` runs only the shared source checks. Tag pushes always behave as `all`.

## Release Outputs

| Platform             | Runner             | Assets                    |
| -------------------- | ------------------ | ------------------------- |
| Ubuntu Desktop ARM64 | `ubuntu-24.04-arm` | `.deb`                    |
| Windows x64          | `windows-latest`   | `.msi`, NSIS `-setup.exe` |
| macOS ARM64          | Authorized Mac     | notarized `.app.zip`      |

The repository variable `WINDOWS_SIGNING_MODE` makes Windows release intent explicit. Set it to `signpath` for SignPath Foundation Authenticode signing, or temporarily to `unsigned` while the OSS application is under review. Unsigned release notes always disclose the warning. Manual `windows` builds remain unsigned short-lived Actions artifacts and never update a GitHub Release.

Ubuntu repository metadata is signed by the dedicated SyncHalo APT key. The private key is stored only as an encrypted local backup and GitHub Actions secrets; GitHub Pages contains the public key, signed metadata, and the current ARM64 DEB. The public ASCII key, Deb822 source, restricted update helper, and Polkit policy are embedded in every DEB. The desktop process remains unprivileged; after explicit confirmation, `pkexec` authorizes only `/usr/lib/synchalo/update-synchalo`, which validates a bounded version and asks APT to install exactly `sync-halo=<version>` from the signed source. The DEB post-install migration removes the exact legacy `synchalo.list` created by older official instructions, but preserves any customized file. The release build verifies that this pinned public key matches the private release key. A downloaded standalone DEB can still be described as an unknown source during that first installation; subsequent APT metadata and packages are authenticated by the enrolled source.

## Required GitHub Secrets

Open **Settings → Secrets and variables → Actions → New repository secret**.

Signed updater artifacts require both of these repository Actions secrets. They are a dedicated minisign key and are not Apple or Authenticode signing material:

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`

SignPath Foundation requires:

- `SIGNPATH_API_TOKEN`: token for a SignPath submitter.
- `SIGNPATH_ORGANIZATION_ID`: SignPath organization ID.

The SignPath project, policy, and artifact configuration slugs are fixed by the workflow as `synchalo`, `release-signing`, and `windows-installers`. Complete the one-time setup in [`signpath/README.md`](signpath/README.md). Every formal request requires manual approval in SignPath.

While SignPath review is pending, explicitly select the temporary mode:

```bash
gh variable set WINDOWS_SIGNING_MODE --repo macji/synchalo --body unsigned
```

After approval and secret configuration, enable signing without changing the workflow:

```bash
gh variable set WINDOWS_SIGNING_MODE --repo macji/synchalo --body signpath
```

The signed APT repository requires:

- `APT_GPG_PRIVATE_KEY`: password-protected ASCII-armored secret key.
- `APT_GPG_PASSPHRASE`: key passphrase.
- `APT_GPG_FINGERPRINT`: repository Actions variable containing the full public fingerprint.

After committing a clean setup revision, generate and configure these without printing the passphrase:

```bash
scripts/setup-apt-signing.sh macji/synchalo
```

The encrypted private-key backup defaults to `~/.config/synchalo/apt-signing/`; on macOS its passphrase is also stored in Keychain under service `io.synchalo.desktop.release` and account `apt-signing-key-password-v1`. Keep an offline backup. Never run the setup script again merely to deploy a new version because changing the key would break existing clients.

Enable **Settings → Pages → Build and deployment → Source: GitHub Actions** once. The release workflow then publishes:

```text
https://macji.github.io/synchalo/apt
```

Only the public APT key at `packaging/apt/synchalo-archive-keyring.asc` is committed. Never commit certificates, private keys, or passwords.

## macOS Local Release

macOS is intentionally excluded from GitHub Actions. `scripts/release-macos.sh` uses the installed `Developer ID Application` identity, the `SyncHaloNotary` keychain profile, and the encrypted updater key at `~/.config/synchalo/updater-signing.key`. It verifies, staples, and stages the canonical local bundle before uploading versioned macOS assets and the macOS/Windows `latest.json`. Ubuntu DEB updates remain under APT rather than the Tauri updater. Apple credentials and certificates stay out of GitHub and Git history.

## Create a Release

Keep these versions identical before tagging:

- `Cargo.toml`
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/tauri.conf.json`
- `package-lock.json`

Commit and push the version change, then validate without creating a tag:

```bash
scripts/release.sh 0.1.7 --dry-run
```

Trigger the release:

```bash
scripts/release-all.sh 0.1.7
```

For a faster non-publishing platform build, open **Actions → Publish Linux and Windows Release → Run workflow**, set `ref` to `main` (or another branch, tag, or commit), then choose `linux`, `windows`, or `validate-only`. Choose `all` only with a matching version tag when the manual run should create or update the complete GitHub Release.

The scripts require a clean `main` branch that exactly matches `origin/main`. `release-all.sh` creates and pushes the annotated version tag, starts the Linux/Windows workflow, builds macOS locally in parallel, waits for GitHub and SignPath approval, uploads macOS, and verifies remote asset digests. When the Windows job shows a pending SignPath request, an authorized maintainer must review and approve it in SignPath before the one-hour workflow timeout. Alternatively, run **Publish Linux and Windows Release** from the Actions page and provide a source ref plus target.

## Automatic Updates

The public repository exposes `https://github.com/macji/synchalo/releases/latest/download/latest.json`. Production builds check its bounded version metadata about five seconds after startup and every 30 minutes afterward, and Settings provides a manual check. Ignoring a version suppresses only automatic checks; a manual check always shows the available version again. On macOS and Windows, disabling automatic updates shows notes before download, while enabling it downloads and verifies into a private temporary file before asking to install and restart. Ubuntu never treats the DEB as a Tauri updater payload: it shows the same release notes, then uses Polkit and the packaged helper to run an exact-version APT upgrade after user confirmation. APT authenticates repository metadata and package hashes before the app restarts. Concurrent operations are collapsed into one. Windows updater signatures are regenerated after SignPath because Authenticode changes installer bytes.

References: [GitHub Releases access](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases), [Tauri GitHub pipeline](https://v2.tauri.app/distribute/pipelines/github/).
