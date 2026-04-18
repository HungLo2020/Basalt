#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[export-github] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[export-github] Missing required command: $1" >&2
    exit 1
  fi
}

main() {
  local script_dir repo_root builds_dir build_meta
  local artifact_path artifact_name build_platform
  local tag_name release_title commit_sha timestamp

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  builds_dir="$repo_root/builds"
  build_meta="$builds_dir/latest-build.env"

  # Always publish to one fixed release tag to avoid accumulating many releases.
  tag_name="latest"

  require_cmd git
  require_cmd gh

  if ! gh auth status >/dev/null 2>&1; then
    echo "[export-github] GitHub CLI is not authenticated. Run: gh auth login" >&2
    exit 1
  fi

  log "Building local platform artifact"
  "$repo_root/DevUtils/Build.sh"

  if [[ ! -f "$build_meta" ]]; then
    echo "[export-github] Build metadata file not found: $build_meta" >&2
    exit 1
  fi

  # shellcheck disable=SC1090
  source "$build_meta"

  artifact_path="${BUILD_ARTIFACT_PATH:-}"
  artifact_name="${BUILD_ARTIFACT_NAME:-}"
  build_platform="${BUILD_PLATFORM:-}"

  if [[ -z "$artifact_path" || ! -f "$artifact_path" ]]; then
    echo "[export-github] Local build artifact path is invalid: ${artifact_path:-<empty>}" >&2
    exit 1
  fi

  if [[ -z "$build_platform" ]]; then
    echo "[export-github] Build metadata missing BUILD_PLATFORM" >&2
    exit 1
  fi

  if [[ -z "$artifact_name" ]]; then
    artifact_name="$(basename "$artifact_path")"
  fi

  timestamp="$(date -u +"%Y-%m-%d %H:%M:%S UTC")"
  commit_sha="$(git -C "$repo_root" rev-parse --short HEAD)"
  release_title="Basalt Latest Build ($commit_sha)"

  if gh release view "$tag_name" >/dev/null 2>&1; then
    log "Deleting previous '$tag_name' release"
    gh release delete "$tag_name" --yes --cleanup-tag
  fi

  log "Creating new '$tag_name' release"
  gh release create "$tag_name" \
    "$artifact_path#$artifact_name" \
    --title "$release_title" \
    --notes "Automated Basalt build exported on $timestamp.\nCommit: $commit_sha\nLocal platform: $build_platform"

  case "$build_platform" in
    linux-amd64)
      log "Triggering CI workflow for macOS arm64 DMG"
      gh workflow run "release-macos-dmg.yml" -f release_tag="$tag_name"
      log "Triggering CI workflow for Windows amd64 installer"
      gh workflow run "release-windows-installer.yml" -f release_tag="$tag_name"
      ;;
    macos-arm64)
      log "Triggering CI workflow for Linux amd64 DEB"
      gh workflow run "release-linux-deb.yml" -f release_tag="$tag_name"
      log "Triggering CI workflow for Windows amd64 installer"
      gh workflow run "release-windows-installer.yml" -f release_tag="$tag_name"
      ;;
    windows-amd64)
      log "Triggering CI workflow for Linux amd64 DEB"
      gh workflow run "release-linux-deb.yml" -f release_tag="$tag_name"
      log "Triggering CI workflow for macOS arm64 DMG"
      gh workflow run "release-macos-dmg.yml" -f release_tag="$tag_name"
      ;;
    *)
      echo "[export-github] Unsupported BUILD_PLATFORM in metadata: $build_platform" >&2
      exit 1
      ;;
  esac

  log "Done"
  echo "Release published and replaced at tag: $tag_name"
  echo "Uploaded local artifact: $artifact_name"
  echo "Local platform: $build_platform"
}

main "$@"
