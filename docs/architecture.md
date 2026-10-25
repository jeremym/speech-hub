# Architecture (living document)

See PLAN.md §3 for the overview diagram. This file tracks what is actually implemented.

## Process model
One Tauri 2 app, menu-bar only (`ActivationPolicy::Accessory`). Single instance enforced by `tauri-plugin-single-instance`. All windows are created on demand and hidden on close.

## Rust modules (`src-tauri/src`)
| Module | Role |
|---|---|
| `hub.rs` | The pipeline. Owns engine, player, status state machine, active job. Every entry point calls `Hub::speak/open/stop`. |
| `engines/` | `Engine` trait + profile catalog + model roots. `kokoro.rs` (Standard), `piper.rs` (Lite), `pocket.rs` (Plus + custom voice library), `phonemize.rs` (piper-phonemize/espeak-ng → IPA codepoints), `sherpa.rs` (shared OfflineTts wrapper with cancellation), `download.rs` (verified on-demand install). |
| `ipc.rs` | Unix socket server: 0600, same-uid, length-prefixed JSON, one request per connection. |
| `deeplink.rs` | `speechhub://` handler: untrusted, tiny allowlist, rate-limited speak, confirm-before-run for Sources. |
| `sources/` | Source model + store (`mod.rs`), execution with limits (`exec.rs`). |
| `pronunciation.rs` | Substitution dictionary applied to synthesis input only. |
| `playback.rs` | cpal output stream on a dedicated thread; mono f32 queue at device rate; pause = silence, stop = clear. |
| `text.rs` | Normalization + sentence segmentation (chunk size bounded for low first-audio latency). |
| `prewarm.rs` | Polls the frontmost app once a second; when a known Chromium/Gecko browser (or an app that needed the slow path this session) comes forward, sets `AXEnhancedUserInterface` so its tree is ready before the first ⌥⎋. Off via General → "Prepare browsers". |
| `services.rs` | macOS Services menu provider ("Speak with Speech Hub", "Open in Speech Hub Reader"); entries declared in `src-tauri/Info.plist`. Reads the Services pasteboard macOS hands us, never the general clipboard. |
| `accessibility.rs` | Own FFI to `AXUIElement*`. `Target::frontmost()` is resolved the instant a shortcut fires; `Target::capture()` then runs on a worker thread and tries three dialects on the focused element and its ancestors: `AXSelectedText`, `AXSelectedTextRange` + `AXStringForRange`, `AXSelectedTextMarkerRange` + `AXStringForTextMarkerRange`. See *Selection capture* below. |
| `hotkeys.rs` | Registers both shortcuts from settings; handler dispatches to hub. |
| `windows.rs` | Labels: onboarding, settings, reader, player, palette. Mini player positions top-right, never focuses. |
| `tray.rs` | Menu-bar icon (template PNG) and menu. |
| `settings.rs` | JSON in app data dir, atomic writes, sanitized. |
| `commands.rs` | Tauri commands: thin wrappers only. |

## Frontend (`src/`)
One React bundle; `App.tsx` switches on the window label. `services/hub.ts` is the only place that calls `invoke`. Preferences panes are reused by the first-launch wizard.

## Events
- `hub:status` — full `Status` snapshot on every change.
- `reader:text` — text pushed to the Reader (`open`).
- `window:shown`, `window:navigate` — UI routing.

## Request flow (speak)
`text → pronunciation rules → text::normalize (Markdown/URL cleanup) → text::segment → for each chunk: phonemize (Kokoro/Piper) → engine.synthesize (cancellable via sherpa callback) → player.enqueue → status progress` then wait for drain → Ready. A new speak cancels the previous job (flag + queue clear + generation bump) so stale chunks are never played. If the engine was idle-unloaded, the request thread reloads it first (state LoadingModel).

## Crates
- `crates/speechhub-protocol`: request/response types, validation, framing, socket path. Shared by app and CLI.
- `crates/speechhub-cli`: the `speak` binary (~500 KB, no Tauri). Bundled as a sidecar; Preferences symlinks it into `~/.local/bin`.

## Selection capture

Measured 2026-10-25 against real apps (probe binaries in `src-tauri/examples/axcap.rs`):

| App | What works | Notes |
|---|---|---|
| AppKit text views, browser URL bars | `AXSelectedText` | Instant. |
| Firefox web content | `AXSelectedTextMarkerRange` + `AXStringForTextMarkerRange` on the focused `AXWebArea` | The plain attributes return "no value". |
| Chromium (Brave 155) web content | Nothing until `AXEnhancedUserInterface` is set on the app, then `AXSelectedText` | Tree appears ~3.3 s after the flag is set on a trivial page. `AXManualAccessibility` is ignored. The set call returns `kAXErrorNotImplemented` and still takes effect. |
| Ghostty | `AXTextArea` advertises `AXSelectedText`/`AXSelectedTextRange` but `AXStringForRange` returns empty strings | Under investigation; use a clipboard fallback Source meanwhile. |

Policy: try the fast path; on failure set `AXEnhancedUserInterface` + `AXManualAccessibility` on the target app and poll for up to `WARMUP_BUDGET` (6 s). If that produced the selection the flags **stay on** (the app demonstrably needs them, and turning them off would make every later capture pay the warm-up again); if it produced nothing they are restored. Capture never blocks the main thread: the shortcut handler resolves the target app and returns, and the mini player shows "Waking up Brave's accessibility…" while the worker waits. The palette captures in the background too and refreshes its "Speak selected text" row via the `palette:selection` event.

## Main-thread rule

Every window operation goes through `windows::on_main`, which hops through a thread and then `run_on_main_thread`. Never run window creation inline on the main thread from a plugin callback (deep link `on_open_url`, global shortcut): Tauri executes `run_on_main_thread` closures immediately when already on the main thread, and creating a window re-enters the plugin-store mutex that the callback is running under. The app deadlocks silently (IPC keeps answering, no window ever appears). Found 2026-10-25 with `sample speech-hub`.

Document windows (Preferences, Reader, wizard) keep AppKit's default collection behavior. Overriding it drops the implicit `FullScreenPrimary`, which removes the green button, which makes AeroSpace's dialog heuristic float the window. While one is visible the app runs with the Regular activation policy (Dock icon, ⌘-Tab); it returns to Accessory when the last one closes.

