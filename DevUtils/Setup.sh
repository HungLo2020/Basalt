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

is_macos() {
  [[ "$(uname -s)" == "Darwin" ]]
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
    cmake \
    clang \
    libssl-dev \
    libasound2-dev \
    libudev-dev \
    libx11-dev \
    libxrandr-dev \
    libxi-dev \
    libxcursor-dev \
    libxinerama-dev \
    libwayland-dev \
    libxkbcommon-dev
}

ensure_homebrew() {
  if command -v brew >/dev/null 2>&1; then
    return 0
  fi

  log "Homebrew not found; installing Homebrew"
  require_cmd curl

  NONINTERACTIVE=1 /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  fi

  require_cmd brew
}

setup_macos() {
  ensure_homebrew

  log "Updating Homebrew"
  brew update

  log "Installing system dependencies"
  brew install \
    ca-certificates \
    curl \
    pkg-config \
    git \
    cmake \
    llvm
}

main() {
  if is_debian_based; then
    setup_debian
  elif is_macos; then
    setup_macos
  else
    echo "[setup] This script currently supports Debian-based Linux distributions and macOS only." >&2
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
