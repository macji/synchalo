#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  echo "Usage: scripts/build-apt-repository.sh <linux-assets-dir> <site-output-dir>" >&2
  exit 2
fi

for required_command in apt-ftparchive dpkg-deb dpkg-scanpackages gpg gpgv gzip install; do
  if ! command -v "$required_command" >/dev/null; then
    echo "Required command is unavailable: $required_command" >&2
    exit 1
  fi
done

: "${RELEASE_VERSION:?RELEASE_VERSION is required}"
: "${APT_GPG_PRIVATE_KEY:?APT_GPG_PRIVATE_KEY is required}"
: "${APT_GPG_PASSPHRASE:?APT_GPG_PASSPHRASE is required}"
: "${APT_GPG_FINGERPRINT:?APT_GPG_FINGERPRINT is required}"

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
assets_dir="$(cd "$1" && pwd)"
site_dir="$2"

mapfile -t deb_packages < <(find "$assets_dir" -maxdepth 1 -type f -name '*.deb' -print)
expected_architectures="${APT_EXPECTED_ARCHITECTURES:-amd64 arm64}"
expected_count="$(wc -w <<< "$expected_architectures" | tr -d '[:space:]')"
if [[ ${#deb_packages[@]} -ne "$expected_count" ]]; then
  echo "Expected $expected_count DEB packages in $assets_dir, found ${#deb_packages[@]}." >&2
  exit 1
fi

apt_root="$site_dir/apt"
pool_dir="$apt_root/pool/main/s/sync-halo"
mkdir -p "$pool_dir"
package_architectures=()
for deb_package in "${deb_packages[@]}"; do
  package_name="$(dpkg-deb -f "$deb_package" Package)"
  package_version="$(dpkg-deb -f "$deb_package" Version)"
  package_architecture="$(dpkg-deb -f "$deb_package" Architecture)"
  if [[ "$package_name" != "sync-halo" ]]; then
    echo "Unexpected Debian package name: $package_name" >&2
    exit 1
  fi
  if [[ "$package_version" != "$RELEASE_VERSION" ]]; then
    echo "DEB version $package_version does not match release $RELEASE_VERSION." >&2
    exit 1
  fi
  case "$package_architecture" in
    amd64|arm64) ;;
    *)
      echo "Unexpected Debian architecture: $package_architecture" >&2
      exit 1
      ;;
  esac
  if [[ " ${package_architectures[*]} " == *" $package_architecture "* ]]; then
    echo "Duplicate Debian architecture: $package_architecture" >&2
    exit 1
  fi
  package_architectures+=("$package_architecture")
  install -m 0644 "$deb_package" \
    "$pool_dir/sync-halo_${RELEASE_VERSION}_${package_architecture}.deb"
done

actual_architectures="$(printf '%s\n' "${package_architectures[@]}" | sort | paste -sd' ' -)"
expected_architectures="$(tr ' ' '\n' <<< "$expected_architectures" | sed '/^$/d' | sort -u | paste -sd' ' -)"
if [[ "$actual_architectures" != "$expected_architectures" ]]; then
  echo "DEB architectures '$actual_architectures' do not match expected '$expected_architectures'." >&2
  exit 1
fi

(
  cd "$apt_root"
  for package_architecture in ${actual_architectures}; do
    binary_dir="dists/stable/main/binary-${package_architecture}"
    mkdir -p "$binary_dir"
    dpkg-scanpackages --arch "$package_architecture" pool /dev/null > "$binary_dir/Packages"
    gzip -9 -n -c "$binary_dir/Packages" > "$binary_dir/Packages.gz"
  done
  apt-ftparchive \
    -o APT::FTPArchive::Release::Origin=SyncHalo \
    -o APT::FTPArchive::Release::Label=SyncHalo \
    -o APT::FTPArchive::Release::Suite=stable \
    -o APT::FTPArchive::Release::Codename=stable \
    -o "APT::FTPArchive::Release::Architectures=$actual_architectures" \
    -o APT::FTPArchive::Release::Components=main \
    -o APT::FTPArchive::Release::Description='SyncHalo signed APT repository' \
    release dists/stable > dists/stable/Release
)

gpg_parent="${RUNNER_TEMP:-/tmp}"
gpg_home="$(mktemp -d "$gpg_parent/synchalo-apt-gpg.XXXXXX")"
chmod 0700 "$gpg_home"
export GNUPGHOME="$gpg_home"
printf '%s\n' "$APT_GPG_PRIVATE_KEY" | gpg --batch --import >/dev/null

actual_fingerprint="$(
  gpg --batch --with-colons --list-secret-keys \
    | awk -F: '$1 == "fpr" { print toupper($10); exit }'
)"
expected_fingerprint="$(printf '%s' "$APT_GPG_FINGERPRINT" | tr -d '[:space:]' | tr '[:lower:]' '[:upper:]')"
if [[ -z "$actual_fingerprint" || "$actual_fingerprint" != "$expected_fingerprint" ]]; then
  echo "Imported APT signing key does not match APT_GPG_FINGERPRINT." >&2
  exit 1
fi
bundled_key="${SYNCHALO_APT_PUBLIC_KEY_PATH:-$project_root/packaging/apt/synchalo-archive-keyring.asc}"
bundled_fingerprint="$(
  gpg --batch --with-colons --show-keys "$bundled_key" 2>/dev/null \
    | awk -F: '$1 == "fpr" { print toupper($10); exit }'
)"
if [[ -z "$bundled_fingerprint" || "$bundled_fingerprint" != "$actual_fingerprint" ]]; then
  echo "Bundled APT public key does not match the release signing key." >&2
  exit 1
fi

release_file="$apt_root/dists/stable/Release"
printf '%s' "$APT_GPG_PASSPHRASE" \
  | gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 \
      --local-user "$actual_fingerprint" --digest-algo SHA256 \
      --clearsign --output "$apt_root/dists/stable/InRelease" "$release_file"
printf '%s' "$APT_GPG_PASSPHRASE" \
  | gpg --batch --yes --pinentry-mode loopback --passphrase-fd 0 \
      --local-user "$actual_fingerprint" --digest-algo SHA256 \
      --armor --detach-sign --output "$apt_root/dists/stable/Release.gpg" "$release_file"

gpg --batch --export "$actual_fingerprint" > "$apt_root/synchalo-archive-keyring.gpg"
gpg --batch --armor --export "$actual_fingerprint" > "$apt_root/synchalo-archive-keyring.asc"
install -m 0644 "$project_root/packaging/deb/synchalo.sources" "$apt_root/synchalo.sources"
gpgv --keyring "$apt_root/synchalo-archive-keyring.gpg" \
  "$apt_root/dists/stable/Release.gpg" "$release_file"
gpgv --keyring "$apt_root/synchalo-archive-keyring.gpg" \
  "$apt_root/dists/stable/InRelease"

install -m 0644 "$project_root/packaging/apt/index.html" "$site_dir/index.html"
install -m 0644 "$project_root/packaging/apt/index.html" "$apt_root/index.html"
touch "$site_dir/.nojekyll"

echo "APT repository created for sync-halo $RELEASE_VERSION ($actual_architectures)."
echo "Signing fingerprint: $actual_fingerprint"
