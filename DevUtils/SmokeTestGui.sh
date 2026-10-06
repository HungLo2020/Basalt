#!/usr/bin/env bash
# Offscreen smoke test of the GUI.
#
# Builds the CLI and GUI, creates a throwaway home with offline fixtures (games, ROMs, a sync
# target, and local artwork so nothing is downloaded), then runs the GUI's screenshot tour
# (ScreenshotTour.qml) offscreen. Fails if the app does not exit cleanly, the tour does not
# finish, or the log shows any QML error. Catches problems the compiler and qmllint cannot,
# such as QML type cycles that stop the app from loading.
#
# Usage: DevUtils/SmokeTestGui.sh [screenshot-dir]
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
out_dir="${1:-$(mktemp -d -t basalt-gui-smoke-XXXXXX)}"
mkdir -p "$out_dir"
log_file="$out_dir/gui.log"

log() {
  printf "[smoke-gui] %s\n" "$1"
}

log "Building basalt and basalt-gui"
cargo build --locked --manifest-path "$repo_root/Cargo.toml" -p basalt-cli -p basalt-gui-kirigami
bin_dir="${CARGO_TARGET_DIR:-$repo_root/target}/debug"

home="$(mktemp -d -t basalt-gui-smoke-home-XXXXXX)"
trap 'rm -rf "$home"' EXIT

basalt() {
  env -u XDG_DATA_HOME -u XDG_CONFIG_HOME -u XDG_CACHE_HOME HOME="$home" \
    "$bin_dir/basalt" "$@" > /dev/null
}

make_script() {
  local path="$home/scripts/$1.sh"
  mkdir -p "$home/scripts"
  printf '#!/bin/bash\n%s\n' "${2:-exit 0}" > "$path"
  printf '%s' "$path"
}

log "Creating fixtures in $home"
art_dir="$home/.local/share/basalt/artwork"
roms="$home/Games/Emulators/roms"
sample_art="$repo_root/resources/gameartwork/Pokemon Radical Red.jpeg"
mkdir -p "$home/Games/MattMC" "$roms/gba/bulk" "$roms/snes" "$art_dir" "$home/remote/roms" "$home/remote/saves"

# The tour launches Celeste and expects it to be running for a moment.
basalt add "Celeste" "$(make_script celeste 'sleep 2')"
basalt add "Hollow Knight" "$(make_script hollow_knight)"
basalt add "Stardew Valley" "$(make_script stardew_valley)"
basalt add "Portal 2" 620
printf '#!/bin/bash\nexit 0\n' > "$home/Games/MattMC/run-mattmc.sh"
head -c 512 /dev/zero > "$roms/gba/Pokemon Radical Red.gba"
head -c 512 /dev/zero > "$roms/snes/Chrono Trigger.sfc"
# Enough files for the tour's ROM sync to report progress before finishing.
for i in $(seq 1 200); do head -c 65536 /dev/zero > "$roms/gba/bulk/file$i.bin"; done
basalt discover --mattmc --emulators
basalt add-to-playlist Favorites Celeste
basalt settings set --roms-root "$home/remote/roms" --saves-root "$home/remote/saves"
# Local artwork overrides, so the run needs no network.
for game in "Pokemon Radical Red" "Chrono Trigger" "Portal 2"; do
  cp "$sample_art" "$art_dir/$game.jpeg"
done

# Without network where the system allows it (unprivileged user namespaces).
isolate=()
if unshare -rn true 2> /dev/null; then
  isolate=(unshare -rn)
fi

log "Running the screenshot tour (screenshots in $out_dir)"
status=0
timeout 180 "${isolate[@]}" env -u XDG_DATA_HOME -u XDG_CONFIG_HOME -u XDG_CACHE_HOME \
  HOME="$home" QT_QPA_PLATFORM=offscreen QT_QUICK_BACKEND=software \
  BASALT_SCREENSHOT_DIR="$out_dir" "$bin_dir/basalt-gui" > "$log_file" 2>&1 || status=$?

problems=()
[[ $status -eq 0 ]] || problems+=("basalt-gui exited with status $status")
grep -q "tour complete" "$log_file" || problems+=("the screenshot tour did not finish")
! grep -q "FAILED" "$log_file" || problems+=("a screenshot could not be saved")
# QML errors from our module, plus load-time failures (type cycles, missing modules).
if grep -E "qrc:/qt/qml/org/basalt|TypeError|ReferenceError|Cyclic dependency|is not a type|is not installed|module .* not found" "$log_file"; then
  problems+=("QML errors in the log (see above)")
fi

if [[ ${#problems[@]} -gt 0 ]]; then
  echo "[smoke-gui] FAILED:" >&2
  printf '  - %s\n' "${problems[@]}" >&2
  echo "[smoke-gui] Full log ($log_file):" >&2
  cat "$log_file" >&2
  exit 1
fi

log "Passed: $(grep -c "^qml: screenshot" "$log_file") screens rendered, no QML errors"
