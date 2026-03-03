#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[flatpak-uninstall] %s\n" "$1"
}

remove_path_line() {
  local file="$1"

  [[ -f "$file" ]] || return 0

  sed -i '/^[[:space:]]*export[[:space:]]\+PATH="\$HOME\/.local\/bin:\$PATH"[[:space:]]*$/d' "$file"
}

main() {
  local wrapper_path

  wrapper_path="$HOME/.local/bin/basalt"

  if command -v flatpak >/dev/null 2>&1; then
    if flatpak info io.matt.Basalt >/dev/null 2>&1; then
      log "Uninstalling io.matt.Basalt Flatpak and deleting app data"
      flatpak uninstall --user -y --delete-data io.matt.Basalt
    else
      log "io.matt.Basalt is not installed"
    fi

    if flatpak remotes --columns=name | grep -qx "basalt-origin"; then
      log "Removing basalt-origin Flatpak remote"
      flatpak remote-delete --user basalt-origin || true
    fi
  else
    log "flatpak not found; skipping Flatpak uninstall step"
  fi

  if [[ -f "$wrapper_path" ]]; then
    log "Removing CLI wrapper: $wrapper_path"
    rm -f "$wrapper_path"
  else
    log "CLI wrapper not found: $wrapper_path"
  fi

  remove_path_line "$HOME/.bashrc"
  remove_path_line "$HOME/.zshrc"

  log "Done"
  echo "If your shell is still open, restart it or run: hash -r"
}

main "$@"
