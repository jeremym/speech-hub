#!/usr/bin/env bash
#
# Build Speech Hub (release, signed with the release identity), install to /Applications, launch.
# Use this for anything you intend to actually run: a debug/ad-hoc build has a cdhash-pinned
# designated requirement, so every rebuild would look like a new app to TCC and break the
# Accessibility grant. Run scripts/create-release-cert.sh once first.
#
# Env: CODESIGN_IDENTITY (default "Speech Hub Release"), SKIP_LAUNCH=1
set -euo pipefail
cd "$(dirname "$0")/.."

IDENTITY="${CODESIGN_IDENTITY:-Speech Hub Release}"
APP_NAME="Speech Hub.app"
BUILT="target/release/bundle/macos/$APP_NAME"
INSTALLED="/Applications/$APP_NAME"

step() { printf '\n==> %s\n' "$1"; }

if ! security find-identity -v -p codesigning 2>/dev/null | grep -qF "$IDENTITY"; then
    echo "error: no valid code-signing identity '$IDENTITY'. Run scripts/create-release-cert.sh first." >&2
    exit 1
fi
[ -f src-tauri/resources/models/standard/model.onnx ] || { echo "error: models missing; run scripts/fetch-models.sh" >&2; exit 1; }

step "Quitting any running instance"
pkill -x speech-hub 2>/dev/null || true
sleep 1

step "Building release signed as '$IDENTITY' (first build takes several minutes: LTO + 390 MB of models)"
APPLE_SIGNING_IDENTITY="$IDENTITY" pnpm tauri build --bundles app

step "Checking the signature"
DR="$(codesign -d -r- "$BUILT" 2>&1 | tail -1)"
if grep -q cdhash <<<"$DR"; then
    echo "error: designated requirement is cdhash-pinned; signing did not take: $DR" >&2
    exit 1
fi
codesign --verify --strict --deep "$BUILT"
codesign -dv "$BUILT/Contents/MacOS/speak" 2>&1 | grep -E "^Authority|^Identifier" | head -2
echo "    $DR"

step "Installing to $INSTALLED"
rm -rf "$INSTALLED"
cp -R "$BUILT" /Applications/
xattr -dr com.apple.quarantine "$INSTALLED" 2>/dev/null || true

if [[ "${SKIP_LAUNCH:-0}" != "1" ]]; then
    step "Launching"
    open -a "$INSTALLED"
fi
echo
echo "Installed. First time: grant Accessibility when asked (System Settings → Privacy & Security)."
echo "The speak CLI: Preferences → Command Line → Install, or: ln -sf '$INSTALLED/Contents/MacOS/speak' ~/.local/bin/speak"
