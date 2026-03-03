#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[flatpak-build] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[flatpak-build] Missing required command: $1" >&2
    exit 1
  fi
}

is_debian_based() {
  [[ -f /etc/os-release ]] || return 1

  # shellcheck disable=SC1091
  source /etc/os-release
  [[ "${ID:-}" == "debian" || "${ID:-}" == "ubuntu" || "${ID_LIKE:-}" == *"debian"* ]]
}

main() {
  local script_dir repo_root manifest_path build_dir workdir repodir bundle_path

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  manifest_path="$repo_root/Resources/Flatpak/io.matt.Basalt.yml"

  build_dir="$repo_root/Build"
  workdir="$build_dir/flatpak-build"
  repodir="$build_dir/flatpak-repo"
  bundle_path="$build_dir/Basalt.flatpak"

  if [[ ! -f "$manifest_path" ]]; then
    echo "[flatpak-build] Manifest not found: $manifest_path" >&2
    exit 1
  fi

  require_cmd cargo

  if is_debian_based; then
    require_cmd sudo
    require_cmd apt-get

    log "Installing Flatpak tooling (Debian-based)"
    sudo apt-get update
    sudo apt-get install -y flatpak flatpak-builder
  else
    echo "[flatpak-build] Non-Debian distro detected. Install flatpak and flatpak-builder manually." >&2
  fi

  require_cmd flatpak
  require_cmd flatpak-builder

  if ! flatpak remotes --columns=name | grep -qx "flathub"; then
    log "Adding Flathub remote"
    flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
  fi

  log "Installing required Flatpak runtime and SDK"
  flatpak install -y flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08

  log "Building Rust release binary"
  cargo build --manifest-path "$repo_root/Cargo.toml" --release

  log "Preparing Build directory"
  mkdir -p "$workdir" "$repodir"

  log "Building Flatpak from manifest"
  flatpak-builder --force-clean --repo="$repodir" "$workdir" "$manifest_path"

  log "Bundling Flatpak"
  rm -f "$bundle_path"
  flatpak build-bundle "$repodir" "$bundle_path" io.matt.Basalt

  log "Done"
  echo "Bundle created at: $bundle_path"
  echo "To install, run: ./Install.sh"
  echo "Run with: flatpak run io.matt.Basalt"
}

main "$@"
