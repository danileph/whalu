#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"
if [[ ! -x "$CARGO_BIN" ]]; then CARGO_BIN="$(command -v cargo)"; fi
"$CARGO_BIN" build --release --locked
APP="$PWD/dist/Magnetic.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/magnetic "$APP/Contents/MacOS/magnetic"
cp assets/Info.plist "$APP/Contents/Info.plist"
cp assets/Magnetic.icns "$APP/Contents/Resources/Magnetic.icns"
codesign --force --sign - --identifier dev.danileph.magnetic "$APP"
codesign --verify --strict "$APP"
printf '\nBuilt: %s\nRun: open "%s"\n' "$APP" "$APP"
