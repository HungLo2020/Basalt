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
  local script_dir repo_root artifact_path tag_name release_title commit_sha timestamp

  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  repo_root="$(cd "$script_dir/.." && pwd)"
  artifact_path=""

  # Always publish to one fixed release tag to avoid accumulating many releases.
  tag_name="latest"

  require_cmd git
  require_cmd gh

  if ! gh auth status >/dev/null 2>&1; then
    echo "[export-github] GitHub CLI is not authenticated. Run: gh auth login" >&2
    exit 1
  fi

  log "Building Debian package"
  "$repo_root/DevUtils/BuildDeb.sh"

  artifact_path="$(ls -1t "$repo_root"/builds/*.deb 2>/dev/null | head -n1 || true)"
  if [[ -z "$artifact_path" || ! -f "$artifact_path" ]]; then
    echo "[export-github] Expected .deb artifact not found in $repo_root/builds" >&2
    exit 1
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
    "$artifact_path" \
    --title "$release_title" \
    --notes "Automated Basalt build exported on $timestamp.\nCommit: $commit_sha"

  log "Done"
  echo "Release published and replaced at tag: $tag_name"
}

main "$@"
