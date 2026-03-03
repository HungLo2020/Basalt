#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[build-macos-arm64] %s\n" "$1" >&2
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[build-macos-arm64] Missing required command: $1" >&2
    exit 1
  fi
}

main() {
  local script_dir repo_root builds_dir cargo_toml version build_meta
  local app_name app_bundle app_dir dmg_root dmg_path

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/../.." && pwd)"
  builds_dir="$repo_root/builds"
  cargo_toml="$repo_root/Cargo.toml"
  build_meta="${BASALT_BUILD_META:-$builds_dir/latest-build.env}"

  require_cmd cargo
  require_cmd hdiutil

  if [[ "$(uname -s)" != "Darwin" ]]; then
    echo "[build-macos-arm64] This script must run on macOS." >&2
    exit 1
  fi

  if [[ "$(uname -m)" != "arm64" && "$(uname -m)" != "aarch64" ]]; then
    echo "[build-macos-arm64] This script expects Apple Silicon (arm64)." >&2
    exit 1
  fi

  version="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]\+\)"/\1/p' "$cargo_toml" | head -n1)"
  if [[ -z "$version" ]]; then
    echo "[build-macos-arm64] Unable to determine version from $cargo_toml" >&2
    exit 1
  fi

  log "Clearing builds directory"
  rm -rf "$builds_dir"
  mkdir -p "$builds_dir"

  log "Building Rust release binary"
  cargo build --manifest-path "$cargo_toml" --release --locked --target aarch64-apple-darwin

  app_name="Basalt"
  app_bundle="${app_name}.app"
  app_dir="$builds_dir/$app_bundle"
  dmg_root="$builds_dir/dmg-root"
  dmg_path="$builds_dir/${app_name}-${version}-macos-arm64.dmg"

  mkdir -p "$app_dir/Contents/MacOS"

  cp "$repo_root/target/aarch64-apple-darwin/release/basalt" "$app_dir/Contents/MacOS/$app_name"
  chmod +x "$app_dir/Contents/MacOS/$app_name"

  cat > "$app_dir/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
  <dict>
    <key>CFBundleName</key>
    <string>Basalt</string>
    <key>CFBundleDisplayName</key>
    <string>Basalt</string>
    <key>CFBundleIdentifier</key>
    <string>com.hunglo2020.basalt</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleExecutable</key>
    <string>Basalt</string>
    <key>LSMinimumSystemVersion</key>
    <string>13.0</string>
  </dict>
</plist>
PLIST

  mkdir -p "$dmg_root"
  cp -R "$app_dir" "$dmg_root/"
  ln -s /Applications "$dmg_root/Applications"

  log "Creating DMG"
  hdiutil create \
    -volname "Basalt" \
    -srcfolder "$dmg_root" \
    -ov \
    -format UDZO \
    "$dmg_path"

  cat > "$build_meta" <<EOF
BUILD_PLATFORM=macos-arm64
BUILD_ARTIFACT_TYPE=dmg
BUILD_ARTIFACT_PATH=$dmg_path
BUILD_ARTIFACT_NAME=$(basename "$dmg_path")
EOF

  log "Done"
  echo "Built artifact: $dmg_path" >&2
  echo "Metadata file: $build_meta" >&2
}

main "$@"
