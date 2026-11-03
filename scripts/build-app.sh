#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
CARGO_BIN="${CARGO:-$HOME/.cargo/bin/cargo}"
if [[ ! -x "$CARGO_BIN" ]]; then CARGO_BIN="$(command -v cargo)"; fi
if [[ $# -gt 1 || ( $# -eq 1 && "$1" != "--universal" ) ]]; then
  printf 'Usage: %s [--universal]\n' "$0" >&2
  exit 2
fi
APP="$PWD/dist/Whalu.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
export MACOSX_DEPLOYMENT_TARGET=15.0
if [[ "${1:-}" == "--universal" ]]; then
  "$CARGO_BIN" build --release --locked --target aarch64-apple-darwin
  "$CARGO_BIN" build --release --locked --target x86_64-apple-darwin
  lipo -create target/aarch64-apple-darwin/release/whalu \
    target/x86_64-apple-darwin/release/whalu -output "$APP/Contents/MacOS/whalu"
  lipo "$APP/Contents/MacOS/whalu" -verify_arch arm64 x86_64
else
  "$CARGO_BIN" build --release --locked
  cp target/release/whalu "$APP/Contents/MacOS/whalu"
fi
cp assets/Info.plist "$APP/Contents/Info.plist"
cp assets/Whalu.icns "$APP/Contents/Resources/Whalu.icns"
cp THIRD_PARTY_NOTICES.md "$APP/Contents/Resources/THIRD_PARTY_NOTICES.md"
codesign --force --sign - --identifier dev.danileph.whalu "$APP"
codesign --verify --strict "$APP"
printf '\nBuilt: %s\nRun: open "%s"\n' "$APP" "$APP"
