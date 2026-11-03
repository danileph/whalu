#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="$(sed -nE 's/^version = "([^"]+)"/\1/p' Cargo.toml | head -1)"
BUNDLE_VERSION="$(plutil -extract CFBundleShortVersionString raw assets/Info.plist)"
if [[ -z "$VERSION" || "$VERSION" != "$BUNDLE_VERSION" ]]; then
  printf 'Cargo.toml and Info.plist versions must match.\n' >&2
  exit 1
fi
if [[ -n "${RELEASE_TAG:-}" && "$RELEASE_TAG" != "v$VERSION" ]]; then
  printf 'Release tag must be v%s.\n' "$VERSION" >&2
  exit 1
fi

./scripts/build-app.sh --universal
ARCHIVE="Whalu-macOS-universal.zip"
ditto -c -k --sequesterRsrc --keepParent dist/Whalu.app "dist/$ARCHIVE"
VERIFY_DIR="$(mktemp -d)"
trap 'rm -rf "$VERIFY_DIR"' EXIT
ditto -x -k "dist/$ARCHIVE" "$VERIFY_DIR"
codesign --verify --strict "$VERIFY_DIR/Whalu.app"
lipo "$VERIFY_DIR/Whalu.app/Contents/MacOS/whalu" -verify_arch arm64 x86_64
cmp dist/Whalu.app/Contents/MacOS/whalu "$VERIFY_DIR/Whalu.app/Contents/MacOS/whalu"
(
  cd dist
  shasum -a 256 "$ARCHIVE" > "$ARCHIVE.sha256"
)
printf 'Release %s: dist/%s\n' "$VERSION" "$ARCHIVE"
