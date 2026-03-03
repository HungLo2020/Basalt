#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[flatpak-install] %s\n" "$1"
}

setup_cli_wrapper() {
  local local_bin wrapper_path

  local_bin="$HOME/.local/bin"
  wrapper_path="$local_bin/basalt"

  mkdir -p "$local_bin"

  cat > "$wrapper_path" <<'EOF'
#!/usr/bin/env bash
exec flatpak run io.matt.Basalt "$@"
EOF

  chmod +x "$wrapper_path"

  log "Installed CLI wrapper: $wrapper_path"

  if [[ ":$PATH:" != *":$local_bin:"* ]]; then
    echo "Add this to your shell profile to use 'basalt' directly:"
    echo "  export PATH=\"$HOME/.local/bin:\$PATH\""
    echo "Then restart your shell or run: source ~/.bashrc"
  fi
}

install_from_local_bundle() {
  local bundle_path="$1"

  if ! command -v flatpak >/dev/null 2>&1; then
    echo "[flatpak-install] flatpak is required to install a bundle. Please install flatpak first." >&2
    exit 1
  fi

  if flatpak info io.matt.Basalt >/dev/null 2>&1; then
    log "Replacing existing Basalt Flatpak install (preserving app data/config)"
    flatpak uninstall --user -y io.matt.Basalt
  fi

  log "Installing local bundle: $bundle_path"
  flatpak install --user -y "$bundle_path"

  setup_cli_wrapper

  log "Installed"
  echo "Run with: basalt list"
}

install_from_github_release() {
  log "GitHub release downloading and installing is not yet implemented."
  echo "Please build the project locally first (e.g., ./DevUtils/BuildFlatpak.sh)."
}

main() {
  local script_dir repo_root build_dir bundle_path choice

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$script_dir"
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
