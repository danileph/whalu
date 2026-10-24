#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"
if [[ ! -x "$CARGO_BIN" ]]; then CARGO_BIN="$(command -v cargo)"; fi
"$CARGO_BIN" build --release --locked
APP="$PWD/dist/Whalu.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/whalu "$APP/Contents/MacOS/whalu"
cp assets/Info.plist "$APP/Contents/Info.plist"
cp assets/Whalu.icns "$APP/Contents/Resources/Whalu.icns"
cp THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/THIRD_PARTY_NOTICES.md"
codesign --force --sign - --identifier dev.danileph.whalu "$APP"
codesign --verify --strict "$APP"
printf '\nBuilt: %s\nRun: open "%s"\n' "$APP" "$APP"
