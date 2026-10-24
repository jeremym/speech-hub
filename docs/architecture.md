# Architecture (living document)

See PLAN.md §3 for the overview diagram. This file tracks what is actually implemented.

## Process model
One Tauri 2 app, menu-bar only (`ActivationPolicy::Accessory`). Single instance enforced by `tauri-plugin-single-instance`. All windows are created on demand and hidden on close.

## Rust modules (`src-tauri/src`)
| Module | Role |
|---|---|
| `hub.rs` | The pipeline. Owns engine, player, status state machine, active job. Every entry point calls `Hub::speak/open/stop`. |
| `engines/` | `Engine` trait + profile catalog. `dev_say.rs` is a dev-only placeholder behind the `dev-say` feature. |
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
`text → text::segment → for each chunk: engine.synthesize (cancellable) → player.enqueue → status progress` then wait for drain → Ready. A new speak cancels the previous job (flag + queue clear + generation bump) so stale chunks are never played.
