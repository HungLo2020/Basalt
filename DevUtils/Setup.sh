#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[setup] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[setup] Missing required command: $1" >&2
    exit 1
  fi
}

is_debian_based() {
  [[ -f /etc/os-release ]] || return 1

  # shellcheck disable=SC1091
  source /etc/os-release

  [[ "${ID:-}" == "debian" || "${ID_LIKE:-}" == *"debian"* || "${ID:-}" == "ubuntu" ]]
}

setup_debian() {
  require_cmd sudo
  require_cmd apt-get

  log "Updating apt package index"
  sudo apt-get update

  log "Installing system dependencies"
  sudo apt-get install -y \
    ca-certificates \
    curl \
    build-essential \
    pkg-config \
    git \
    dpkg-dev \
    libudev-dev \
    qt6-base-dev \
    qt6-base-dev-tools \
    qt6-declarative-dev \
    qt6-declarative-dev-tools \
    qml6-module-org-kde-kirigami \
    qml6-module-org-kde-desktop \
    qml6-module-qtquick-controls \
    qml6-module-qtquick-layouts \
    qt6-svg-plugins
}

main() {
  if is_debian_based; then
    setup_debian
  else
    echo "[setup] This script currently supports Debian-based Linux distributions only." >&2
    exit 1
  fi

  if ! command -v rustup >/dev/null 2>&1; then
    log "Installing rustup (minimal profile)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
  else
    log "rustup already installed; updating"
    rustup self update
  fi

  # shellcheck disable=SC1090
  source "$HOME/.cargo/env"

  log "Installing/Updating Rust stable toolchain"
  rustup toolchain install stable
  rustup default stable
  rustup component add rustfmt clippy

  log "Verifying toolchain"
  rustc --version
  cargo --version

  log "Setup complete"
}

main "$@"
