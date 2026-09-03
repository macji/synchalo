#!/usr/bin/env bash
set -euo pipefail

for required_command in apt-cache apt-ftparchive apt-get dpkg-deb gpg gpgv; do
  if ! command -v "$required_command" >/dev/null; then
    echo "Required command is unavailable: $required_command" >&2
    exit 1
  fi
done

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
test_root="$(mktemp -d /tmp/synchalo-apt-smoke.XXXXXX)"
cleanup() {
  if [[ -d "$test_root" && "$test_root" == /tmp/synchalo-apt-smoke.* ]]; then
    find "$test_root" -type f -exec chmod u+w {} +
    find "$test_root" -depth -delete
  fi
}
trap cleanup EXIT

package_root="$test_root/package"
assets_dir="$test_root/assets"
site_dir="$test_root/site"
gpg_home="$test_root/gnupg"
mkdir -p "$package_root/DEBIAN" "$assets_dir" "$gpg_home"
chmod 0700 "$gpg_home"

cat > "$package_root/DEBIAN/control" <<'EOF'
Package: sync-halo
Version: 9.9.9
Architecture: arm64
Maintainer: SyncHalo <releases@synchalo.io>
Description: SyncHalo APT repository smoke-test package
EOF
dpkg-deb --root-owner-group --build \
  "$package_root" "$assets_dir/SyncHalo_9.9.9_ubuntu-arm64.deb" >/dev/null

export GNUPGHOME="$gpg_home"
gpg --batch --pinentry-mode loopback --passphrase test-only-passphrase \
  --quick-gen-key "SyncHalo APT Test <test@synchalo.invalid>" ed25519 sign 1d >/dev/null
export APT_GPG_FINGERPRINT="$(
  gpg --batch --with-colons --list-secret-keys \
    | awk -F: '$1 == "fpr" { print toupper($10); exit }'
)"
export APT_GPG_PRIVATE_KEY="$(
  gpg --batch --pinentry-mode loopback --passphrase test-only-passphrase \
    --armor --export-secret-keys "$APT_GPG_FINGERPRINT"
)"
export SYNCHALO_APT_PUBLIC_KEY_PATH="$test_root/test-archive-keyring.asc"
gpg --batch --armor --export "$APT_GPG_FINGERPRINT" > "$SYNCHALO_APT_PUBLIC_KEY_PATH"
export APT_GPG_PASSPHRASE=test-only-passphrase
export RELEASE_VERSION=9.9.9

"$project_root/scripts/build-apt-repository.sh" "$assets_dir" "$site_dir"
cmp "$project_root/packaging/deb/synchalo.sources" "$site_dir/apt/synchalo.sources"

source_list="$test_root/synchalo.list"
lists_dir="$test_root/lists"
mkdir -p "$lists_dir/partial"
chmod 0755 "$test_root" "$site_dir" "$site_dir/apt" "$lists_dir" "$lists_dir/partial"
printf 'deb [arch=arm64 signed-by=%s] file:%s stable main\n' \
  "$site_dir/apt/synchalo-archive-keyring.gpg" \
  "$site_dir/apt" \
  > "$source_list"
apt_options=(
  -o "Dir::Etc::sourcelist=$source_list"
  -o Dir::Etc::sourceparts=-
  -o "Dir::State::lists=$lists_dir"
  -o APT::Get::List-Cleanup=0
)
apt-get update "${apt_options[@]}" >/dev/null
candidate="$(apt-cache "${apt_options[@]}" policy sync-halo | awk '/Candidate:/ { print $2 }')"
if [[ "$candidate" != "9.9.9" ]]; then
  echo "APT did not resolve the smoke-test package; candidate was $candidate." >&2
  exit 1
fi

echo "APT repository smoke test passed."
