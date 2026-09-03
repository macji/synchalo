#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/setup-apt-signing.sh [owner/repository]

Creates a password-protected APT archive signing key, stores the private key
and passphrase as GitHub Actions secrets, stores the public fingerprint as a
GitHub Actions variable, and keeps an encrypted local backup outside the repo.
EOF
}

if [[ $# -gt 1 ]]; then
  usage >&2
  exit 2
fi

for required_command in gh git gpg openssl; do
  if ! command -v "$required_command" >/dev/null; then
    echo "Required command is unavailable: $required_command" >&2
    exit 1
  fi
done

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"
repository="${1:-$(gh repo view --json nameWithOwner --jq .nameWithOwner)}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "Run this setup only from a clean working tree." >&2
  git status --short >&2
  exit 1
fi

if [[ "$(uname -s)" == "Darwin" ]]; then
  local_user_home="$(dscl . -read "/Users/$(id -un)" NFSHomeDirectory | awk '{print $2}')"
else
  local_user_home="$(getent passwd "$(id -u)" | cut -d: -f6)"
fi
backup_dir="${SYNCHALO_APT_KEY_DIR:-$local_user_home/.config/synchalo/apt-signing}"
private_backup="$backup_dir/synchalo-archive-private.asc"
public_backup="$backup_dir/synchalo-archive-public.asc"
if [[ -e "$private_backup" || -e "$public_backup" ]]; then
  echo "APT signing key backup already exists in $backup_dir; refusing to overwrite it." >&2
  exit 1
fi

temporary_root="$(mktemp -d /tmp/synchalo-apt-key.XXXXXX)"
cleanup() {
  if [[ -d "$temporary_root" && "$temporary_root" == /tmp/synchalo-apt-key.* ]]; then
    find "$temporary_root" -type f -exec chmod u+w {} +
    find "$temporary_root" -depth -delete
  fi
}
trap cleanup EXIT
gpg_home="$temporary_root/gnupg"
passphrase_file="$temporary_root/passphrase"
mkdir -m 0700 "$gpg_home"
openssl rand -base64 36 > "$passphrase_file"
chmod 0600 "$passphrase_file"
export GNUPGHOME="$gpg_home"

gpg --batch --pinentry-mode loopback \
  --passphrase-file "$passphrase_file" \
  --quick-gen-key \
  "SyncHalo Archive Signing <releases@synchalo.io>" \
  rsa4096 sign 3y
fingerprint="$(
  gpg --batch --with-colons --list-secret-keys \
    | awk -F: '$1 == "fpr" { print toupper($10); exit }'
)"
if [[ -z "$fingerprint" ]]; then
  echo "Could not resolve the generated key fingerprint." >&2
  exit 1
fi

mkdir -p "$backup_dir"
chmod 0700 "$backup_dir"
gpg --batch --pinentry-mode loopback \
  --passphrase-file "$passphrase_file" \
  --armor --export-secret-keys "$fingerprint" > "$private_backup"
gpg --batch --armor --export "$fingerprint" > "$public_backup"
chmod 0600 "$private_backup"
chmod 0644 "$public_backup"

gh secret set APT_GPG_PRIVATE_KEY --repo "$repository" < "$private_backup"
gh secret set APT_GPG_PASSPHRASE --repo "$repository" < "$passphrase_file"
gh variable set APT_GPG_FINGERPRINT --repo "$repository" --body "$fingerprint"

if [[ "$(uname -s)" == "Darwin" ]]; then
  security add-generic-password -U \
    -s io.synchalo.desktop.release \
    -a apt-signing-key-password-v1 \
    -w "$(tr -d '\r\n' < "$passphrase_file")" >/dev/null
  echo "The backup passphrase was also saved in the macOS Keychain."
fi

echo "APT signing credentials configured for $repository."
echo "Fingerprint: $fingerprint"
echo "Encrypted private-key backup: $private_backup"
