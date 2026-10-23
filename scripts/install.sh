#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
./scripts/build-app.sh
DEST="$HOME/Applications/Whalu.app"
if pgrep -x whalu >/dev/null || pgrep -x magnetic >/dev/null; then
  printf 'Quit Whalu (or the former Magnetic app) from its menu before replacing the installed app.\n' >&2
  exit 1
fi
mkdir -p "$HOME/Applications"
ditto dist/Whalu.app "$DEST"
printf '\nInstalled: %s\n' "$DEST"
open "$DEST"
