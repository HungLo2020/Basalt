#!/usr/bin/env bash
set -euo pipefail

log() {
  printf "\n[run-release] %s\n" "$1"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "[run-release] Missing required command: $1" >&2
    exit 1
  fi
}

main() {
  require_cmd gh

  if ! gh auth status >/dev/null 2>&1; then
    echo "[run-release] GitHub CLI is not authenticated. Run: gh auth login" >&2
    exit 1
  fi

  log "Triggering release-latest workflow"
  gh workflow run release-latest.yml

  log "Done — workflow dispatched. Monitor progress at:"
  gh browse --no-browser 2>/dev/null || true
  echo "  https://github.com/$(gh repo view --json nameWithOwner -q .nameWithOwner)/actions"
}

main "$@"
