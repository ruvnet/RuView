#!/usr/bin/env bash
# Ephemeral Ubuntu 24.04 CI dependency installation. Keep Ubuntu signature/TLS
# verification, but bypass the hosted image's failing mirror+file indirection.
set -euo pipefail

if (( $# == 0 )); then
  echo 'Pass one or more package names.' >&2
  exit 2
fi
for package in "$@"; do
  if [[ ! "$package" =~ ^[a-z0-9][a-z0-9+.-]*(:amd64)?$ ]]; then
    echo 'Invalid package argument; APT options are not accepted.' >&2
    exit 2
  fi
done
if [[ $(uname -s) != Linux ]] || [[ ! -r /etc/os-release ]]; then
  echo 'This helper requires an Ubuntu 24.04 amd64 CI runner.' >&2
  exit 2
fi
os_id=$(. /etc/os-release; printf '%s' "$ID")
os_version=$(. /etc/os-release; printf '%s' "$VERSION_ID")
if [[ "$os_id" != ubuntu || "$os_version" != 24.04 || $(dpkg --print-architecture) != amd64 ]]; then
  echo 'This helper requires an Ubuntu 24.04 amd64 CI runner.' >&2
  exit 2
fi
keyring=/usr/share/keyrings/ubuntu-archive-keyring.gpg
[[ -r "$keyring" ]] || { echo 'Ubuntu archive keyring is unavailable.' >&2; exit 1; }
# Fail quickly on actual repository DNS loss, rather than spending hours trying
# each package through broken mirror fallbacks.
for host in archive.ubuntu.com security.ubuntu.com; do
  timeout 20 getent ahostsv4 "$host" >/dev/null
done
apt_dir=$(mktemp -d -t ruview-ci-apt.XXXXXX)
source_list="$apt_dir/ubuntu.list"
parts_dir="$apt_dir/empty"
trap 'rm -f -- "$source_list"; rmdir -- "$parts_dir" "$apt_dir"' EXIT
mkdir "$parts_dir"
chmod 755 "$apt_dir" "$parts_dir"
cat > "$source_list" <<SOURCES
deb [signed-by=$keyring] https://archive.ubuntu.com/ubuntu noble main restricted universe multiverse
deb [signed-by=$keyring] https://archive.ubuntu.com/ubuntu noble-updates main restricted universe multiverse
deb [signed-by=$keyring] https://archive.ubuntu.com/ubuntu noble-backports main restricted universe multiverse
deb [signed-by=$keyring] https://security.ubuntu.com/ubuntu noble-security main restricted universe multiverse
SOURCES
chmod 644 "$source_list"
apt_command=(env DEBIAN_FRONTEND=noninteractive apt-get)
if (( EUID != 0 )); then apt_command=(sudo "${apt_command[@]}"); fi
apt_options=(-o "Dir::Etc::sourcelist=$source_list" -o "Dir::Etc::sourceparts=$parts_dir"
  -o APT::Get::List-Cleanup=0 -o APT::Update::Error-Mode=any
  -o Acquire::Retries=2 -o Acquire::http::Timeout=20 -o Acquire::https::Timeout=20)
"${apt_command[@]}" "${apt_options[@]}" update
"${apt_command[@]}" "${apt_options[@]}" install -y --no-install-recommends "$@"
