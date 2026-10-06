#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[build-linux-amd64] %s\n" "$1" >&2
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[build-linux-amd64] Missing required command: $1" >&2
    exit 1
  fi
}

read_cargo_package_version() {
  local cargo_toml_path="$1"

  awk '
    /^\[(workspace\.)?package\][[:space:]]*$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      line = $0
      sub(/^[[:space:]]*version[[:space:]]*=[[:space:]]*"/, "", line)
      sub(/".*/, "", line)
      print line
      exit
    }
  ' "$cargo_toml_path"
}

main() {
  local script_dir repo_root builds_dir legacy_build_dir cargo_toml version
  local package_root control_file deb_path desktop_file icon_file build_meta

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/../.." && pwd)"
  builds_dir="$repo_root/builds"
  legacy_build_dir="$repo_root/Build"
  cargo_toml="$repo_root/Cargo.toml"
  build_meta="${BASALT_BUILD_META:-$builds_dir/latest-build.env}"

  require_cmd cargo
  require_cmd dpkg-deb
  require_cmd dpkg-shlibdeps

  version="$(read_cargo_package_version "$cargo_toml")"
  if [[ -z "$version" ]]; then
    echo "[build-linux-amd64] Unable to determine version from $cargo_toml" >&2
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

  log "Building Rust release binaries"
  # basalt-gui is the Kirigami GUI (crate basalt-gui-kirigami); it needs Qt 6 development files.
  cargo build --manifest-path "$cargo_toml" --release -p basalt-cli -p basalt-gui-kirigami

  package_root="$builds_dir/pkgroot"
  mkdir -p \
    "$package_root/DEBIAN" \
    "$package_root/usr/bin" \
    "$package_root/usr/share/applications" \
    "$package_root/usr/share/icons/hicolor/scalable/apps"

  install -m 755 "$repo_root/target/release/basalt" "$package_root/usr/bin/basalt"
  install -m 755 "$repo_root/target/release/basalt-gui" "$package_root/usr/bin/basalt-gui"

  desktop_file="$repo_root/resources/packaging/linux/basalt.desktop"
  if [[ ! -f "$desktop_file" ]]; then
    echo "[build-linux-amd64] Missing desktop entry file: $desktop_file" >&2
    exit 1
  fi
  install -m 644 "$desktop_file" "$package_root/usr/share/applications/basalt.desktop"

  # Bundled artwork overrides; users can add their own in ~/.local/share/basalt/artwork.
  mkdir -p "$package_root/usr/share/basalt/artwork"
  find "$repo_root/resources/gameartwork" -maxdepth 1 -type f \
    \( -iname '*.png' -o -iname '*.jpg' -o -iname '*.jpeg' \) \
    -exec install -m 644 -t "$package_root/usr/share/basalt/artwork" {} +

  icon_file="$repo_root/resources/assets/icons/basalt.svg"
  if [[ ! -f "$icon_file" ]]; then
    echo "[build-linux-amd64] Missing icon file: $icon_file" >&2
    echo "[build-linux-amd64] Place the icon SVG at resources/assets/icons/basalt.svg" >&2
    exit 1
  fi
  install -m 644 "$icon_file" "$package_root/usr/share/icons/hicolor/scalable/apps/basalt.svg"

  log "Computing package dependencies"
  local shlibs_dir shlib_depends
  shlibs_dir="$(mktemp -d)"
  mkdir -p "$shlibs_dir/debian"
  printf 'Source: basalt\n\nPackage: basalt\nArchitecture: amd64\n' > "$shlibs_dir/debian/control"
  shlib_depends="$(
    cd "$shlibs_dir" &&
      dpkg-shlibdeps -O "$package_root/usr/bin/basalt" "$package_root/usr/bin/basalt-gui" 2>/dev/null |
      sed -n 's/^shlibs:Depends=//p'
  )"
  rm -rf "$shlibs_dir"
  if [[ -z "$shlib_depends" ]]; then
    echo "[build-linux-amd64] dpkg-shlibdeps did not report any dependencies" >&2
    exit 1
  fi
  # Qt's private ABI would pin the package to one exact Qt version, so the next distribution Qt
  # update would hold Qt back or remove Basalt. The QML is deliberately not compiled
  # ahead-of-time (see crates/basalt-gui-kirigami/build.rs) to avoid it.
  if [[ "$shlib_depends" == *private-abi* ]]; then
    echo "[build-linux-amd64] basalt-gui depends on Qt's private ABI: $shlib_depends" >&2
    echo "[build-linux-amd64] Check that no QML is compiled with qmlcachegen." >&2
    exit 1
  fi

  control_file="$package_root/DEBIAN/control"
  # Shared libraries come from dpkg-shlibdeps; QML modules and image plugins are loaded at
  # runtime, so they are listed by hand.
  cat > "$control_file" <<EOF
Package: basalt
Version: $version
Section: utils
Priority: optional
Architecture: amd64
Maintainer: Basalt Maintainers
Depends: $shlib_depends, qml6-module-org-kde-kirigami, qml6-module-qtquick-controls, qml6-module-qtquick-layouts, qt6-svg-plugins
Recommends: qml6-module-org-kde-desktop, breeze-icon-theme, rsync
Description: Basalt game launcher (basalt CLI and basalt-gui)
EOF

  deb_path="$builds_dir/basalt_${version}_amd64.deb"

  log "Building Debian package"
  dpkg-deb --root-owner-group --build "$package_root" "$deb_path"

  cat > "$build_meta" <<EOF
BUILD_PLATFORM=linux-amd64
BUILD_ARTIFACT_TYPE=deb
BUILD_ARTIFACT_PATH=$deb_path
BUILD_ARTIFACT_NAME=$(basename "$deb_path")
EOF

  log "Done"
  echo "Built artifact: $deb_path" >&2
  echo "Metadata file: $build_meta" >&2
}

main "$@"
