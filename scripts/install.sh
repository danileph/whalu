#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-app.sh
DEST="$HOME/Applications/Magnetic.app"
if pgrep -x magnetic >/dev/null; then
  printf 'Quit Magnetic from its menu before replacing the installed app.\n' >&2
  exit 1
fi
mkdir -p "$HOME/Applications"
ditto dist/Magnetic.app "$DEST"
printf '\nInstalled: %s\n' "$DEST"
open "$DEST"
