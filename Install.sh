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
  :
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

  if command -v basalt >/dev/null 2>&1; then
    log "Installed successfully"
    echo "Run with: basalt list"
    echo "If your current shell still points to an old command path, run: hash -r"
  else
    echo "[deb-install] Install finished but 'basalt' is not on PATH in this shell yet." >&2
    echo "[deb-install] Run 'hash -r' or open a new shell, then run: basalt list" >&2
  fi
}

install_from_local_deb() {
  local deb_path="$1"
  install_deb_package "$deb_path"
}

install_from_github_release() {
  local repo_slug api_url response download_url temp_deb

  require_cmd curl
  require_cmd grep

  repo_slug="$(extract_repo_slug)"
  api_url="https://api.github.com/repos/${repo_slug}/releases/latest"

  log "Fetching latest GitHub release metadata from ${repo_slug}"
  response="$(curl -fsSL -H 'Accept: application/vnd.github+json' -H 'User-Agent: Basalt-InstallScript' "$api_url")"

  download_url="$(printf '%s' "$response" | grep -oE '"browser_download_url"[[:space:]]*:[[:space:]]*"[^"]+\.deb"' | head -n1 | sed -E 's/.*"(https:[^"]+\.deb)"/\1/')"

  if [[ -z "$download_url" ]]; then
    echo "[deb-install] No .deb asset found in latest release for ${repo_slug}." >&2
    echo "[deb-install] Expected an uploaded .deb artifact in GitHub releases." >&2
    exit 1
  fi

  temp_deb="$(mktemp --suffix=.deb)"
  trap "rm -f '$temp_deb'" RETURN

  log "Downloading latest release package"
  curl -fL "$download_url" -o "$temp_deb"

  install_deb_package "$temp_deb"
}

main() {
  local script_dir repo_root builds_dir deb_path choice

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$script_dir"
  builds_dir="$repo_root/builds"
  deb_path="$(ls -1t "$builds_dir"/*.deb 2>/dev/null | head -n1 || true)"

  if [[ -n "$deb_path" && -f "$deb_path" ]]; then
    echo "A local Debian package was found at: $deb_path"
    echo "Choose an option:"
    echo "  [1] Install local package"
    echo "  [2] Fetch and install latest GitHub release"

    while true; do
      read -r -p "Enter choice [1/2]: " choice
      case "$choice" in
        1)
          install_from_local_deb "$deb_path"
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
