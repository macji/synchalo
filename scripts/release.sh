#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release.sh <version|tag> [--dry-run]

Examples:
  scripts/release.sh 0.1.9 --dry-run
  scripts/release.sh v0.1.9

The version must already match Cargo.toml, package.json,
apps/desktop/package.json, and apps/desktop/src-tauri/tauri.conf.json.
The non-dry-run command creates and pushes an annotated version tag, which
triggers .github/workflows/release.yml.
EOF
}

if [[ $# -lt 1 || $# -gt 2 ]]; then
  usage >&2
  exit 2
fi

requested="$1"
dry_run=false
if [[ ${2:-} == "--dry-run" ]]; then
  dry_run=true
elif [[ -n ${2:-} ]]; then
  usage >&2
  exit 2
fi

if [[ "$requested" == v* ]]; then
  tag="$requested"
else
  tag="v$requested"
fi
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$ ]]; then
  echo "Invalid release version: $requested" >&2
  exit 1
fi
version="${tag#v}"

for command in git jq sed; do
  if ! command -v "$command" >/dev/null; then
    echo "Required command is unavailable: $command" >&2
    exit 1
  fi
done

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "Releases must be created from the main branch." >&2
  exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "The working tree must be clean before creating a release tag." >&2
  git status --short >&2
  exit 1
fi

tauri_version="$(jq -r '.version' apps/desktop/src-tauri/tauri.conf.json)"
root_npm_version="$(jq -r '.version' package.json)"
desktop_npm_version="$(jq -r '.version' apps/desktop/package.json)"
lock_version="$(jq -r '.version' package-lock.json)"
lock_root_version="$(jq -r '.packages[""] | .version' package-lock.json)"
lock_desktop_version="$(jq -r '.packages["apps/desktop"] | .version' package-lock.json)"
cargo_version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' Cargo.toml)"

for declared in \
  "$tauri_version" \
  "$root_npm_version" \
  "$desktop_npm_version" \
  "$lock_version" \
  "$lock_root_version" \
  "$lock_desktop_version" \
  "$cargo_version"; do
  if [[ "$declared" != "$version" ]]; then
    echo "Version mismatch: tag is $tag but a project file declares $declared" >&2
    exit 1
  fi
done

git fetch origin main --tags
local_commit="$(git rev-parse HEAD)"
remote_commit="$(git rev-parse origin/main)"
if [[ "$local_commit" != "$remote_commit" ]]; then
  echo "Local main must exactly match origin/main before releasing." >&2
  echo "local:  $local_commit" >&2
  echo "remote: $remote_commit" >&2
  exit 1
fi
if git rev-parse --verify "refs/tags/$tag" >/dev/null 2>&1 \
  || git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null 2>&1; then
  echo "Release tag already exists: $tag" >&2
  exit 1
fi

echo "Release tag: $tag"
echo "Commit:      $local_commit"
echo "Workflow:    .github/workflows/release.yml"
if [[ "$dry_run" == true ]]; then
  echo "Dry run passed; no tag was created or pushed."
  exit 0
fi

git tag -a "$tag" -m "SyncHalo $tag"
git push origin "$tag"
echo "Release workflow triggered: https://github.com/macji/synchalo/actions"
