#!/usr/bin/env bash
set -euo pipefail

for required_command in gpg jq; do
  if ! command -v "$required_command" >/dev/null; then
    echo "Required command is unavailable: $required_command" >&2
    exit 1
  fi
done

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
tauri_config="$project_root/apps/desktop/src-tauri/tauri.conf.json"
linux_config="$project_root/apps/desktop/src-tauri/tauri.linux.conf.json"
source_file="$project_root/packaging/deb/synchalo.sources"
key_file="$project_root/packaging/apt/synchalo-archive-keyring.asc"
postinst_file="$project_root/packaging/deb/postinst"
update_helper="$project_root/packaging/deb/update-synchalo"
update_policy="$project_root/packaging/deb/io.synchalo.desktop.update.policy"
expected_fingerprint="D12C8DBA7726A408BBDEC87FA4C4CF3A9C37E151"
test_root="$(mktemp -d /tmp/synchalo-deb-bootstrap.XXXXXX)"
cleanup() {
  if [[ -d "$test_root" && "$test_root" == /tmp/synchalo-deb-bootstrap.* ]]; then
    find "$test_root" -depth -delete
  fi
}
trap cleanup EXIT
export GNUPGHOME="$test_root/gnupg"
mkdir -p "$GNUPGHOME"
chmod 0700 "$GNUPGHOME"

jq -e \
  --arg source_path "../../../packaging/deb/synchalo.sources" \
  --arg key_path "../../../packaging/apt/synchalo-archive-keyring.asc" \
  --arg postinst_path "../../../packaging/deb/postinst" \
  --arg helper_path "../../../packaging/deb/update-synchalo" \
  --arg policy_path "../../../packaging/deb/io.synchalo.desktop.update.policy" \
  '.bundle.linux.deb.files["/etc/apt/sources.list.d/synchalo.sources"] == $source_path
    and .bundle.linux.deb.files["/usr/lib/synchalo/update-synchalo"] == $helper_path
    and .bundle.linux.deb.files["/usr/share/keyrings/synchalo-archive-keyring.asc"] == $key_path
    and .bundle.linux.deb.files["/usr/share/polkit-1/actions/io.synchalo.desktop.update.policy"] == $policy_path
    and .bundle.linux.deb.postInstallScript == $postinst_path' \
  "$tauri_config" >/dev/null
jq -e \
  '.bundle.createUpdaterArtifacts == false and .bundle.targets == ["deb"]' \
  "$linux_config" >/dev/null

legacy_root="$test_root/legacy-root"
mkdir -p "$legacy_root/etc/apt/sources.list.d"
legacy_source="$legacy_root/etc/apt/sources.list.d/synchalo.list"
printf '%s\n' \
  'deb [arch=arm64 signed-by=/etc/apt/keyrings/synchalo.gpg] https://macji.github.io/synchalo/apt stable main' \
  > "$legacy_source"
DPKG_ROOT="$legacy_root" "$postinst_file" configure
if [[ -e "$legacy_source" ]]; then
  echo "The exact legacy APT source was not migrated." >&2
  exit 1
fi
printf '%s\n' '# user-managed source' > "$legacy_source"
DPKG_ROOT="$legacy_root" "$postinst_file" configure
if [[ "$(cat "$legacy_source")" != "# user-managed source" ]]; then
  echo "The DEB migration changed a user-managed APT source." >&2
  exit 1
fi

expected_source="$(cat <<'EOF'
Types: deb
URIs: https://macji.github.io/synchalo/apt
Suites: stable
Components: main
Architectures: arm64
Signed-By: /usr/share/keyrings/synchalo-archive-keyring.asc
EOF
)"
if [[ "$(cat "$source_file")" != "$expected_source" ]]; then
  echo "Unexpected SyncHalo Deb822 APT source configuration." >&2
  exit 1
fi

actual_fingerprint="$(
  gpg --batch --with-colons --show-keys "$key_file" 2>/dev/null \
    | awk -F: '$1 == "fpr" { print toupper($10); exit }'
)"
if [[ "$actual_fingerprint" != "$expected_fingerprint" ]]; then
  echo "Bundled APT public key fingerprint is $actual_fingerprint, expected $expected_fingerprint." >&2
  exit 1
fi

if ! rg -q '<allow_active>auth_admin</allow_active>' "$update_policy" \
  || ! rg -q '<annotate key="org.freedesktop.policykit.exec.path">/usr/lib/synchalo/update-synchalo</annotate>' "$update_policy"; then
  echo "Unexpected SyncHalo Polkit policy." >&2
  exit 1
fi
if "$update_helper" 0.1.7 >/dev/null 2>&1; then
  echo "The update helper unexpectedly ran without root privileges." >&2
  exit 1
elif [[ $? -ne 77 ]]; then
  echo "The update helper did not reject an unprivileged caller safely." >&2
  exit 1
fi

if [[ $# -gt 1 ]]; then
  echo "Usage: tests/release/deb_apt_bootstrap_smoke.sh [package.deb]" >&2
  exit 2
fi

if [[ $# -eq 1 ]]; then
  deb_package="$1"
  if ! command -v dpkg-deb >/dev/null; then
    echo "dpkg-deb is required when validating a built package." >&2
    exit 1
  fi
  if [[ "$(dpkg-deb -f "$deb_package" Package)" != "sync-halo" ]]; then
    echo "Unexpected Debian package name." >&2
    exit 1
  fi
  if [[ "$(dpkg-deb -f "$deb_package" Architecture)" != "arm64" ]]; then
    echo "Unexpected Debian package architecture." >&2
    exit 1
  fi
  if ! dpkg-deb -f "$deb_package" Depends | tr ',' '\n' | sed 's/^ *//' | grep -Eq '^pkexec([[:space:](]|$)'; then
    echo "Built DEB does not depend on pkexec." >&2
    exit 1
  fi

  package_root="$test_root/package-root"
  control_root="$test_root/control-root"
  mkdir -p "$package_root" "$control_root"
  dpkg-deb --extract "$deb_package" "$package_root"
  dpkg-deb --control "$deb_package" "$control_root"

  packaged_source="$(cat "$package_root/etc/apt/sources.list.d/synchalo.sources")"
  if [[ "$packaged_source" != "$expected_source" ]]; then
    echo "Built DEB does not contain the expected APT source." >&2
    exit 1
  fi

  packaged_key="$test_root/synchalo-archive-keyring.asc"
  cp "$package_root/usr/share/keyrings/synchalo-archive-keyring.asc" "$packaged_key"
  packaged_key_fingerprint="$(
    gpg --batch --with-colons --show-keys "$packaged_key" 2>/dev/null \
      | awk -F: '$1 == "fpr" { print toupper($10); exit }'
  )"
  if [[ "$packaged_key_fingerprint" != "$expected_fingerprint" ]]; then
    echo "Built DEB contains an unexpected APT public key." >&2
    exit 1
  fi

  if ! cmp -s "$postinst_file" "$control_root/postinst"; then
    echo "Built DEB does not contain the expected post-install migration." >&2
    exit 1
  fi

  if ! cmp -s "$update_helper" "$package_root/usr/lib/synchalo/update-synchalo"; then
    echo "Built DEB does not contain the expected update helper." >&2
    exit 1
  fi
  if ! cmp -s "$update_policy" "$package_root/usr/share/polkit-1/actions/io.synchalo.desktop.update.policy"; then
    echo "Built DEB does not contain the expected Polkit policy." >&2
    exit 1
  fi
  helper_mode="$(stat -c '%a' "$package_root/usr/lib/synchalo/update-synchalo")"
  if [[ "$helper_mode" != "755" ]]; then
    echo "Built DEB update helper mode is $helper_mode, expected 755." >&2
    exit 1
  fi
fi

echo "DEB APT bootstrap smoke test passed."
