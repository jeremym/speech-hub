# Speech Hub

**Give anything on your Mac a voice.**

Speech Hub is a menu-bar utility for macOS that reads selected text aloud with a natural, fully local AI voice. It is also a shared speech service: any app, script, Apple Shortcut, or user-built "Source" can speak through it with one command.

> Status: feature-complete through Milestone 6 (engines, CLI, Sources, pronunciation, voice cloning). Not yet signed or notarized. See [PLAN.md](PLAN.md).

## What makes it different

- **One voice layer for the whole Mac.** `echo "done" | speak` from any tool. No model embedding, no Python, no server.
- **User-built Sources.** Read a file, a command's output, a Shortcut's result, or a URL. Built in a GUI, no code.
- **One pronunciation dictionary.** Fix how a term is said once; it applies everywhere.
- **Runs on anything.** Lite (Piper) and Standard (Kokoro) engines are bundled, CPU-only, offline. Optional Plus tier for voice cloning.
- **Private.** No telemetry, accounts, or cloud. Text is never logged.

## Shortcuts

| Keys | Action |
|---|---|
| ⌥ ⎋ | Speak the selected text |
| ⌥ ⇧ ⎋ | Quick Speak palette |

Both are configurable.

## Engines

| Profile | Model | Size | First audio* | Notes |
|---|---|---|---|---|
| Lite | Piper (3 voices, int8) | 53 MB | 380 ms | bundled, runs on anything |
| Standard | Kokoro-82M fp32 (54 voices, 9 languages) | 337 MB | 340 ms | bundled, default on Apple Silicon |
| Plus | Pocket TTS (Kyutai) | 98 MB download | 261 ms | voice cloning from a 10 s WAV; CC-BY-4.0 |

\* M3 Ultra, release build. See [docs/perf.md](docs/perf.md).

## Build

Requirements: Rust (stable), Node 20+, pnpm, Xcode command line tools.

```bash
pnpm install
scripts/fetch-models.sh        # downloads + verifies the bundled models (~390 MB)
pnpm tauri dev                 # run (also builds the `speak` sidecar)
pnpm tauri build               # .app + .dmg in target/release/bundle
cargo test                     # Rust tests
pnpm test                      # frontend tests
cargo run --release -p speech-hub --example bench -- standard   # measure
```

## Docs

- [docs/public-api.md](docs/public-api.md): the `speak` command, socket protocol, `speechhub://` links
- [Shortcuts, Services menu, and automation](docs/shortcuts.md)
- [docs/sources.md](docs/sources.md): building Sources
- [docs/security.md](docs/security.md): trust boundaries
- [docs/architecture.md](docs/architecture.md), [docs/perf.md](docs/perf.md), [docs/model-licenses.md](docs/model-licenses.md)

## Install

```bash
brew install --cask jeremym/tap/speech-hub     # also puts `speak` on your PATH
```
Or download `SpeechHub-x.y.z.zip` from Releases and unzip into /Applications. The app is signed with
the project's own certificate, not notarized, so macOS blocks the first launch:

1. Double-click Speech Hub once; dismiss the "cannot be opened" dialog.
2. System Settings → Privacy & Security → scroll to Security → **Open Anyway** → confirm.

Right-click → Open no longer bypasses Gatekeeper on macOS 15 and later. Terminal alternative:
`xattr -dr com.apple.quarantine "/Applications/Speech Hub.app"`.

### Verifying a download
```bash
shasum -a 256 SpeechHub-x.y.z.zip                 # must match the .sha256 asset on the release
codesign -d -r- "/Applications/Speech Hub.app"    # must print:
#   designated => identifier "com.jeremym.speechhub" and certificate leaf = H"b6025fb03507222832dcb4303acaa4ac194fbd67"
```

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Bundled models and libraries carry their own licenses; see `docs/model-licenses.md` (Milestone 2).
