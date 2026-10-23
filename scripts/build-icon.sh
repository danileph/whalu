#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Keep the master opaque and square; macOS supplies the rounded icon mask.
# Build every standard and Retina representation from the committed master.
ICON_WORK="$(mktemp -d "${TMPDIR:-/tmp}/whalu-icon.XXXXXX")"
trap 'rm -rf "$ICON_WORK"' EXIT
ICONSET="$ICON_WORK/Whalu.iconset"
mkdir -p "$ICONSET"
for size in 16 32 128 256 512; do
  sips -z "$size" "$size" assets/Whalu.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -z "$retina" "$retina" assets/Whalu.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil --convert icns "$ICONSET" --output assets/Whalu.icns
printf 'Built: assets/Whalu.icns\n'
