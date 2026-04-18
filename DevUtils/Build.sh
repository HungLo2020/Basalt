#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[build-dispatch] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[build-dispatch] Missing required command: $1" >&2
    exit 1
  fi
}

detect_platform() {
  local os_name machine_arch

  os_name="$(uname -s)"
  machine_arch="$(uname -m)"

  case "$os_name/$machine_arch" in
    Linux/x86_64)
      echo "linux-amd64"
      ;;
    Darwin/arm64|Darwin/aarch64)
      echo "macos-arm64"
      ;;
    MINGW64_NT-*/x86_64|MSYS_NT-*/x86_64|CYGWIN_NT-*/x86_64)
      echo "windows-amd64"
      ;;
    *)
      return 1
      ;;
  esac
}

main() {
  local script_dir repo_root builds_dir platform build_script build_meta powershell_cmd

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  builds_dir="$repo_root/builds"
  build_meta="$builds_dir/latest-build.env"

  require_cmd uname
  platform="$(detect_platform || true)"
  if [[ -z "$platform" ]]; then
    echo "[build-dispatch] Unsupported platform: $(uname -s)/$(uname -m)" >&2
    echo "[build-dispatch] Add a matching build script under DevUtils/BuildScripts/." >&2
    exit 1
  fi

  log "Detected platform: $platform"
  if [[ "$platform" == "windows-amd64" ]]; then
    build_script="$repo_root/DevUtils/BuildScripts/build-${platform}.ps1"
    if [[ ! -f "$build_script" ]]; then
      echo "[build-dispatch] Missing build script for platform '$platform': $build_script" >&2
      exit 1
    fi

    if command -v pwsh >/dev/null 2>&1; then
      powershell_cmd="pwsh"
    elif command -v powershell >/dev/null 2>&1; then
      powershell_cmd="powershell"
    else
      echo "[build-dispatch] Missing required command for Windows build: pwsh or powershell" >&2
      exit 1
    fi

    log "Delegating to: $build_script"
    BASALT_BUILD_META="$build_meta" "$powershell_cmd" -NoProfile -ExecutionPolicy Bypass -File "$build_script"
  else
    build_script="$repo_root/DevUtils/BuildScripts/build-${platform}.sh"
    if [[ ! -f "$build_script" ]]; then
      echo "[build-dispatch] Missing build script for platform '$platform': $build_script" >&2
      exit 1
    fi

    log "Delegating to: $build_script"
    BASALT_BUILD_META="$build_meta" bash "$build_script"
  fi

  if [[ ! -f "$build_meta" ]]; then
    echo "[build-dispatch] Build metadata not generated: $build_meta" >&2
    exit 1
  fi

  # shellcheck disable=SC1090
  source "$build_meta"

  if [[ -z "${BUILD_ARTIFACT_PATH:-}" || ! -f "$BUILD_ARTIFACT_PATH" ]]; then
    echo "[build-dispatch] Build metadata is missing a valid BUILD_ARTIFACT_PATH" >&2
    exit 1
  fi

  log "Done"
  echo "Built platform: ${BUILD_PLATFORM:-unknown}"
  echo "Artifact type: ${BUILD_ARTIFACT_TYPE:-unknown}"
  echo "Artifact path: $BUILD_ARTIFACT_PATH"
  echo "Metadata file: $build_meta"
}

main "$@"
