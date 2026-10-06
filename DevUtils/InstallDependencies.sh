#!/usr/bin/env bash
# Installs the apt packages listed in apt-dependencies.txt (uses sudo unless already root).
set -euo pipefail

list="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/apt-dependencies.txt"
mapfile -t packages < <(sed 's/#.*//' "$list" | xargs -n1)

as_root=()
if [[ $EUID -ne 0 ]]; then
  as_root=(sudo)
fi

"${as_root[@]}" apt-get update
"${as_root[@]}" apt-get install -y --no-install-recommends "${packages[@]}"
