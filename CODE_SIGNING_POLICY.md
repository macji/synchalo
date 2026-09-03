# SyncHalo Code Signing Policy

## Scope

This policy applies to executable artifacts published by the SyncHalo project.
The repository is public and licensed under `MIT OR Apache-2.0`.

## Authorized release process

1. Release source must be committed to `main` and identified by an annotated
   `v*` tag whose version matches every Cargo, npm, and Tauri manifest.
2. Frontend tests, Rust tests, lint, formatting, Clippy, and native package
   checks must pass on GitHub-hosted runners before publication.
3. Windows MSI and NSIS installers are built from the tagged commit by GitHub
   Actions and submitted as GitHub workflow artifacts to SignPath.
4. Every SignPath Foundation signing request requires manual approval by a
   project maintainer with two-factor authentication enabled.
5. The workflow accepts the result only when both installers have valid
   Authenticode signatures issued to SignPath Foundation. Tauri updater
   signatures are generated after Authenticode signing.
6. macOS artifacts are signed and notarized only on the authorized local Mac;
   Apple signing material is never uploaded to GitHub Actions.
7. Ubuntu repository metadata is signed by the dedicated SyncHalo APT archive
   key stored as an encrypted local backup and GitHub Actions secrets.

Manual single-platform builds are test artifacts and are not official
releases. They may be unsigned and must not be redistributed as official
SyncHalo packages.

## Access and incident response

Maintainers use two-factor authentication for GitHub and SignPath. Signing
credentials are never committed to source control. Access is limited to the
release workflow and is removed when no longer required.

If source, build infrastructure, or a signing credential is suspected to be
compromised, maintainers stop releases, remove affected artifacts, revoke or
rotate the credential, investigate the tagged source and build provenance, and
publish a security advisory before resuming signing.
