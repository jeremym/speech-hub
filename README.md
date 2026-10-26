# Speech Hub

**Give anything on your Mac a voice.**

Speech Hub is a menu-bar utility for macOS that reads selected text aloud with a natural, fully
local AI voice. It is also a shared **speech service**: any app, script, Apple Shortcut, or
user-built "Source" can speak through it with one command, using the same voices and the same
pronunciation dictionary.

> Status: milestones 1 to 7a are built and verified on a real Mac (engines, CLI, Sources,
> pronunciation, voice cloning, self-signed release build). Remaining: first public release and
> Homebrew cask. See [PLAN.md](PLAN.md) and [docs/testing.md](docs/testing.md).

## Contents

- [What it does](#what-it-does)
- [Vocabulary](#vocabulary)
- [Keyboard shortcuts](#keyboard-shortcuts)
- [Engines and voices](#engines-and-voices)
- [Sources](#sources)
- [The `speak` command and other integrations](#the-speak-command-and-other-integrations)
- [Privacy and security](#privacy-and-security)
- [Requirements and install](#requirements-and-install)
- [Build from source](#build-from-source)
- [Repository layout](#repository-layout)
- [Documentation](#documentation)
- [License](#license)

## What it does

**Speak what you select.** Highlight text in almost any app, press ⌥⎋, and hear it. Pressing the
key again while it is speaking stops it (configurable to restart instead). Selection capture uses
the macOS Accessibility API and understands the WebKit, Gecko, and Chromium dialects, so it works
in Safari, Firefox, Chrome, Brave, Mail, TextEdit, PDF viewers, and terminals. If nothing readable
is selected you choose what happens: a notice, the Quick Speak panel, or a Source of your choice
(for example "Current clipboard").

**Quick Speak panel.** Press ⌥⇧⎋ for a small command palette. Type text to speak it, run any of
your Sources, speak or open the currently selected text, or open an empty Reader.

**Reader window.** A full window for longer text, with play, pause, stop, speed, a voice picker,
chunk progress, and a "Fix pronunciation" button for any word you select in it.

**Mini player.** A small floating control that appears while speech plays, with pause and stop.

**Sources.** User-built inputs that feed text into the Hub: a saved snippet, a file, the output of
a command, the result of an Apple Shortcut, or a URL. Built in a GUI with templates, a Test
button, import and export, and a trust gate for anything executable. See [Sources](#sources).

**Pronunciation Studio.** One dictionary of substitution rules (plain or regular expression, with
ordering and live preview) that applies everywhere: selection, Reader, Sources, CLI, deep links.

**Voices.** Two bundled engines with 57 voices in 9 languages, a default voice and speaking speed,
per-Source overrides, and voice previews. The optional Plus engine adds voice cloning from a 10
second recording or WAV file.

**Automation.** The `speak` command-line tool, a Unix-socket protocol, `speechhub://` links, two
items in the macOS Services menu, and recipes for Apple Shortcuts, Raycast, Alfred, Keyboard
Maestro, cron, and Makefiles.

**Menu bar app.** No Dock icon, optional launch at login, Preferences with an onboarding wizard
that walks through the one Accessibility permission the app needs.

## Vocabulary

| Term | Meaning |
|---|---|
| Speak Selection | The ⌥⎋ action: read the highlighted text in the front app. |
| Quick Speak | The ⌥⇧⎋ panel: type text, run a Source, or act on the selection. |
| Source | A saved way to get text (file, command, Shortcut, URL, snippet). |
| Reader | The window for listening to longer text with controls. |
| Mini player | The floating pause/stop/speed control shown while speaking. |
| Engine | The speech model that turns text into audio (Lite, Standard, Plus). |
| Voice | One speaker within an engine. |
| Pronunciation rule | A substitution applied to text before it is spoken. |

## Keyboard shortcuts

| Keys | Action |
|---|---|
| ⌥ ⎋ | Speak the selected text (press again to stop) |
| ⌥ ⇧ ⎋ | Open the Quick Speak panel |

Both are configurable in Preferences → Shortcuts. The pane also detects a clash with Apple's
built-in "Speak selection" shortcut and offers to resolve it.

## Engines and voices

Everything runs on the CPU, offline, through [sherpa-onnx](https://github.com/k2-fsa/sherpa-onnx).
Nothing is sent anywhere.

| Profile | Model | Size | First audio* | Notes |
|---|---|---|---|---|
| Lite | Piper (3 voices, int8) | 53 MB | 380 ms | bundled, lowest memory |
| Standard | Kokoro-82M fp32 (54 voices, 9 languages) | 337 MB | 340 ms | bundled, default |
| Plus | Pocket TTS (Kyutai) | 98 MB download | 261 ms | voice cloning; CC-BY-4.0 with a consent notice |

\* Apple M3 Ultra, release build. Real measurements in [docs/perf.md](docs/perf.md).

The Plus engine is never bundled. Preferences → Engines downloads it on request after showing
its license, verifies the hash, and stores it under Application Support. Custom voices are created
from a recording made in the app (microphone permission asked once) or from a WAV file you pick,
and stay on your Mac.

Models unload after a period of idleness and reload on the next request. The first request after
an unload takes about a second longer.

## Sources

A Source is a saved recipe for getting text. Each one has a name, an icon, its own optional voice
and speed, and an action: speak at once or open in the Reader.

| Kind | What it does | Needs approval |
|---|---|---|
| Saved text | Speaks a fixed snippet | no |
| File | Reads a `.txt` or `.md` file you picked (1 MB cap) | no |
| Command | Runs a program with an argument list; stdout is the text. No shell unless you turn it on | yes |
| Apple Shortcut | Runs a Shortcut by name and reads its text result | yes |
| URL | HTTP GET; plain text, JSON via a field path, or HTML reduced to text | no |

Templates give a working Source in one click: Current clipboard, Speak a file, Run a command,
Shortcut output, Fetch URL. Sources run with a 10 second timeout, a 1 MB output cap, a scrubbed
environment, and are marked untrusted on import until you approve them after seeing the exact
executable. Details in [docs/sources.md](docs/sources.md).

Sources appear in the Quick Speak panel and can be run from the CLI (`speak run <name>`), from a
`speechhub://source/<name>` link (with a confirmation sheet), or as the fallback when no text is
selected.

## The `speak` command and other integrations

Preferences → Command Line installs `speak` into `~/.local/bin` (no sudo). It is compatible with
the common flags of Apple's `say`.

```bash
echo "Build finished" | speak          # stdin
speak "Hello, world"                   # positional
speak -f notes.md                      # a file
speak -v af_heart -r 1.2 "Hi"          # voice and rate
pbpaste | speak --open                 # open in the Reader instead of speaking
speak run morning-briefing             # run a saved Source
speak palette                          # open Quick Speak
speak stop
speak status --json
speak voices --json
speak sources list
```

Exit codes: 0 ok, 1 request error, 2 app unavailable, 3 permission or engine not ready, 64 usage.
Requests are capped at 1 MB. The app is launched on demand if it is not running.

Other ways in:

- **Unix socket** at `~/Library/Application Support/Speech Hub/run/hub.sock`, length-prefixed JSON,
  same-user only. Verbs: speak, open, run_source, stop, pause, resume, status, list_voices,
  list_sources, palette. There is no "run this command" verb; only pre-approved Sources run.
- **`speechhub://` links**: `open`, `palette`, `preferences`, `stop`, `speak?text=` (2 KB cap,
  rate limited), `source/<id-or-name>` (asks first).
- **Services menu**: right-click selected text → Services → "Speak with Speech Hub" or "Open in
  Speech Hub Reader". Works in Chromium browsers too.
- **Apple Shortcuts, Raycast, Alfred, Keyboard Maestro, BetterTouchTool, cron**: recipes in
  [docs/shortcuts.md](docs/shortcuts.md).

Full reference: [docs/public-api.md](docs/public-api.md).

## Privacy and security

- No telemetry, accounts, or cloud. The only network traffic is a URL Source you created or a
  model download you approved.
- Submitted text is never written to logs. Logs carry request ids and sizes only.
- Everything from outside is untrusted: deep links, imported Sources, socket requests, HTTP
  bodies, command output. Size caps, timeouts, schema checks, argv-only execution, 0600 socket
  with a uid check, and text rendered as text.
- One OS permission: Accessibility, to read the selection. Microphone only if you record a voice.
- The app is signed with the project's own long-lived certificate, not an Apple Developer ID, so
  Accessibility grants survive updates. It is not notarized.

Trust boundaries and known limits: [docs/security.md](docs/security.md).

## Requirements and install

Requires an Apple Silicon Mac (M1 or later) running macOS 13 or newer. There is no Intel build.

Once the first release is published:

```bash
brew install --cask jeremym/tap/speech-hub     # also puts `speak` on your PATH
```

Or download `SpeechHub-x.y.z.zip` from Releases and unzip into /Applications. Because the app is
not notarized, macOS blocks the first launch:

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

### First run

The onboarding wizard walks through five steps: the Accessibility permission, engine, voice,
shortcuts, and the optional `speak` command. Every step is also in Preferences later.

## Build from source

Requirements: Rust (stable), Node 20+, pnpm, Xcode command line tools.

```bash
pnpm install
scripts/fetch-models.sh        # downloads and hash-verifies the bundled models (~390 MB)
pnpm tauri dev                 # run in dev mode (also builds the `speak` sidecar)
pnpm tauri build               # .app and .dmg in target/release/bundle
cargo test                     # Rust tests
pnpm test                      # frontend tests (Vitest)
pnpm typecheck                 # tsc --noEmit
cargo run --release -p speech-hub --example bench -- standard   # measure first-audio latency
```

For anything you intend to run for real, use `scripts/install-local.sh`. It builds a release,
signs it with the release identity, installs to /Applications, and launches. A debug build has a
hash-pinned signature, so each rebuild looks like a new app to macOS and loses the Accessibility
grant. Run `scripts/create-release-cert.sh` once first to create the self-signed identity.

Release packaging: [docs/release.md](docs/release.md) (`scripts/build-release.sh`,
`scripts/update-tap.sh`).

## Repository layout

```
speech-hub/
├── src/                      # React + TypeScript UI: Reader, Quick Speak, Source Builder,
│                             #   Mini player, Voices, Pronunciation Studio, Preferences, Onboarding
├── src-tauri/                # Rust app: hub, engines, playback, sources, ipc, hotkeys,
│   │                         #   accessibility, settings, services, deep links
│   ├── resources/models/     # bundled Lite + Standard models (fetched by script, not committed)
│   └── Info.plist            # Services menu and URL scheme declarations
├── crates/
│   ├── speechhub-protocol/   # request and response types shared by app and CLI
│   └── speechhub-cli/        # builds `speak`
├── scripts/                  # fetch-models, build-cli, install-local, build-release, create-release-cert, update-tap
├── docs/                     # see below
├── PLAN.md                   # plan of record and milestone status
└── CLAUDE.md                 # conventions for contributors and coding agents
```

Stack: Tauri 2 (Rust, edition 2024), React 19, TypeScript, Vite, Tailwind 4, sherpa-onnx, cpal.

## Documentation

- [docs/public-api.md](docs/public-api.md): the `speak` command, socket protocol, `speechhub://` links
- [docs/shortcuts.md](docs/shortcuts.md): Apple Shortcuts, Services menu, launchers, shell automation
- [docs/sources.md](docs/sources.md): building Sources, kinds, limits, recipes
- [docs/security.md](docs/security.md): trust boundaries and permissions
- [docs/architecture.md](docs/architecture.md): process model, modules, selection capture
- [docs/perf.md](docs/perf.md): measured latency, real-time factor, memory
- [docs/model-licenses.md](docs/model-licenses.md): license of every bundled model and library
- [docs/release.md](docs/release.md): signing and release runbook
- [docs/testing.md](docs/testing.md): the hands-on test matrix
- [docs/decisions.md](docs/decisions.md): why things are the way they are
- [PLAN.md](PLAN.md): plan of record, milestones, backlog

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Bundled models and libraries carry their own licenses,
listed in [docs/model-licenses.md](docs/model-licenses.md).
