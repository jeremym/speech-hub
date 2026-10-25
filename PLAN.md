# Speech Hub — Plan

> Give anything on your Mac a voice. One local speech engine, shared by every app, script, and shortcut.

Status: **M1–M6 built and verified end-to-end through the CLI** (2026-10-24). M7 (signing, notarization, DMG, CI) remains. Hands-on tests that need a human (Accessibility, hotkeys, windows) are deferred until after signing by design. See `docs/perf.md` for measured numbers. This document supersedes the earlier "SpeakLocal" spec draft; that draft's ideas are kept where they hold up and changed where noted in §1.

---

## 0. TL;DR

- **What:** a menu-bar macOS utility that speaks selected text with a local AI voice, **plus** a tiny CLI / socket API so any other app can speak through it, **plus** user-built "Sources" (file, command, Shortcut, URL) that feed text in.
- **Why it's different:** competitors (aloud-tts, Sotto, Kokori, MLXRead) are *readers*. We're the *shared voice layer*. The hub + CLI + Sources + one shared pronunciation dictionary is the moat, not voice quality.
- **Hardware:** CPU-only by design. Two engine profiles: **Lite** (Piper-class, ~20–60 MB, instant, runs on anything incl. Intel) and **Standard** (Kokoro-82M int8, ~90 MB, the "good" voice). Optional **Plus** tier (Pocket TTS) for voice cloning, downloaded on demand.
- **Order of work:** real selected-text speech → bundled model → CLI/IPC → Source Builder → pronunciation → cloning → release.
- **Decisions needed from Jeremy** before scaffolding: §2.

---

## 1. Review of the draft spec

### What holds up
- The core thesis is right: **don't compete on TTS quality, compete on being the universal voice layer.** Research confirms several Kokoro menu-bar readers already exist; none are an open, documented, local speech service with user-defined Sources.
- Removing clipboard from core is a good architectural call. It falls out naturally once "command Source" exists.
- Tauri 2 is viable: global-shortcut, single-instance, and deep-link plugins are all stable (v2.0.0, Oct 2024). Tray/menu-bar-only apps are supported.
- Kokoro-82M via sherpa-onnx is real and maintained; Rust bindings exist (`sherpa-rs`, official `sherpa-onnx` crate, `pipecrab-tts-sherpa`).
- Unix socket IPC, versioned JSON protocol, "CLI is a thin client" — all correct.
- The security posture (deep links untrusted, command Sources opt-in, no shell interpolation) is correct and should stay.

### What I'd change
1. **Name.** Folder is `speech-hub`; spec says "SpeakLocal" with "Speech Hub" as an internal term. Pick one. I recommend **Speech Hub** as the product and a short CLI verb (see §2).
2. **Licensing trap the spec missed.** Kokoro in sherpa-onnx phonemizes via **espeak-ng, which is GPLv3**. Statically linking it makes the *distributed binary* GPL-covered even if our code is MIT. Options: (a) ship under GPLv3 (it's open source anyway), (b) MIT source + GPL binary notice, (c) replace the phonemizer later. Decide up front; don't discover it at release.
3. **Hardware tiers.** The spec says "lightweight" but assumes Kokoro everywhere. Make it explicit: **Lite / Standard / Plus** engine profiles with auto-detection and a one-click switch (§4). This is also how Intel Macs get supported without blocking Apple Silicon.
4. **Selection-capture fallback.** The spec bans a clipboard fallback. Accessibility selection *will* fail in some apps (Electron apps with AX disabled, some Java apps, some PDF viewers). Keep core clipboard-free, but let the user pick a **"fallback Source"** that runs when selection capture fails (default: none; one-click template: `pbpaste`). Architecturally identical to any other Source, so no purity loss.
5. **`say` compatibility.** macOS already has a `say` CLI that thousands of scripts use. Make our CLI accept the common `say` flags (`-v`, `-r`, `-f`, `-o`, positional text). That's a zero-cost adoption path and a real differentiator for developers.
6. **Scope discipline.** Seven phases is fine as a map, but the product is Phases 1–3 and the differentiator is Phase 4. Treat 5–7 as "after it's in daily use." Voice cloning (Phase 6) depends on gated, CC-BY-4.0 weights and should be download-on-demand, never bundled.
7. **Missing integration surfaces** worth adding later (cheap wins): macOS **Services menu** entry ("Speak with Speech Hub" in any right-click menu), and **Shortcuts actions** (App Intents) so Speech Hub is a *provider* inside Shortcuts, not just a consumer. Both need a small Swift target; defer, but leave room.
8. **Streaming honesty.** Kokoro is sentence-level, not token-streaming. First audible speech = time to synthesize sentence 1. Pipeline: segment → synth sentence N+1 while playing N. Document it as chunked, not streaming.

---

## 2. Decisions (final, 2026-10-24)

| # | Decision | Choice |
|---|---|---|
| 1 | Product name | **Speech Hub** |
| 2 | CLI binary | **`speak`** |
| 3 | URL scheme | `speechhub://` |
| 4 | Bundle id | `com.jeremym.speechhub` |
| 5 | License | **GPL-3.0-or-later** for the whole project. Reason: phonemization goes through piper-phonemize/espeak-ng (GPLv3), so the shipped binary is GPL-covered regardless. (sherpa-onnx 2.0 itself dropped espeak-ng on 2026-10-20; we link the phonemizer separately because its lexicon-only fallback silently *skips* out-of-vocabulary words, which is unacceptable for technical text.) One license, no dual notice. Integrators talk to us over a process boundary. Reversible later by swapping the phonemizer behind its trait. |
| 6 | Models | **Bundle Lite + Standard.** Standard = Kokoro-82M **fp32** (337 MB; int8 measured 3× slower on Apple Silicon). Lite = three Piper medium-int8 voices (53 MB). Plus (Pocket TTS int8, 98 MB download) is download-on-demand. |
| 7 | First launch | **Mandatory wizard**, every pane is a reused Preferences pane: Welcome → Accessibility permission → Model picker (Handy-style: name, size, quality/speed badges, bundled vs download) → Voice preview → Hotkeys (with Apple Speak Selection conflict check) → optional CLI install → Done. |
| 8 | Default profile | Standard on Apple Silicon; wizard pre-selects Lite on Intel or low memory; always overridable. |
| 9 | Settings | JSON in `~/Library/Application Support/Speech Hub/` |

## 3. Architecture

```
  Selected text (AX)   CLI (`speak`)   Shortcuts / Services   speechhub://   Sources
         │                 │                   │                 │            │
         └─────────────────┴───────────┬───────┴─────────────────┴────────────┘
                                       ▼
                               ┌──────────────┐
                               │  Speech Hub  │  validate → normalize → pronounce → segment
                               └──────┬───────┘
                                      ▼
                         ┌───────────────────────┐
                         │ Engine (trait object)  │  Lite | Standard | Plus
                         └──────────┬────────────┘
                                    ▼
                         ┌───────────────────────┐
                         │ Playback (cpal/rodio)  │  play/pause/resume/stop, chunk queue
                         └───────────────────────┘
```

**Process model:** one long-running Tauri app (menu-bar only). It owns the model, the socket, the hotkeys, and all windows. The CLI is a ~1 MB static Rust binary that connects to the socket, sends one JSON frame, prints the reply, exits. If the app isn't running, the CLI launches it (`open -b <bundle-id>`) and retries with backoff for up to ~8 s.

**Rust crates**
- `crates/speechhub-protocol` — request/response types, versioning, size limits. Shared by app + CLI.
- `crates/speechhub-cli` — the `speak` binary. No Tauri dependency.
- `src-tauri/` — app: `hub/`, `engines/`, `playback/`, `sources/`, `ipc/`, `hotkeys/`, `accessibility/`, `settings/`.

**Frontend windows** (React, all small): Reader, Quick Speak palette, Mini Player, Source Builder, Voices, Pronunciation Studio, Preferences, Onboarding.

**State machine** (single source of truth in Rust, mirrored to UI via Tauri events): `Idle → LoadingModel → Ready → LoadingSource → Synthesizing ⇄ Playing ⇄ Paused → Cancelling → Ready | Error`.

---

## 4. Engine profiles (the "runs on anything" answer)

| Profile | Model | Size on disk | RSS (measured) | First audio (M3 Ultra) | Notes |
|---|---|---|---|---|---|
| **Lite** | Piper VITS medium int8: Amy, Ryan, Lessac | 53 MB | 85 MB idle / 247 MB peak | 380 ms | Bundled. Intel-friendly. |
| **Standard** | Kokoro-82M v1.0 fp32, 54 voices | 337 MB | 563 MB idle / 729 MB peak | 340 ms | Bundled. Default on Apple Silicon. |
| **Plus** | Pocket TTS int8 (Kyutai) | 194 MB installed | TBD | ~260 ms (spike) | Voice cloning. CC-BY-4.0, gated upstream. **Download on demand, with license + consent screen.** |

*Numbers from `docs/perf.md`; re-measure on an M1 8 GB before release.*

Rules:
- One `Engine` trait: `load / unload / voices / capabilities / synthesize(chunk, voice, speed, cancel_token) / metadata`.
- Only one model resident at a time by default; switching profiles unloads the other.
- Auto-pick on first launch: Intel → Lite; Apple Silicon with <8 GB free at launch → offer Lite; otherwise Standard. Always overridable in Preferences.
- Idle unload after N minutes (configurable) so a menu-bar app doesn't pin 400 MB forever on an 8 GB machine. Reload is the "LoadingModel" state and the mini player shows it.
- Experiment later: ONNX Runtime CoreML execution provider for ANE offload. Not a dependency.

---

## 5. Public interface

### CLI (`speak`)
```bash
echo "Hello" | speak                 # stdin, speak now
speak "Hello, world"                 # positional (say-compatible)
speak -f notes.md                    # file (say-compatible)
speak -v narrator -r 1.2 "Hi"        # voice, rate
pbpaste | speak --open               # open in Reader instead of speaking
speak run morning-briefing           # run a saved Source
speak palette                        # open Quick Speak
speak stop
speak status --json
speak voices --json
speak sources list
speak -o out.wav "Hello"             # render to file, no playback (later)
```
- Exit codes: 0 ok, 1 request error, 2 app unavailable, 3 permission/engine not ready, 64 usage.
- Input cap: 1 MB UTF-8 per request (documented). Errors → stderr. Never logs text.
- Install: Preferences → "Install Command Line Tool" symlinks the bundled binary into `~/.local/bin` (or `/usr/local/bin` if writable), shows PATH hint if needed. No sudo.

### Socket protocol v1
- Path: `~/Library/Application Support/Speech Hub/run/hub.sock` (dir 0700, socket 0600; refuse symlinks; unlink stale on start after a connect-probe).
- Framing: 4-byte big-endian length + JSON. One request per connection for v1 (simple, race-free).
- Peer check: `getpeereid` must match our uid.
- Requests: `speak`, `open`, `run_source`, `stop`, `pause`, `resume`, `status`, `list_voices`, `list_sources`, `palette`. Every request carries `version`, `request_id`; every response `ok | error{code,message}`.
- `run_source` is the only privileged verb and it only references a Source *by id* that the user already created and approved. There is no "execute this command" verb. Ever.

### URL scheme `speechhub://`
- `open`, `palette`, `source/<id>` (confirmation sheet before running anything executable), `speak?text=` capped at 2 KB and rate-limited. Implemented after CLI works.

### Playback policy v1
New `speak` replaces current playback. `stop` cancels inference + audio. Obsolete chunks are dropped by request_id. Queueing is a later, opt-in feature.

---

## 6. Sources

A Source = `{id, name, icon, type, config, voice?, speed?, action: speak|open, enabled, trusted}`.

Types in v1: **Text**, **File**, **Command** (exe + argv array + cwd, no shell), **Shortcut** (`shortcuts run <name>` with stdout capture; verify behavior on current macOS), **HTTP GET** (plain text, or JSON + a simple field path). Advanced opt-in: Command with `shell: true`.

Safety: 10 s default timeout, 1 MB output cap, show exact executable before first run, `trusted=false` on import until the user approves, env scrubbed to a minimal allowlist, no network unless type is HTTP.

Templates (one click): Speak a file · Run a command · Shortcut output · Fetch URL · **Current clipboard (`pbpaste`)**.

Fallback Source hook: Preferences → "When selected text can't be read, run: [none | pick a Source]".

---

## 7. Security boundaries (summary)

Untrusted: deep links, imported Sources, socket requests, HTTP bodies, command stdout, picked files, all text. Enforced: size caps, timeouts, schema + version validation, argv-only exec, 0600 socket + uid check, text rendered as text (never HTML), no public verb executes anything the user didn't pre-configure. Full write-up lands in `docs/security.md` with Phase 3.

---

## 8. Milestones

Each milestone ends with running code on a real Mac and a short demo note in `.context/`.

Progress legend: ✅ built + verified · 🟡 built, needs hands-on test · ⬜ not started

**M1 — Native shell (Phase 1)** 🟡 (code complete; AX capture, hotkeys and window focus behavior await a human after signing)
Tauri menu-bar app · AX selection capture (`AXUIElement`: `AXSelectedText`, `AXSelectedTextRange`, and the WebKit/Gecko `AXSelectedTextMarkerRange` dialect, with on-demand `AXEnhancedUserInterface` warm-up for Chromium) · Option+Escape · Accessibility-permission onboarding · stub engine that plays via macOS `say` *only as a dev placeholder, removed in M2*.
Done when: select text in TextEdit and Firefox, press the key, hear it.

**M2 — Real offline speech (Phase 2)** ✅
sherpa-onnx integration · bundled Kokoro int8 + Piper Lite · `Engine` trait · profile picker · chunked sentence pipeline · cpal playback · pause/resume/stop · mini player · **measured benchmarks** committed to `docs/perf.md`.
Done when: M1 demo runs with Kokoro, offline, from a signed `.app`.

**M3 — Speech Hub + CLI (Phase 3)** ✅ ← *the differentiating milestone*
Protocol crate · socket server · `speak` CLI with say-compatible flags · launch-on-demand · `docs/public-api.md` · `examples/` (shell, clipboard-manager, Raycast).
Done when: `echo hi | speak` works from a cold app.

**M4 — Sources + Quick Speak (Phase 4)** ✅ (backend verified via `speak run`; Builder UI awaits hands-on)
Source model + persistence · Option+Shift+Escape palette · Source Builder UI · 5 types · templates · test button · import/export with trust gate · fallback-Source hook.
Done when: a non-dev builds a working `pbpaste` Source in under a minute.

**M5 — Personalization (Phase 5)** ✅
Pronunciation Studio (substitution rules, deterministic order, preview, import/export) · per-Source voice/speed · voice previews · reader polish · idle unload.

**M6 — Plus tier / cloning (Phase 6)** ✅ (download, verify, extract, engine, custom voices from WAV; microphone recording deferred)
Pocket TTS via sherpa-onnx, download-on-demand with license + consent UI · reference-audio voice creation · local voice store. Ship only if real cloning works.

**M7 — Release (Phase 7)** ⬜
Signing + notarization · DMG · GitHub Actions (build + Rust/Vitest tests; native AX tests are manual, documented) · license notices (incl. espeak-ng GPL) · clean-machine verification · README for users / power users / developers.

**Later / research:** `AVSpeechSynthesisProviderAudioUnit` (macOS 13+, works with VoiceOver and Speak Screen — needs Swift app extension) · Services menu · Shortcuts actions · Intel universal build · optional queue mode.

---

## 9. Risks

| Risk | Impact | Mitigation |
|---|---|---|
| espeak-ng GPL via sherpa-onnx | License of distributed binary | Decide in §2; notice in docs; track phonemizer alternatives |
| ~~sherpa-onnx dylib bundling~~ | — | Resolved: sherpa-onnx 2.0 and piper-phonemize link statically; the binary has no third-party dylibs. |
| AX selection unavailable in some apps | Core UX failure | Three AX dialects + on-demand tree warm-up for Chromium/Gecko (flag left on once proven needed); clear error + fallback Source hook; app matrix in `docs/architecture.md` |
| Option+Escape owned by macOS Speak Selection | Hotkey conflict | Detect at registration; onboarding tells user to disable Apple's |
| Kokoro int8 quality regression vs fp32 | Voice quality | A/B in M2; ship fp16 if int8 is audibly worse and size allows |
| Pocket TTS gating / terms | Can't ship cloning | Download-on-demand only; feature is optional by design |
| 8 GB Macs under memory pressure | Swapping, slow | Standard holds ~560 MB RSS. Lite profile, idle unload, single resident model; wizard recommends Lite under 8 GB. |
| `shortcuts run` output quirks | Shortcut Source flaky | Verify early in M4; document the "return text" recipe |

---

## 10. Repo layout (target)

```
speech-hub/
├── src/                       # React + TS (Reader, QuickSpeak, SourceBuilder, Player, Voices, Pronunciation, Preferences, Onboarding)
├── src-tauri/                 # Rust app: hub/ engines/ playback/ sources/ ipc/ hotkeys/ accessibility/ settings/
│   ├── resources/models/      # bundled Lite + Standard models (fetched by script, verified by hash)
│   └── capabilities/
├── crates/
│   ├── speechhub-protocol/
│   └── speechhub-cli/         # builds `speak`
├── scripts/                   # fetch-models.sh, verify-models.sh, bench.sh
├── examples/                  # shell/, shortcuts/, integrations/
├── docs/                      # architecture, public-api, sources, security, model-licenses, perf, release
├── .context/                  # plans / handoff notes (gitignored)
├── CLAUDE.md  PLAN.md  README.md  LICENSE  package.json
```

---

## 11. Open questions

1. `speak -o file.wav` in v1? Leaning yes, M3.
2. Kokoro int8 vs fp16: decide by ear in M2.
3. Idle-unload default (minutes): decide after measuring reload time in M2.

## 12. Sources consulted (Oct 2026)
- sherpa-onnx Rust: [sherpa-rs](https://github.com/Limit-LAB/sherpa-rs), [sherpa-onnx crate](https://docs.rs/sherpa-onnx), [pipecrab-tts-sherpa](https://docs.rs/pipecrab-tts-sherpa); pure-Rust alternatives [kokoroxide](https://docs.rs/kokoroxide), [Kokoros](https://www.sourcepulse.org/projects/2336393)
- Pocket TTS ONNX + license notes: [IgnitiveLabs/PocketTTS-ONNX](https://huggingface.co/IgnitiveLabs/PocketTTS-ONNX)
- Apple: [AVSpeechSynthesisProviderAudioUnit](https://developer.apple.com/documentation/avfaudio/avspeechsynthesisprovideraudiounit.md), [Creating a custom speech synthesizer](https://developer.apple.com/documentation/avfaudio/creating-a-custom-speech-synthesizer.md)
- Competitors: [aloud-tts](https://pypi.org/project/aloud-tts), [Sotto](https://www.hunted.space/dashboard/sotto-7), [Kokori](https://feedbagel.com/post/kokori-macos-text-to-speech-app-with-local-api-server-and-desktop-interface)
- Tauri 2 plugins: [global-shortcut 2.0.0](https://v2.tauri.app/de/release/global-shortcut/v2.0.0/), [single-instance macOS](https://v2.tauri.app/release/single-instance/v2.0.0-beta.5/)

## 13. Backlog (from hands-on testing, 2026-10-25)

Done the same day: cancellable capture, fast-fail on empty selection, tiled Preferences/Reader (activation policy + default collection behavior), ⌘, everywhere, browser pre-warm, microphone recording for custom voices, Services menu, Shortcuts/automation guide (`docs/shortcuts.md`).

| Item | Notes |
|---|---|
| Selection inside TUI apps in Ghostty (e.g. Claude Code's input box) | Ghostty's `accessibilitySelectedText` reads the live selection, so plain terminal text works. TUI apps that enable mouse reporting swallow the drag; needs a real repro with the watcher probe. Workaround: ⌥-drag to select in Ghostty, or the clipboard fallback Source. |
| Pocket TTS speed | sherpa-onnx ignores `speed` for Pocket (measured, `docs/perf.md`). Would need WSOLA time-stretch in `engines/pocket.rs`. |
| Universal (x86_64) build | Needs the rustup `x86_64-apple-darwin` target; Homebrew's rust ships only the host std. Decision: install rustup alongside, or stay arm64-only. |
| Shortcuts App Intents, `AVSpeechSynthesisProvider` | Both need a Swift target built with Xcode (App Intents extension; an Audio Unit speech-synthesis extension bundled as `.appex`). Pure-Rust Tauri can't produce them. Decision: add an Xcode subproject later, or keep the CLI/deep-link/Services route documented in `docs/shortcuts.md`. |
| Kokoro ja/zh quality | Needs listening by a speaker; phonemizer uses espeak `ja`/`cmn`. |
