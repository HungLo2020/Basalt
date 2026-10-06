#!/usr/bin/env bash
# Lints the GUI's QML with qmllint, failing on any warning.
#
# The QML is compiled by Qt at runtime (see crates/basalt-gui-kirigami/build.rs), so this is the
# build-time check for QML mistakes, including mismatches with the Rust Backend's properties.
# Needs a prior `cargo build`/`cargo check` of basalt-gui-kirigami for Backend's type info, and
# the Kirigami and QtQuick QML modules installed so imports resolve.
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
qt_bins="$(qmake6 -query QT_HOST_BINS)"
qmllint="$qt_bins/qmllint"
backend_types="$repo_root/target/cxxqt/qml_modules"

if [[ ! -x "$qmllint" ]]; then
  echo "[lint-qml] qmllint not found at $qmllint (install qt6-declarative-dev-tools)" >&2
  exit 1
fi
if [[ ! -d "$backend_types/org/basalt/app" ]]; then
  echo "[lint-qml] Backend type info missing; run: cargo check -p basalt-gui-kirigami" >&2
  exit 1
fi

"$qmllint" --max-warnings 0 -I "$backend_types" "$repo_root"/crates/basalt-gui-kirigami/qml/*.qml
echo "[lint-qml] QML is clean"
