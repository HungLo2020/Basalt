#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[flatpak-install] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[flatpak-install] Missing required command: $1" >&2
    exit 1
  fi
}

extract_repo_slug() {
  local origin_url slug

  if [[ -n "${BASALT_GITHUB_REPO:-}" ]]; then
    echo "$BASALT_GITHUB_REPO"
    return
  fi

  if command -v git >/dev/null 2>&1; then
    if origin_url="$(git -C "$repo_root" remote get-url origin 2>/dev/null)"; then
      if [[ "$origin_url" =~ github.com[:/]([^/]+/[^/.]+)(\.git)?$ ]]; then
        slug="${BASH_REMATCH[1]}"
        echo "$slug"
        return
      fi
    fi
  fi

  echo "HungLo2020/Basalt"
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

install_bundle() {
  local bundle_path="$1"

  require_cmd flatpak

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

install_from_local_bundle() {
  local bundle_path="$1"
  install_bundle "$bundle_path"
}

install_from_github_release() {
  local repo_slug api_url response download_url temp_bundle

  require_cmd curl
  require_cmd grep

  repo_slug="$(extract_repo_slug)"
  api_url="https://api.github.com/repos/${repo_slug}/releases/latest"

  log "Fetching latest GitHub release metadata from ${repo_slug}"
  response="$(curl -fsSL -H 'Accept: application/vnd.github+json' -H 'User-Agent: Basalt-InstallScript' "$api_url")"

  download_url="$(printf '%s' "$response" | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+\.flatpak"' | head -n1 | sed -E 's/.*"(https:[^"]+\.flatpak)"/\1/')"

  if [[ -z "$download_url" ]]; then
    echo "[flatpak-install] No .flatpak asset found in latest release for ${repo_slug}." >&2
    echo "[flatpak-install] Expected an uploaded .flatpak artifact in GitHub releases." >&2
    exit 1
  fi

  temp_bundle="$(mktemp --suffix=.flatpak)"
  trap "rm -f '$temp_bundle'" RETURN

  log "Downloading latest release bundle"
  curl -fL "$download_url" -o "$temp_bundle"

  install_bundle "$temp_bundle"
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
