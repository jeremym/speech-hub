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
| `accessibility.rs` | Own FFI to `AXUIElement*`; selected text via `AXSelectedText`, fallback `AXSelectedTextRange` + `AXStringForRange`. |
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
