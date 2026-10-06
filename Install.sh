#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[deb-install] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[deb-install] Missing required command: $1" >&2
    exit 1
  fi
}

has_mattpackages_source() {
  grep -rqs "mattpackages" /etc/apt/sources.list /etc/apt/sources.list.d/
}

install_deb_package() {
  local deb_path="$1"

  require_cmd sudo
  require_cmd apt-get
  require_cmd dpkg

  log "Installing or upgrading Debian package: $deb_path"
  if ! sudo dpkg -i "$deb_path"; then
    log "Resolving dependencies"
    sudo apt-get install -f -y
    sudo dpkg -i "$deb_path"
  fi

  if ! has_mattpackages_source; then
    echo "[deb-install] Note: the MattPackages apt repository is not configured on this system." >&2
    echo "[deb-install] Basalt updates are delivered through it with apt; without it this install will not update." >&2
  fi

  if command -v basalt >/dev/null 2>&1; then
    log "Installed successfully"
    echo "Run with: basalt list"
    echo "If your current shell still points to an old command path, run: hash -r"
  else
    echo "[deb-install] Install finished but 'basalt' is not on PATH in this shell yet." >&2
    echo "[deb-install] Run 'hash -r' or open a new shell, then run: basalt list" >&2
  fi
}

read_build_metadata_value() {
  local metadata_path="$1"
  local metadata_key="$2"

  while IFS= read -r line || [[ -n "$line" ]]; do
    line="${line#"${line%%[![:space:]]*}"}"
    line="${line%"${line##*[![:space:]]}"}"

    [[ -z "$line" || "$line" == \#* ]] && continue
    [[ "$line" == "$metadata_key="* ]] || continue

    printf '%s\n' "${line#*=}"
    return 0
  done < "$metadata_path"

  return 1
}

install_from_local_deb() {
  local build_script build_meta deb_path artifact_type

  build_script="$repo_root/DevUtils/Build.sh"
  build_meta="$repo_root/builds/latest-build.env"

  if [[ ! -f "$build_script" ]]; then
    echo "[deb-install] Local build script not found: $build_script" >&2
    exit 1
  fi

  log "Building local Debian package"
  bash "$build_script"

  if [[ ! -f "$build_meta" ]]; then
    echo "[deb-install] Local build metadata not found: $build_meta" >&2
    exit 1
  fi

  deb_path="$(read_build_metadata_value "$build_meta" "BUILD_ARTIFACT_PATH" || true)"
  artifact_type="$(read_build_metadata_value "$build_meta" "BUILD_ARTIFACT_TYPE" || true)"

  if [[ -z "$deb_path" || ! -f "$deb_path" ]]; then
    echo "[deb-install] Local build metadata is missing a valid BUILD_ARTIFACT_PATH." >&2
    exit 1
  fi

  if [[ "$artifact_type" != "deb" || "$deb_path" != *.deb ]]; then
    echo "[deb-install] Local build produced an unsupported artifact for this installer: $deb_path" >&2
    exit 1
  fi

  install_deb_package "$deb_path"
}

install_from_apt() {
  require_cmd sudo
  require_cmd apt-get

  log "Installing Basalt from the MattPackages apt repository"
  sudo apt-get update
  sudo apt-get install -y basalt
  log "Installed successfully; apt keeps Basalt up to date"
}

main() {
  local script_dir repo_root

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-.}")" && pwd)"
  repo_root="$script_dir"

  if [[ -f "$repo_root/DevUtils/Build.sh" ]]; then
    install_from_local_deb
    return
  fi

  if has_mattpackages_source; then
    install_from_apt
    return
  fi

  echo "[deb-install] Run this script from a Basalt checkout to build and install it:" >&2
  echo "  git clone https://github.com/HungLo2020/Basalt.git && cd Basalt && ./Install.sh" >&2
  echo "[deb-install] Or configure the MattPackages apt repository and run: sudo apt install basalt" >&2
  exit 1
}

main "$@"
