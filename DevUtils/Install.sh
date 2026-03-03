#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[flatpak-install] %s\n" "$1"
}

install_from_local_bundle() {
  local bundle_path="$1"

  if ! command -v flatpak >/dev/null 2>&1; then
    echo "[flatpak-install] flatpak is required to install a bundle. Please install flatpak first." >&2
    exit 1
  fi

  log "Installing local bundle: $bundle_path"
  flatpak install --user -y "$bundle_path"

  log "Installed"
  echo "Run with: flatpak run io.matt.Basalt"
}

install_from_github_release() {
  log "GitHub release downloading and installing is not yet implemented."
  echo "Please build the project locally first (e.g., ./DevUtils/BuildFlatpak.sh)."
}

main() {
  local script_dir repo_root build_dir bundle_path choice

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  build_dir="$repo_root/Build"
  bundle_path="$build_dir/Basalt.flatpak"

  if [[ -f "$bundle_path" ]]; then
    echo "A local Flatpak bundle was found at: $bundle_path"
    echo "Choose an option:"
    echo "  [1] Install local bundle"
    echo "  [2] Fetch and install latest GitHub release"

    while true; do
      read -r -p "Enter choice [1/2]: " choice
      case "$choice" in
        1)
          install_from_local_bundle "$bundle_path"
          return
          ;;
        2)
          install_from_github_release
          return
          ;;
        *)
          echo "Please enter 1 or 2."
          ;;
      esac
    done
  else
    install_from_github_release
  fi
}

main "$@"
