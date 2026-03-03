#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[deb-build] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[deb-build] Missing required command: $1" >&2
    exit 1
  fi
}

main() {
  local script_dir repo_root builds_dir legacy_build_dir cargo_toml version arch
  local package_root control_file deb_path

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  builds_dir="$repo_root/builds"
  legacy_build_dir="$repo_root/Build"
  cargo_toml="$repo_root/Cargo.toml"

  require_cmd cargo
  require_cmd dpkg-deb

  if command -v dpkg >/dev/null 2>&1; then
    arch="$(dpkg --print-architecture)"
  else
    arch="amd64"
  fi

  version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]\+\)"/\1/p' "$cargo_toml" | head -n1)"
  if [[ -z "$version" ]]; then
    echo "[deb-build] Unable to determine version from $cargo_toml" >&2
    exit 1
  fi

  log "Clearing builds directory"
  rm -rf "$builds_dir"
  mkdir -p "$builds_dir"

  if [[ -d "$legacy_build_dir" ]]; then
    log "Removing legacy Flatpak artifacts from Build/"
    rm -rf "$legacy_build_dir/flatpak-build" "$legacy_build_dir/flatpak-repo"
    find "$legacy_build_dir" -maxdepth 1 -type f -name '*.flatpak' -delete
  fi

  log "Building Rust release binary"
  cargo build --manifest-path "$cargo_toml" --release

  package_root="$builds_dir/pkgroot"
  mkdir -p "$package_root/DEBIAN" "$package_root/usr/bin"

  install -m 755 "$repo_root/target/release/basalt" "$package_root/usr/bin/basalt"

  control_file="$package_root/DEBIAN/control"
  cat > "$control_file" <<EOF
Package: basalt
Version: $version
Section: utils
Priority: optional
Architecture: $arch
Maintainer: Basalt Maintainers
Description: Basalt game launcher CLI
EOF

  deb_path="$builds_dir/basalt_${version}_${arch}.deb"

  log "Building Debian package"
  dpkg-deb --root-owner-group --build "$package_root" "$deb_path"

  log "Done"
  echo "Package created: $deb_path"
  echo "Install with: sudo apt install -y $deb_path"
  echo "Use with: basalt list"
}

main "$@"
