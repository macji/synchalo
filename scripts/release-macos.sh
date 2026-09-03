#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/release-macos.sh <version|tag> [--dry-run]

Builds, signs, notarizes, staples, verifies, and uploads the macOS ARM64
artifacts for an existing release tag. The matching Linux/Windows GitHub
workflow must finish successfully before the macOS assets are uploaded.
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

for required_command in cargo codesign curl ditto dscl gh git jq npm rg security shasum spctl tar unzip xcrun; do
  if ! command -v "$required_command" >/dev/null; then
    echo "Required command is unavailable: $required_command" >&2
    exit 1
  fi
done
if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "macOS releases must run on an Apple Silicon Mac." >&2
  exit 1
fi

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

github_repository="${SYNCHALO_GITHUB_REPOSITORY:-macji/synchalo}"
signing_identity="${SYNCHALO_APPLE_SIGNING_IDENTITY:-Developer ID Application: Hangzhou Xpower Technology Co., Ltd. (39UVPY4WQL)}"
notary_profile="${SYNCHALO_NOTARY_PROFILE:-SyncHaloNotary}"
user_home_dir="$(dscl . -read "/Users/$(id -un)" NFSHomeDirectory | awk '{print $2}')"
updater_key_path="${SYNCHALO_UPDATER_KEY_PATH:-$user_home_dir/.config/synchalo/updater-signing.key}"
updater_key_service="${SYNCHALO_UPDATER_KEY_SERVICE:-io.synchalo.desktop.release}"
updater_key_account="${SYNCHALO_UPDATER_KEY_ACCOUNT:-updater-signing-key-password-v1}"

if [[ "$(git branch --show-current)" != "main" ]]; then
  echo "macOS releases must run from main." >&2
  exit 1
fi
if [[ -n "$(git status --porcelain)" ]]; then
  echo "The working tree must be clean before a macOS release." >&2
  git status --short >&2
  exit 1
fi

tauri_version="$(jq -r '.version' apps/desktop/src-tauri/tauri.conf.json)"
root_npm_version="$(jq -r '.version' package.json)"
desktop_npm_version="$(jq -r '.version' apps/desktop/package.json)"
cargo_version="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' Cargo.toml)"
lock_version="$(jq -r '.packages[""] | .version' package-lock.json)"
lock_desktop_version="$(jq -r '.packages["apps/desktop"] | .version' package-lock.json)"
for declared_version in \
  "$tauri_version" \
  "$root_npm_version" \
  "$desktop_npm_version" \
  "$cargo_version" \
  "$lock_version" \
  "$lock_desktop_version"; do
  if [[ "$declared_version" != "$version" ]]; then
    echo "Version mismatch: tag is $tag but a project file declares $declared_version" >&2
    exit 1
  fi
done

git fetch origin main --tags
head_commit="$(git rev-parse HEAD)"
main_commit="$(git rev-parse origin/main)"
tag_commit="$(git rev-parse --verify "$tag^{}")"
remote_tag_commit="$(git ls-remote origin "refs/tags/$tag^{}" | awk '{print $1}')"
if [[ -z "$remote_tag_commit" ]]; then
  echo "Remote tag does not exist: $tag" >&2
  exit 1
fi
for release_commit in "$main_commit" "$tag_commit" "$remote_tag_commit"; do
  if [[ "$release_commit" != "$head_commit" ]]; then
    echo "Release refs do not resolve to the same commit." >&2
    echo "HEAD:        $head_commit" >&2
    echo "origin/main: $main_commit" >&2
    echo "$tag:       $tag_commit" >&2
    echo "remote tag: $remote_tag_commit" >&2
    exit 1
  fi
done

if [[ ! -s "$updater_key_path" ]]; then
  echo "Updater private key is unavailable: $updater_key_path" >&2
  exit 1
fi
if [[ "$(stat -f '%Lp' "$updater_key_path")" != "600" ]]; then
  echo "Updater private key must have mode 0600: $updater_key_path" >&2
  exit 1
fi
if ! security find-generic-password \
  -s "$updater_key_service" \
  -a "$updater_key_account" \
  >/dev/null 2>&1; then
  echo "Updater key password is unavailable in the macOS Keychain." >&2
  exit 1
fi
if ! security find-identity -v -p codesigning | rg -Fq "$signing_identity"; then
  echo "Developer ID signing identity is unavailable: $signing_identity" >&2
  exit 1
fi
if ! xcrun notarytool history --keychain-profile "$notary_profile" >/dev/null; then
  echo "Notary profile is unavailable: $notary_profile" >&2
  exit 1
fi
gh auth status >/dev/null

echo "Release tag:  $tag"
echo "Commit:       $head_commit"
echo "Repository:   $github_repository"
echo "Architecture: arm64"
if [[ "$dry_run" == true ]]; then
  echo "Dry run passed; no build or upload was performed."
  exit 0
fi

running_pids="$(ps -axo pid=,command= | awk -v root="$project_root" '$0 ~ root && $0 ~ /synchalo-desktop$/ {print $1}')"
if [[ -n "$running_pids" ]]; then
  echo "Stopping running SyncHalo process(es): $running_pids"
  while IFS= read -r process_id; do
    [[ -n "$process_id" ]] && kill "$process_id"
  done <<< "$running_pids"
  for _ in 1 2 3 4 5; do
    sleep 1
    still_running=false
    while IFS= read -r process_id; do
      if [[ -n "$process_id" ]] && ps -p "$process_id" -o pid= >/dev/null; then
        still_running=true
      fi
    done <<< "$running_pids"
    [[ "$still_running" == false ]] && break
  done
  if [[ "$still_running" == true ]]; then
    echo "SyncHalo did not stop cleanly." >&2
    exit 1
  fi
fi

npm run lint
npm test
npm run build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

updater_key_password="$(security find-generic-password \
  -s "$updater_key_service" \
  -a "$updater_key_account" \
  -w)"
TAURI_SIGNING_PRIVATE_KEY="$updater_key_path" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$updater_key_password" \
APPLE_SIGNING_IDENTITY="$signing_identity" \
  npm run tauri -- build
unset updater_key_password

fresh_app="$project_root/target/release/bundle/macos/SyncHalo.app"
updater_archive="$(find "$project_root/target/release/bundle/macos" \
  -maxdepth 1 -type f -name '*.app.tar.gz' -print -quit)"
if [[ ! -d "$fresh_app" || -z "$updater_archive" || ! -s "$updater_archive.sig" ]]; then
  echo "Expected macOS application and updater artifacts were not produced." >&2
  exit 1
fi

work_dir="$(mktemp -d /tmp/synchalo-macos-release.XXXXXX)"
canonical_ready=false
cleanup() {
  if [[ "$canonical_ready" == false \
    && -n "${previous_app:-}" \
    && -d "$previous_app" ]]; then
    if [[ -d "${release_dir:-}/SyncHalo.app" ]]; then
      mv "$release_dir/SyncHalo.app" "$work_dir/failed-SyncHalo.app"
    fi
    mv "$previous_app" "$release_dir/SyncHalo.app"
  fi
  if [[ "$work_dir" == /tmp/synchalo-macos-release.* && -d "$work_dir" ]]; then
    rm -rf -- "$work_dir"
  fi
}
trap cleanup EXIT

notary_archive="$work_dir/SyncHalo-notary.zip"
ditto -c -k --sequesterRsrc --keepParent "$fresh_app" "$notary_archive"
xcrun notarytool submit "$notary_archive" --keychain-profile "$notary_profile" --wait
xcrun stapler staple "$fresh_app"
codesign --verify --deep --strict --verbose=2 "$fresh_app"
xcrun stapler validate "$fresh_app"
spctl --assess --type execute -vv "$fresh_app"

release_dir="$project_root/release/macos-arm64"
staged_app="$work_dir/SyncHalo.app"
previous_app="$work_dir/previous-SyncHalo.app"
stale_app_dir="$work_dir/stale-apps"
mkdir "$stale_app_dir"
while IFS= read -r stale_app; do
  mv "$stale_app" "$stale_app_dir/"
done < <(find "$release_dir" -maxdepth 1 -type d -name 'SyncHalo*.app' ! -name 'SyncHalo.app' -print)
ditto "$fresh_app" "$staged_app"
codesign --verify --deep --strict --verbose=2 "$staged_app"
xcrun stapler validate "$staged_app"
if [[ -d "$release_dir/SyncHalo.app" ]]; then
  mv "$release_dir/SyncHalo.app" "$previous_app"
fi
mv "$staged_app" "$release_dir/SyncHalo.app"

canonical_zip="$release_dir/SyncHalo-macos-arm64.zip"
next_zip="$work_dir/SyncHalo-macos-arm64.zip"
ditto -c -k --sequesterRsrc --keepParent "$release_dir/SyncHalo.app" "$next_zip"
unzip -tq "$next_zip"
archive_root="$(unzip -Z1 "$next_zip" | sed -n '1p')"
if [[ "$archive_root" != "SyncHalo.app/" ]]; then
  echo "Unexpected macOS ZIP root: $archive_root" >&2
  exit 1
fi
mv "$next_zip" "$canonical_zip"
(
  cd "$release_dir"
  shasum -a 256 SyncHalo-macos-arm64.zip > SHA256SUMS.next
  mv SHA256SUMS.next SHA256SUMS
)
codesign --verify --deep --strict --verbose=2 "$release_dir/SyncHalo.app"
xcrun stapler validate "$release_dir/SyncHalo.app"
spctl --assess --type execute -vv "$release_dir/SyncHalo.app"
canonical_ready=true
stat -f 'Canonical app modified: %Sm' \
  -t '%Y-%m-%d %H:%M:%S %z' \
  "$release_dir/SyncHalo.app"

run_id=""
for _ in {1..120}; do
  run_id="$(gh run list \
    --repo "$github_repository" \
    --workflow release.yml \
    --event push \
    --branch "$tag" \
    --limit 10 \
    --json databaseId,headSha \
    --jq ".[] | select(.headSha == \"$head_commit\") | .databaseId" \
    | sed -n '1p')"
  [[ -n "$run_id" ]] && break
  sleep 5
done
if [[ -z "$run_id" ]]; then
  echo "Could not locate the GitHub release workflow for $tag." >&2
  exit 1
fi
gh run watch "$run_id" --repo "$github_repository" --exit-status

for _ in {1..60}; do
  if gh release view "$tag" --repo "$github_repository" >/dev/null 2>&1; then
    break
  fi
  sleep 5
done
if ! gh release view "$tag" --repo "$github_repository" >/dev/null 2>&1; then
  echo "GitHub Release was not created for $tag." >&2
  exit 1
fi

upload_dir="$work_dir/upload"
signature_dir="$work_dir/signatures"
mkdir "$upload_dir" "$signature_dir"
mac_zip_name="SyncHalo_${version}_macos-arm64.zip"
mac_update_name="SyncHalo_${version}_macos-arm64.app.tar.gz"
mac_signature_name="$mac_update_name.sig"
ditto "$canonical_zip" "$upload_dir/$mac_zip_name"
install -m 0644 "$updater_archive" "$upload_dir/$mac_update_name"
install -m 0644 "$updater_archive.sig" "$upload_dir/$mac_signature_name"
(
  cd "$upload_dir"
  shasum -a 256 "$mac_zip_name" "$mac_update_name" "$mac_signature_name" \
    > SHA256SUMS-macos.txt
)
gh release upload "$tag" \
  "$upload_dir/$mac_zip_name" \
  "$upload_dir/$mac_update_name" \
  "$upload_dir/$mac_signature_name" \
  "$upload_dir/SHA256SUMS-macos.txt" \
  --repo "$github_repository" \
  --clobber

linux_signature_name="SyncHalo_${version}_linux-arm64.AppImage.sig"
windows_signature_name="SyncHalo_${version}_windows-x64-setup.exe.sig"
gh release download "$tag" \
  --repo "$github_repository" \
  --pattern "$linux_signature_name" \
  --pattern "$windows_signature_name" \
  --dir "$signature_dir"
linux_signature="$(tr -d '\r\n' < "$signature_dir/$linux_signature_name")"
windows_signature="$(tr -d '\r\n' < "$signature_dir/$windows_signature_name")"
mac_signature="$(tr -d '\r\n' < "$upload_dir/$mac_signature_name")"
published_at="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
release_base_url="https://github.com/$github_repository/releases/download/$tag"
release_notes="$(gh release view "$tag" --repo "$github_repository" --json body --jq .body)"
jq -n \
  --arg version "$version" \
  --arg notes "$release_notes" \
  --arg pub_date "$published_at" \
  --arg linux_url "$release_base_url/SyncHalo_${version}_linux-arm64.AppImage" \
  --arg linux_signature "$linux_signature" \
  --arg windows_url "$release_base_url/SyncHalo_${version}_windows-x64-setup.exe" \
  --arg windows_signature "$windows_signature" \
  --arg mac_url "$release_base_url/$mac_update_name" \
  --arg mac_signature "$mac_signature" \
  '{
    version: $version,
    notes: $notes,
    pub_date: $pub_date,
    platforms: {
      "linux-aarch64": {url: $linux_url, signature: $linux_signature},
      "windows-x86_64": {url: $windows_url, signature: $windows_signature},
      "darwin-aarch64": {url: $mac_url, signature: $mac_signature}
    }
  }' > "$upload_dir/latest.json"
gh release upload "$tag" "$upload_dir/latest.json" --repo "$github_repository" --clobber

notes_file="$work_dir/release-notes.md"
gh release view "$tag" --repo "$github_repository" --json body --jq .body > "$notes_file"
if ! rg -q 'macOS ARM64' "$notes_file"; then
  printf '\n- macOS ARM64: notarized application ZIP\n' >> "$notes_file"
  gh release edit "$tag" --repo "$github_repository" --notes-file "$notes_file"
fi

for local_asset in \
  "$upload_dir/$mac_zip_name" \
  "$upload_dir/$mac_update_name" \
  "$upload_dir/$mac_signature_name" \
  "$upload_dir/SHA256SUMS-macos.txt" \
  "$upload_dir/latest.json"; do
  asset_name="$(basename "$local_asset")"
  local_digest="sha256:$(shasum -a 256 "$local_asset" | awk '{print $1}')"
  remote_digest=""
  for _ in {1..12}; do
    remote_digest="$(gh release view "$tag" \
      --repo "$github_repository" \
      --json assets \
      --jq ".assets[] | select(.name == \"$asset_name\") | .digest")"
    [[ -n "$remote_digest" && "$remote_digest" != "null" ]] && break
    sleep 5
  done
  if [[ "$remote_digest" != "$local_digest" ]]; then
    echo "Remote digest mismatch for $asset_name" >&2
    echo "local:  $local_digest" >&2
    echo "remote: $remote_digest" >&2
    exit 1
  fi
done
curl -fsSL "https://github.com/$github_repository/releases/latest/download/latest.json" \
  | jq -e --arg version "$version" '.version == $version and (.platforms | length == 3)' \
  >/dev/null

echo "macOS release uploaded and verified: https://github.com/$github_repository/releases/tag/$tag"
