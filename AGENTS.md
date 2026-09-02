# Repository Guidelines

## Project Structure & Module Organization

SyncHalo is a Tauri 2 application. React/TypeScript lives in `apps/desktop/src/`; Tauri commands and runtime code live in `apps/desktop/src-tauri/`. Rust is split across `crates/core`, `network`, `platform`, `storage`, and `transfer`. Tests are in `tests/e2e/`, screenshots in `artifacts/ui/`, packages in `release/`, and specifications in `PRD.md` and `UI-DESIGN.md`.

Keep private keys, database handles, file bytes, and cryptographic operations in Rust. The WebView should receive only bounded view models and command results.

## Build, Test, and Development Commands

- `npm ci`: install locked Node.js dependencies.
- `npm run dev`: run the mock-data Web UI at `http://127.0.0.1:1420`.
- `npm run tauri -- dev`: run the complete desktop app.
- `npm run build` / `npm run lint` / `npm test`: build, lint, and test the frontend.
- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings`: verify Rust style.
- `cargo test --workspace`: run Rust tests.
- `npm run tauri -- build`: create a native host-platform bundle.

## Release Artifact Policy

Before native builds, inspect and stop any running SyncHalo process from this repository. After a successful build, replace canonical artifacts under `release/<platform>/` and remove stale numbered app copies. For macOS releases, build locally with the `Developer ID Application` identity, submit with the `SyncHaloNotary` keychain profile, staple the accepted ticket, and verify `codesign`, `stapler`, and `spctl`; macOS signing material must never enter GitHub Actions. Stage the fresh bundle, then replace `SyncHalo.app` as a whole; never merge-copy into the existing bundle because that preserves a stale Finder modification date. Rebuild its ZIP, verify archive integrity, then regenerate `SHA256SUMS`. Versioned Linux and Windows GitHub releases must use `.github/workflows/release.yml` and the tag checks documented in `RELEASING.md`; never place signing material in the repository. Unsigned Windows artifacts must be identified in release notes. Never publish failed or unverified output.

## Coding Style & Naming Conventions

Use two-space indentation for TypeScript, JSX, and CSS; let `rustfmt` format Rust. React components use `PascalCase`, functions use `camelCase`, CSS classes use kebab-case, and Rust modules/functions use `snake_case`. Keep code within existing module boundaries. Document any necessary lint suppression.

## Testing Guidelines

Frontend tests use Vitest and Testing Library with `*.test.tsx` names. Rust tests live in local `#[cfg(test)] mod tests`. Run `python3 tests/e2e/ui_smoke.py` beside Vite for browser checks. Add regression tests for behavior changes, especially migrations, encryption, file integrity, and platform fallbacks.

## Commit & Pull Request Guidelines

Use concise Conventional Commit-style subjects, such as `feat(storage): wrap data keys in sqlite`. Pull requests should describe scope, platforms, migration/security implications, and validation commands. Include UI screenshots and linked issues. Update `release/` only intentionally.

After every completed modification, run the relevant frontend and/or Rust tests before committing. Commit each verified task with a focused Conventional Commit message; never commit failing output, local databases, key files, secrets, or unverified release artifacts. When `origin` is configured and the user has requested synchronization, push the verified commit to `origin/main`.

## Security & Configuration Tips

Never commit secrets or log clipboard/file contents. Keep the generated KEK in the user-only `synchalo.key` file with mode `0600`; SQLite stores only wrapped DEKs and encrypted transport credentials. Keychain access is permitted only in the one-time legacy migration path. `SYNCHALO_EPHEMERAL_KEYS=1` is for local debugging only and must not be used for release builds.
