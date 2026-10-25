# Release runbook

Releases are built and signed **locally**. No Apple Developer ID, no notarization, no CI secrets.
Integrity = sha256 of the zip + the signing certificate's leaf hash. Same pattern as ClipySecure.

> **Recorded values**
> - Signing identity: `Speech Hub Release` (self-signed, 20 years)
> - Certificate leaf SHA-1: `b6025fb03507222832dcb4303acaa4ac194fbd67` (created 2026-10-24)

## Why self-signed works
TCC keys the Accessibility grant to the designated requirement
`identifier "com.jeremym.speechhub" and certificate leaf = H"…"`. A stable self-signed cert keeps
that hash constant across releases, so users grant once. Ad-hoc signing pins to the cdhash and
breaks the grant every build. For cask installs Gatekeeper is handled by the cask stripping
quarantine; zip users go through Privacy & Security → Open Anyway (see README).

## One-time
1. `scripts/create-release-cert.sh` ⚠️ two login-password dialogs (keychain import, trust
   settings). Store the `.p12` backup in the password manager and delete it from disk.
2. `scripts/install-local.sh` → installs the signed app to /Applications. Grant Accessibility once.
3. Record the leaf SHA-1 above and in README "Verifying a download".

## Each release
```bash
# bump version in package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml (same X.Y.Z)
scripts/build-release.sh --version X.Y.Z          # dist/SpeechHub-X.Y.Z.zip + .sha256 + .dmg
git tag vX.Y.Z && git push origin vX.Y.Z
gh release create vX.Y.Z dist/SpeechHub-X.Y.Z.zip dist/SpeechHub-X.Y.Z.zip.sha256 dist/SpeechHub-X.Y.Z.dmg --title "vX.Y.Z" --generate-notes
scripts/update-tap.sh --tap ~/GitProjects/personal/homebrew-tap --version X.Y.Z --zip dist/SpeechHub-X.Y.Z.zip
# review, then commit + push the tap yourself
```
Users: `brew install --cask jeremym/tap/speech-hub` (also puts `speak` on PATH).

## Verifying a download
```bash
shasum -a 256 SpeechHub-X.Y.Z.zip                      # matches the .sha256 asset
codesign -d -r- "/Applications/Speech Hub.app"          # leaf hash matches the value above
```

## Notes
- **Apple Silicon only (decision, 2026-10-25).** No universal build for v1. Reasons: no Intel Mac to
  verify on (an untested x86_64 slice would be faking it), a universal bundle roughly doubles the
  sherpa-onnx/ONNX Runtime payload, and the Homebrew toolchain has no `x86_64-apple-darwin` target.
  Revisit only on a concrete request; the recipe is rustup + `pnpm tauri build --target universal-apple-darwin`.
- Hardened runtime is on (Tauri default). The only entitlement is `com.apple.security.device.audio-input`
  (`Entitlements.plist`) for "Record a voice"; audio output, Accessibility and the socket need none.
- Tauri skips notarization unless `APPLE_ID`/`APPLE_API_KEY` env vars are set. Leave them unset.
