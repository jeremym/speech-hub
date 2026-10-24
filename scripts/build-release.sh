#!/usr/bin/env bash
#
# Build a signed release and package it for a GitHub release:
#   dist/SpeechHub-X.Y.Z.zip (+ .sha256)   ← what the Homebrew cask downloads
#   dist/SpeechHub-X.Y.Z.dmg               ← for people who prefer a DMG
# Signs with an identity already in the keychain. Never touches TCC or trust settings.
#
# Usage: scripts/build-release.sh --version X.Y.Z [--identity NAME]
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=""; IDENTITY="${CODESIGN_IDENTITY:-Speech Hub Release}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --version) VERSION="$2"; shift 2 ;;
        --identity) IDENTITY="$2"; shift 2 ;;
        *) echo "error: unknown argument '$1'" >&2; exit 2 ;;
    esac
done
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "error: --version X.Y.Z is required" >&2; exit 2; }

step() { printf '\n==> %s\n' "$1"; }
APP_NAME="Speech Hub.app"
BUILT="target/release/bundle/macos/$APP_NAME"
OUT="dist"; ZIP="$OUT/SpeechHub-$VERSION.zip"; DMG="$OUT/SpeechHub-$VERSION.dmg"

step "Checking identity '$IDENTITY'"
security find-identity -v -p codesigning 2>/dev/null | grep -qF "$IDENTITY" || { echo "error: identity not found" >&2; exit 1; }

step "Checking versions match $VERSION"
for f in package.json src-tauri/tauri.conf.json; do
    grep -q "\"version\": \"$VERSION\"" "$f" || { echo "error: $f is not at version $VERSION (bump it first)" >&2; exit 1; }
done
grep -q "^version = \"$VERSION\"" src-tauri/Cargo.toml || { echo "error: src-tauri/Cargo.toml is not at $VERSION" >&2; exit 1; }

step "Building signed release (arm64; universal needs the x86_64 Rust target, see docs/release.md)"
APPLE_SIGNING_IDENTITY="$IDENTITY" pnpm tauri build

step "Verifying signature"
DR="$(codesign -d -r- "$BUILT" 2>&1 | tail -1)"
grep -q cdhash <<<"$DR" && { echo "error: cdhash-pinned signature: $DR" >&2; exit 1; }
codesign --verify --strict --deep "$BUILT"
echo "    $DR"

step "Packaging"
mkdir -p "$OUT"; rm -f "$ZIP" "$ZIP.sha256" "$DMG"
ditto -c -k --keepParent "$BUILT" "$ZIP"
cp target/release/bundle/dmg/*.dmg "$DMG" 2>/dev/null || echo "    (no dmg produced)"
( cd "$OUT" && shasum -a 256 "$(basename "$ZIP")" | tee "$(basename "$ZIP").sha256" )
du -h "$ZIP" "$DMG" 2>/dev/null

echo
echo "Next:"
echo "  git tag v$VERSION && git push origin v$VERSION"
echo "  gh release create v$VERSION $ZIP $ZIP.sha256 $DMG --title \"v$VERSION\" --generate-notes"
echo "  scripts/update-tap.sh --tap ~/GitProjects/personal/homebrew-tap --version $VERSION --zip $ZIP"
