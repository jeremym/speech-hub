# Decisions inventory

An exhaustive list of the architectural, product, and platform decisions that are actually implemented, one bullet each, stated as fact with the reason and a file pointer. Compiled 2026-10-25 from the code, `PLAN.md`, `docs/`, the Obsidian notes (`Speech Hub/Decisions.md`, `Status.md`), and `git log`. Use it to audit what was missed; `PLAN.md` stays the plan of record.


---

## 1. Product shape and scope

- Product is **Speech Hub**: a menu-bar macOS utility that speaks selected text with a local AI voice, plus a shared speech service any app, script, Shortcut, or user-built Source can talk to; the moat is "universal voice layer", not voice quality; `PLAN.md` §0, §1.
- Names are fixed: product "Speech Hub", CLI binary `speak`, URL scheme `speechhub://`, bundle id `com.jeremym.speechhub`; `PLAN.md` §2, `src-tauri/tauri.conf.json`, `crates/speechhub-protocol/src/lib.rs` (`BUNDLE_ID`).
- Whole project is **GPL-3.0-or-later** because piper-phonemize/espeak-ng (GPLv3) is statically linked; single license, no dual notice; integrators cross a process boundary so GPL never reaches their code; `Cargo.toml` (workspace), `package.json`, `docs/model-licenses.md`.
- **Local-first, no network by default**: the only network calls are user-configured URL Sources and the one-click Plus download; no telemetry, accounts, update pings, HTTP server, or TCP port; `CLAUDE.md`, `docs/security.md`.
- **All speech goes through the Hub**: every entry point (hotkey, CLI, socket, deep link, Services, Sources, UI) calls `Hub::speak/open/stop`; no UI component synthesizes or plays audio; `src-tauri/src/hub.rs`, `src/components/Preferences/VoiceRecorder.tsx` (capture only, no preview).
- **No clipboard code in core**: clipboard support is a Command Source template running `/usr/bin/pbpaste`; no clipboard history or monitoring; `src-tauri/src/sources/mod.rs` (`templates()`), `docs/sources.md`.
- **Never log submitted text**: logs carry request ids, sizes, timings, and deep-link paths only; the deep-link log prints the path, not the query; `src-tauri/src/deeplink.rs`, `docs/security.md`.
- CPU-only by design with three engine profiles (Lite / Standard / Plus) so the app runs on Intel and 8 GB machines; CoreML/ANE offload is explicitly deferred research; `PLAN.md` §4.
- Competing on being an open, documented local speech service with user-defined Sources, not on being another Kokoro reader; `PLAN.md` §1.
- "Don't fake it": the dev-only `say` engine exists only behind the `dev-say` cargo feature and is off by default; `src-tauri/Cargo.toml`, `src-tauri/src/engines/dev_say.rs`.
- Chunked, not streaming: first audio is the time to synthesize sentence one; documented honestly as chunked; `PLAN.md` §1.8, `docs/perf.md`.
- Settings, Sources, pronunciation rules, voices, models, and the socket all live under `~/Library/Application Support/com.jeremym.speechhub/`; `src-tauri/src/lib.rs` (`app_data_dir`), `docs/public-api.md`.
- Stack: Tauri 2 (stable, not 3.0 alpha) + Rust backend, React 19 + TypeScript + Vite 8 + Tailwind 4 frontend, pnpm; CLI and protocol are separate Rust crates; `package.json`, `Cargo.toml`, `CLAUDE.md`.
- Release profile is LTO, one codegen unit, `panic = "abort"`, stripped, for a small, fast binary; `Cargo.toml`.
- Minimum macOS is 13.0 (Ventura); `src-tauri/tauri.conf.json`, `distribution/speech-hub.rb`.

## 2. Window and process model

- One long-running Tauri app owns the model, socket, hotkeys, and all windows; the CLI is a thin client that never loads a model; `PLAN.md` §3, `crates/speechhub-cli/src/main.rs`.
- **Single instance** via `tauri-plugin-single-instance`, registered first; a second launch shows the Settings window and exits; `src-tauri/src/lib.rs`.
- **Activation policy is Accessory at startup** (menu-bar only, no Dock icon); `src-tauri/src/lib.rs` (`set_activation_policy`).
- **Policy switches to Regular while any document window (Settings, Reader, Onboarding) is visible, back to Accessory when the last one hides**, so tiling window managers (AeroSpace) treat them as real windows and they appear in Cmd-Tab with a Dock icon; `src-tauri/src/windows.rs` (`sync_activation_policy`, `DOCUMENT_WINDOWS`).
- The Regular policy is set *before* `show()` so the window manager sees a regular app's window from the start; `src-tauri/src/windows.rs` (`show`).
- Exactly five windows exist, all created on demand from one `index.html` and routed by label: `onboarding`, `settings`, `reader`, `player`, `palette`; `src-tauri/src/windows.rs` (`ALL`), `src/App.tsx`.
- `tauri.conf.json` declares no windows (`"windows": []`); everything is built lazily by `windows::ensure`; `src-tauri/tauri.conf.json`.
- **Closing any window hides it instead of destroying it** so reopening is instant and React state survives; the app lives in the menu bar; `src-tauri/src/lib.rs` (`on_window_event`), `src-tauri/src/windows.rs`.
- **Document windows keep AppKit's default collection behavior**: overriding it drops the implicit `FullScreenPrimary`, removes the green button, and makes AeroSpace float them as dialogs; setting `FullScreenPrimary` explicitly deadlocked tao; `src-tauri/src/windows.rs` (`ensure`), `docs/architecture.md`.
- **Player and palette are HUD-style floaters**: `CanJoinAllSpaces | FullScreenAuxiliary | Stationary | IgnoresCycle` at `NSFloatingWindowLevel`, so they appear on whichever Space is active and survive native full-screen apps; `src-tauri/src/macos.rs` (`float_on_all_spaces`).
- Player and palette are undecorated, always-on-top, skip the taskbar, and the player is `visible_on_all_workspaces`; `src-tauri/src/windows.rs`.
- **Mini player never takes focus**: shown with `orderFrontRegardless`, built with `focused(false)` and `accept_first_mouse(true)`; positioned top-right of the primary monitor under the menu bar; `src-tauri/src/windows.rs` (`show_player`, `position_player`), `src-tauri/src/macos.rs`.
- Mini player is shown only when `show_mini_player` is on **and the Reader is not visible**; it auto-hides 1.8 s after a job finishes, 0.6 s after stop, 0.1 s after a notice clears; a hide token cancels stale hides; `src-tauri/src/hub.rs` (`speak`, `finish`, `stop`), `src-tauri/src/windows.rs` (`schedule_hide_player`).
- Mini player grows from 76 px to 128 px to show a notice or error in full; dismiss stops playback if active, otherwise hides; `src/components/Player/MiniPlayer.tsx`.
- **Palette hands keyboard focus back** to the previous app on hide via `NSApplication.deactivate`; `src-tauri/src/macos.rs` (`deactivate_app`), `src-tauri/src/windows.rs` (`hide`).
- **Main-thread rule**: every window operation goes through `windows::on_main`, which hops through a spawned thread before `run_on_main_thread`; creating a window inline from a plugin callback (deep link, global shortcut) re-enters Tauri's plugin-store mutex and deadlocks silently; `src-tauri/src/windows.rs` (`on_main`), `docs/architecture.md`.
- Window sizes: onboarding 760×580 fixed, settings 860×620 (min 700×480), reader 640×480 (min 420×300), palette 560×420 fixed, player 340×76; `src-tauri/src/windows.rs`.
- Preferences window renders at 1.15 zoom for larger type; other windows stay compact; `src/App.tsx`.
- **⌘, opens Preferences from any Speech Hub window**, like every Mac app; `src/App.tsx`.
- Menu-bar tray uses a template PNG icon, left-click shows the menu, items: Quick Speak, Open Reader, Speak Selected Text, Stop Speaking, Sources, Voices, Pronunciation Studio, Preferences (⌘,), Quit (⌘Q); `src-tauri/src/tray.rs`.
- Quit is only reachable from the tray menu (and ⌘Q while a document window is key); quitting stops playback first; `src-tauri/src/tray.rs`.
- **Launch at login exists** as a user setting (default off) via `tauri-plugin-autostart` with the LaunchAgent launcher; `src-tauri/src/lib.rs`, `src-tauri/src/commands.rs` (`update_settings`), `src-tauri/src/settings.rs`.
- First launch shows the onboarding wizard whenever `onboarding_complete` is false; `src-tauri/src/lib.rs`.
- Navigation into a Preferences pane (from tray, palette, Reader) goes through a `window:navigate` event emitted 250 ms after `show` so React has mounted its listener; `src-tauri/src/windows.rs` (`navigate_with`), `src/components/Preferences/Preferences.tsx`.
- Frontend is one React bundle; `services/hub.ts` is the only module that calls `invoke`; Tauri commands are thin wrappers with no logic; `src/services/hub.ts`, `src-tauri/src/commands.rs`.
- Tauri capability grants the five windows only `core:default`, window show/hide/close/focus/drag/set-size/is-visible, `opener:allow-open-url`, and `dialog:allow-open`; `src-tauri/capabilities/default.json`.
- CSP is `default-src 'self'` with inline styles, data images, and the Tauri IPC origin only; `src-tauri/tauri.conf.json`.
- Logging uses `env_logger` with `info` default, controllable via `RUST_LOG`; `src-tauri/src/lib.rs`.

- Visibility of our own windows is tracked in a `HashSet` kept by `show`/`hide`/`show_player`/close; worker threads read `windows::is_shown` and never call Tauri window getters, which are dispatched to the event loop and deadlock when the main thread is blocked on that worker; `src-tauri/src/windows.rs`.
- Commands that can take more than a few ms (`sources_run`, `sources_test`, `cli_status`, `shortcuts_list`, `system_info`) are `async` and run in `spawn_blocking`, so the main thread (tray, hotkeys, window ops) never waits on them; `src-tauri/src/commands.rs`.
- ⌘Q in Preferences/Reader closes the windows and leaves the service running (custom app menu with "Close Windows (keep running)" and "Quit Speech Hub Entirely"); only the tray and that second item exit; `src-tauri/src/menu.rs`.
- The IPC socket file is removed on `RunEvent::Exit`; `src-tauri/src/ipc.rs`, `src-tauri/src/lib.rs`.

## 3. User-facing settings (settings.rs and the Preferences panes)

Stored as `settings.json` in the app data dir, written atomically via a `.tmp` rename, loaded with defaults for any missing field, sanitized on save; `src-tauri/src/settings.rs`.

| Setting | Default | What it does | Where |
|---|---|---|---|
| `onboardingComplete` | `false` | Wizard shows on launch until set by "Start using Speech Hub" or "Skip setup" | `settings.rs`, `Wizard.tsx` |
| `hotkeys.speakSelection` | `Alt+Escape` (⌥⎋) | Global shortcut: capture selection and speak | Shortcuts pane |
| `hotkeys.quickSpeak` | `Alt+Shift+Escape` (⌥⇧⎋) | Global shortcut: palette / Reader / a Source | Shortcuts pane |
| `engineProfile` | `standard` | `lite`, `standard`, `plus`, or `dev-say`; switching loads the new engine | Engine pane |
| `voiceId` | `""` (engine default) | Default voice for every request without an explicit voice | Voices pane, Reader |
| `speed` | `1.0` | Default speed multiplier, clamped 0.5 to 3.0 on save; UI slider goes 0.5 to 2.5 | Voices pane, Reader |
| `launchAtLogin` | `false` | Registers / removes a Login Item | General pane |
| `showMiniPlayer` | `true` | Floating controller while speaking when the Reader isn't open | General pane |
| `repeatShortcut` | `replace` | ⌥⎋ while speaking: `replace` reads the new selection, `stop` stops | Shortcuts pane |
| `quickSpeakAction` | `palette` | ⌥⇧⎋ opens the `palette`, a blank `reader`, or runs a `source` | Shortcuts pane |
| `quickSpeakSourceId` | `null` | Which Source ⌥⇧⎋ runs when action is `source`; falls back to the palette on failure | Shortcuts pane |
| `fallbackSourceId` | `null` | Source to run when selection capture fails; `null` = just show why | Accessibility pane |
| `idleUnloadMinutes` | `0` (never) | Unload the model after N idle minutes to free RAM; checked every 30 s; "Unload now" button | General pane |
| `prewarmBrowsers` | `true` | Set `AXEnhancedUserInterface` on known browsers the moment they come to the front | General pane |

- Empty hotkey strings are replaced with the defaults on save; `src-tauri/src/settings.rs` (`sanitize`).
- Changing hotkeys re-registers them; changing the engine profile reloads the engine; changing voice/speed updates hub status; changing launch-at-login toggles the autostart plugin; `src-tauri/src/commands.rs` (`update_settings`).
- Settings UI uses optimistic patching: the pane updates immediately, then replaces its state with the saved snapshot; `src/services/hub.ts` (`useSettings`).
- Preferences panes, in order: General, Accessibility, Engine, Voices, Shortcuts, Sources, Pronunciation, Command Line, About; `src/components/Preferences/Preferences.tsx`.
- The onboarding wizard reuses the Preferences panes: Welcome → Permission (Accessibility) → Engine → Voice → Shortcuts → Command line → Done; every step is skippable from step 2 on; `src/components/Onboarding/Wizard.tsx`.
- The wizard pre-selects the recommended engine once: Lite on non-Apple-Silicon or under 8 GB RAM, otherwise Standard; always overridable; `src/components/Onboarding/Wizard.tsx`, `src-tauri/src/commands.rs` (`recommended_profile`).
- Engine picker is Handy-style cards: name, tagline, size, RAM, quality/speed dots, languages, license, Bundled/Installed/Recommended/Active/Voice cloning badges; `src-tauri/src/engines/mod.rs` (`catalog`), `src/components/Preferences/panes.tsx` (`EnginePane`).
- Shortcut recorder captures a keydown and refuses modifier-less keys except F-keys and Escape, so bare letters can't hijack typing everywhere; `src/lib/shortcut.ts` (`eventToShortcut`).
- Shortcuts pane shows per-shortcut registration errors (taken or invalid) and links to Apple's Spoken Content settings for the ⌥⎋ conflict; `src/components/Preferences/panes.tsx` (`HotkeysPane`).
- A `system_speech` command reads `com.apple.universalaccess` defaults (speak selection, hover speech, typing feedback) and the Shortcuts pane shows On/Off badges and warns when Apple's ⌥⎋ conflicts; read-only, nothing is written; `src-tauri/src/commands.rs`, `src/components/Preferences/panes.tsx`.
- About pane shows version, macOS version, and the settings folder path; license text says GPL-3.0; `src/components/Preferences/panes.tsx` (`AboutPane`).
- Open System Settings is allowlisted to three panes (accessibility, spoken_content, login_items); no arbitrary URL leaves the app; `src-tauri/src/commands.rs` (`open_system_settings`).

## 4. Global shortcuts and input paths

- Two global shortcuts, registered in Rust (frontend needs no shortcut permission) via `tauri-plugin-global-shortcut`; both configurable; registration never panics and reports per-shortcut status; `src-tauri/src/hotkeys.rs`.
- ⌥⎋ = speak selection; ⌥⇧⎋ = Quick Speak (palette by default); `src-tauri/src/settings.rs` (`Hotkeys::default`).
- The ⌥⎋ handler returns immediately after resolving the target app; capture and speech run on a worker thread so a slow browser never freezes the UI; `src-tauri/src/hub.rs` (`speak_selection`).
- Shortcut strings use the plugin grammar (`Alt+Shift+Escape`) and are displayed as Mac symbols (⌥⇧⎋); `src/lib/shortcut.ts`.
- **macOS Services menu**: "Speak with Speech Hub" and "Open in Speech Hub Reader" for `public.utf8-plain-text`, declared in `Info.plist`, served by an `NSServicesProvider` registered at setup; reads the Services pasteboard only, never the general clipboard; works in apps whose AX selection can't be read; `src-tauri/Info.plist`, `src-tauri/src/services.rs`, `docs/shortcuts.md`.
- **CLI `speak`**: ~500 KB static Rust binary, no Tauri dependency, bundled as a Tauri sidecar (`binaries/speak-<triple>`) and built by `scripts/build-cli.sh` before every dev/build; `crates/speechhub-cli`, `src-tauri/tauri.conf.json` (`externalBin`), `scripts/build-cli.sh`.
- CLI is `say`-compatible: `-v`, `-r <wpm>` (175 = 1.0×), `-f`, `-o`, positional text; also `--speed`, `--stdin`, `-t/--title`, `--open`, `--json`, `--no-launch`, `--timeout`; subcommands `open run palette stop pause resume status voices sources version`; `crates/speechhub-cli/src/main.rs`.
- CLI reads stdin automatically when no text is given and stdin is not a terminal; `crates/speechhub-cli/src/main.rs` (`read_text`).
- CLI exit codes: 0 ok, 1 request rejected, 2 app unavailable, 3 engine not ready or permission missing, 64 usage; `crates/speechhub-cli/src/main.rs`, `docs/public-api.md`.
- CLI launches the app if it isn't running (`open -g -b com.jeremym.speechhub`, no focus steal) and polls the socket up to 10 s; `status`, `stop`, `pause`, `resume`, `version` never launch it; `speak status` on a stopped app prints "not running" with exit 0; `crates/speechhub-cli/src/main.rs` (`connect`).
- CLI honors `SPEECH_HUB_SOCKET` to override the socket path; `crates/speechhub-cli/src/main.rs`.
- `-o file.wav` renders without playback via the `render` action; path is made absolute by the CLI and must end in `.wav`; `crates/speechhub-cli/src/main.rs`, `crates/speechhub-protocol/src/lib.rs` (`validate`).
- CLI install is a symlink `~/.local/bin/speak` → the binary inside the app bundle, so updates stay in sync and no sudo is needed; Preferences shows a PATH hint if `~/.local/bin` is missing from the login shell's PATH; `src-tauri/src/commands.rs` (`cli_install`, `cli_status`).
- **Unix socket protocol v1**: `run/hub.sock` in the app data dir, 4-byte big-endian length + JSON, one request per connection, every request carries `version` and `request_id`; `crates/speechhub-protocol/src/lib.rs`, `src-tauri/src/ipc.rs`.
- Socket actions: `speak`, `open`, `render`, `run_source`, `stop`, `pause`, `resume`, `status`, `list_voices`, `list_sources`, `palette`, `version`; **there is deliberately no verb that executes a command or reads an arbitrary path**; `crates/speechhub-protocol/src/lib.rs` (`Request`).
- Each connection runs on its own thread with 15 s read/write timeouts; `src-tauri/src/ipc.rs`.
- **Deep links `speechhub://`**: `open`/`reader`/empty, `palette`/`quick-speak`, `preferences`/`settings`, `stop`, `speak?text=` (≤ 2 KB, max one per second), `source/<id-or-name>` (confirmation in the palette); unknown paths are logged and ignored; `src-tauri/src/deeplink.rs`.
- **Sources** are the fifth input path: Text, File, Command, Shortcut, HTTP, each with optional voice/speed and a speak-or-open action; run from the palette, tray, ⌥⇧⎋, `speak run`, or a confirmed link; `src-tauri/src/sources/mod.rs`.
- **Fallback Source**: when selection capture fails, the configured Source runs; if none is configured or it fails, a notice explains why; `src-tauri/src/hub.rs` (`selection_failed`), `src-tauri/src/sources/mod.rs` (`run_fallback`).
- Shortcuts app integration is CLI/deep-link/Services only; no App Intents (needs a Swift target); documented recipes in `docs/shortcuts.md`.
- Quick Speak palette lists actions (speak selection, open Reader, new Source), the user's Sources, and typed text; Enter speaks typed text, Shift+Enter opens it in the Reader; Sources sort first when searching; `src/components/QuickSpeak/Palette.tsx`.
- Public Source listing (palette, `speak sources list`) shows names, kinds, and a sanitized summary only: program and file *names*, never arguments, full paths, or URLs; `src-tauri/src/sources/mod.rs` (`PublicSource`, `summary`).
- Sources resolve by id, exact name (case-insensitive), or slug (`morning-briefing` == "Morning Briefing"); `src-tauri/src/sources/mod.rs` (`resolve`).

## 5. Selection capture (Accessibility)

- Own minimal FFI to `AXUIElement*` instead of a binding crate: a handful of functions, no version drift; never activates a window or touches the clipboard; `src-tauri/src/accessibility.rs`.
- **Three AX dialects are tried on the focused element and up to six ancestors (stopping at `AXWindow`)**: `AXSelectedText`; `AXSelectedTextRange` + `AXStringForRange`; `AXSelectedTextMarkerRange` + `AXStringForTextMarkerRange` (Firefox/WebKit); `src-tauri/src/accessibility.rs` (`capture_once`, `selection_of`).
- Walking up ancestors matters because a focused link or button inside a selected paragraph doesn't carry the selection, but the web area above it does; `src-tauri/src/accessibility.rs`.
- **Fast-fail on empty selection**: an element that speaks the text protocol and answers "empty" yields `NoSelection` immediately; only an app that exposes no text at all is worth waking up; `src-tauri/src/accessibility.rs` (`Probe::Empty`).
- **Chromium warm-up**: on the slow path, set `AXEnhancedUserInterface` and `AXManualAccessibility` on the target app and poll every 100 ms for up to 6 s (`WARMUP_BUDGET`); Brave 155 measured 3.3 s on a trivial page; `src-tauri/src/accessibility.rs` (`Target::capture`).
- **Flags stay on if the warm-up produced the selection** (the app demonstrably needs them, and turning them off would make every later capture pay the warm-up again); they are restored to their prior values if it produced nothing; `src-tauri/src/accessibility.rs`.
- Apps that needed the slow path are remembered for the session as "lazy apps" and pre-warmed on later activations; `src-tauri/src/accessibility.rs` (`remember_lazy_app`), `src-tauri/src/prewarm.rs`.
- **Browser pre-warm thread** polls the frontmost pid once a second; when a known Chromium/Gecko browser (Brave, Chrome, Chromium, Edge, Arc, Vivaldi, Opera, Firefox, Zen, and betas) or a remembered lazy app comes forward, it sets the flag so the first ⌥⎋ is instant; off via the `prewarmBrowsers` setting; skipped when Accessibility isn't granted; nothing is read from those apps; `src-tauri/src/prewarm.rs`.
- **Target is resolved the instant the shortcut fires** (system-wide `AXFocusedApplication`, falling back to `NSWorkspace.frontmostApplication`), before any Speech Hub window becomes key; the capture itself runs later on a worker thread because `AXFocusedUIElement` is per-app; `src-tauri/src/accessibility.rs` (`Target::frontmost`).
- **Palette snapshot**: opening the palette captures the selection in the background and shows "Speak selected text from <app>" with a 12-word preview once `palette:selection` fires; the snapshot is valid for 300 s and consumed by the "Speak selected text" row; `src-tauri/src/hub.rs` (`open_palette`, `selection_preview`, `take_fresh_snapshot`), `src/components/QuickSpeak/Palette.tsx`.
- **Capture is cancellable**: the mini player's stop button sets a cancel flag that aborts a warm-up wait; `src-tauri/src/hub.rs` (`capture_cancel`, `stop`), `src-tauri/src/accessibility.rs`.
- While waiting, the mini player shows "Waking up <App>'s accessibility…" via the `LoadingSource` state; `src-tauri/src/hub.rs` (`speak_selection`, `set_loading_source`).
- The set call for `AXEnhancedUserInterface` returns `kAXErrorNotImplemented` on Chromium yet still takes effect, so return codes from `set_bool_attr` are deliberately not trusted; `src-tauri/src/accessibility.rs`.
- Repeat behavior: ⌥⎋ while busy either replaces the reading (default) or stops it, per `repeatShortcut`; `src-tauri/src/hub.rs` (`speak_selection`).
- **Permission flow**: `AXIsProcessTrustedWithOptions` checked without prompting on every capture; the Accessibility pane polls trust every 1.5 s, offers "Grant access…" (system prompt) and a deep link to System Settings → Privacy & Security → Accessibility; `src-tauri/src/accessibility.rs` (`is_trusted`, `SETTINGS_URL`), `src/components/Preferences/panes.tsx` (`AccessibilityPane`).
- **Without permission**: capture fails with `NotTrusted` → error code `permission_required` (CLI exit 3); the fallback Source runs if configured, else the mini player shows the reason; the wizard lets the user continue without granting; `src-tauri/src/hub.rs` (`HubError::code`), `src/components/Onboarding/Wizard.tsx`.
- Accessibility is never required for the CLI, socket, Sources, or Services paths; `docs/security.md`.
- Measured app matrix (TextEdit/AppKit, Firefox, Brave, Ghostty) and the probe binary are kept for re-testing; `docs/architecture.md`, `src-tauri/examples/axcap.rs`.
- Error messages are user-facing sentences ("Nothing is selected", "This app doesn't expose its selected text to Accessibility"); `src-tauri/src/accessibility.rs` (`AxError`).

## 6. Speech engines and audio

- One `Engine` trait (`info`, `voices`, `default_voice`, `synthesize(chunk, voice, speed, cancel)`); the hub only ever talks to `dyn Engine`; `src-tauri/src/engines/mod.rs`.
- **Lite = three Piper VITS medium int8 voices (Amy, Ryan, Lessac), 53 MB, bundled**; chosen over the 904-speaker libritts model because anonymous numbered speakers make a bad picker; each voice is its own model, loaded on first use and kept, default voice loaded eagerly; `src-tauri/src/engines/piper.rs`, `scripts/fetch-models.sh`.
- **Standard = Kokoro-82M v1.0 fp32, 54 voices, 9 languages, 337 MB, bundled, default on Apple Silicon**; fp32 chosen over int8 because int8 measured ~3× slower on Apple Silicon CPUs under ONNX Runtime; `src-tauri/src/engines/kokoro.rs`, `docs/perf.md`.
- **Plus = Pocket TTS int8 (Kyutai), 98 MB download / 194 MB installed, voice cloning from a reference clip, never bundled**; CC-BY-4.0 with a consent clause and gated upstream; `src-tauri/src/engines/pocket.rs`, `src-tauri/src/engines/download.rs`.
- All three run through statically linked sherpa-onnx 2.0 with the CPU provider; no third-party dylibs to codesign; `src-tauri/src/engines/sherpa.rs`, `docs/model-licenses.md`.
- **Phonemization is in-process via piper-phonemize (espeak-ng)**, passing `phoneme_codepoints` to sherpa, because sherpa-onnx 2.0 removed espeak-ng and its lexicon-only fallback silently skips out-of-vocabulary words; espeak-ng is not thread-safe so calls are serialized; warmed up on engine load (~400 ms first call); `src-tauri/src/engines/phonemize.rs`.
- espeak voice per Kokoro language prefix: a→en-us, b→en (British RP), e→es, f→fr, h→hi, i→it, j→ja, p→pt-br, z→cmn; unavailable voices fall back to en-us; `src-tauri/src/engines/phonemize.rs`.
- ONNX Runtime threads = half the cores clamped to 2..=8 (Piper capped at 4); `src-tauri/src/engines/sherpa.rs` (`num_threads`).
- sherpa is configured with `max_num_sentences: 1` and `silence_scale: 0.2`; `generate()` calls are serialized behind a mutex so a cancelled job can't fight the next one for CPU; `src-tauri/src/engines/sherpa.rs`.
- Cancellation is cooperative: the sherpa progress callback returns `false` when the job's cancel flag is set; `src-tauri/src/engines/sherpa.rs`.
- **Model roots in priority order**: `SPEECH_HUB_MODELS_DIR` (benchmarks/tests), the app bundle's `resources/models`, then `<app data>/models` for downloads; `src-tauri/src/engines/mod.rs` (`ModelRoots`).
- Only Lite and Standard are bundled as Tauri resources; Plus is fetched at runtime; `src-tauri/tauri.conf.json` (`bundle.resources`).
- Models are never committed; `scripts/fetch-models.sh` downloads from sherpa-onnx GitHub releases, verifies SHA-256, caches in `.cache/models`, and copies only runtime files; `scripts/fetch-models.sh`, `.gitignore`.
- **Plus download**: fixed GitHub URL, SHA-256 pinned in code, fetched with system `curl` (`--proto =https`, retries), verified before extraction, extracted to a staging dir, only needed files kept, sample WAVs become sample voices, then atomically swapped in; one download at a time; progress via `models:download` events; `src-tauri/src/engines/download.rs`.
- **Download consent**: the Engine pane shows the CC BY 4.0 license and the consent clause with a checkbox that must be ticked before "Download Plus (98 MB)" enables; `src/components/Preferences/panes.tsx` (`EnginePane`).
- Removing Plus while it is the active profile unloads it, switches to the recommended profile, and clears the voice; `src-tauri/src/commands.rs` (`plus_remove`).
- Engine create falls back to the best installed bundled profile (standard, then lite) and reports the id actually used, so the UI never lies about what's speaking; `src-tauri/src/engines/mod.rs` (`create`).
- Only one model is resident at a time; switching profiles unloads the other; `src-tauri/src/hub.rs` (`load_engine`).
- **Idle unload**: a watcher thread checks every 30 s and unloads after `idleUnloadMinutes` of inactivity (0 = never); the next request reloads synchronously on the request thread, showing `LoadingModel`; `src-tauri/src/hub.rs` (`start_idle_watch`, `engine_or_load`).
- **Voice library (Plus custom voices)**: `<app data>/voices/{voices.json, *.wav}`; recordings are validated, re-encoded as mono 16-bit WAV, trimmed to 30 s, require ≥ 2 s of audio, a 1–60 char name, and an explicit consent flag; `src-tauri/src/engines/pocket.rs` (`VoiceLibrary`).
- Custom voices can be added from a WAV file or **recorded in-app with the microphone** (Web Audio in the Preferences webview, up to 30 s, encoded to WAV in the window, sent as a raw IPC body with name/consent in headers); capture only, no preview, because the Hub owns playback; `src/components/Preferences/VoiceRecorder.tsx`, `src-tauri/src/commands.rs` (`voices_custom_add_recording`).
- Microphone access requires the hardened-runtime `com.apple.security.device.audio-input` entitlement and an `NSMicrophoneUsageDescription`; requested only when "Record a voice" is clicked; `src-tauri/Entitlements.plist`, `src-tauri/Info.plist`.
- Plus reference-audio embeddings are cached per voice (sherpa cache capacity 8, plus a decoded-audio cache in the engine); `src-tauri/src/engines/pocket.rs`.
- Plus default voice is the first custom voice, else the first sample voice; an unknown voice id falls back to the first sample; `src-tauri/src/engines/pocket.rs`.
- **Pocket TTS ignores `speed`** (measured with `examples/speedtest.rs`); the Voices pane says so when Plus is active; `docs/perf.md`, `src/components/Preferences/panes.tsx`.
- Speed is clamped to 0.5–3.0 everywhere (settings, protocol validation, engines); `src-tauri/src/settings.rs`, `crates/speechhub-protocol/src/lib.rs`, `src-tauri/src/engines/sherpa.rs`.
- **Text pipeline**: pronunciation rules → `normalize` (Markdown/URL cleanup, whitespace) → sentence segmentation into chunks of at most 320 chars, merging fragments under 24 chars forward, splitting long sentences at clause punctuation; paragraphs are boundaries; common abbreviations don't split; `src-tauri/src/text.rs`, `src-tauri/src/hub.rs` (`prepare`).
- Markdown is read as prose: headings become sentences, bullets/quotes/table pipes are stripped, links read their text, raw URLs read their host, code fences keep their content, `2 * 3` survives; `src-tauri/src/text.rs` (`strip_markup`).
- **Pronunciation Studio**: one shared substitution dictionary (`pronunciation.json`) applied to synthesis input only, longest match first then oldest first, whole-word and case-insensitive by default, import merges by match text; the Reader text never changes; `src-tauri/src/pronunciation.rs`.
- Reader has a "Fix pronunciation…" action that pre-fills a rule from the selected word with surrounding context; `src/components/Reader/Reader.tsx`, `src/components/PronunciationStudio/PronunciationStudio.tsx`.
- **Playback stack**: cpal, one output stream for the app's lifetime on a dedicated thread, mono f32 queue resampled (linear interpolation) to the device rate and duplicated across channels; `src-tauri/src/playback.rs`.
- Pause = the callback outputs silence; stop = clear the queue, so both are instant; a generation counter lets producers notice their chunks were discarded; `src-tauri/src/playback.rs`.
- **Playback policy v1**: a new `speak` replaces whatever is playing; `stop` cancels inference and audio; obsolete chunks are dropped by cancel flag + generation bump so stale audio is never played late; queueing is deferred as an opt-in; `src-tauri/src/hub.rs` (`cancel_current`, `run_job`), `docs/public-api.md`.
- Synthesis is sentence-pipelined: chunk N+1 is synthesized while N plays; progress (`synthesized/total`) is in status; `src-tauri/src/hub.rs` (`run_job`).
- State machine lives in Rust and is mirrored to the UI through `hub:status` snapshots: `Idle → LoadingModel → Ready → LoadingSource → Synthesizing ⇄ Playing ⇄ Paused → Ready | Error`; `crates/speechhub-protocol/src/lib.rs` (`HubState`), `src-tauri/src/hub.rs`.
- Errors auto-clear to Ready after ~2.5–2.8 s; notices are transient messages in the mini player; `src-tauri/src/hub.rs` (`finish`, `notice`).
- `render` synthesizes to 16-bit mono WAV without playback, blocking the IPC thread; `src-tauri/src/hub.rs` (`render`).
- Voice preview speaks a fixed sentence with the chosen voice through the normal pipeline; `src-tauri/src/hub.rs` (`preview_voice`).
- Kokoro voice ids encode language and gender in their prefix and are sorted English-first; the default voice is `af_heart`; `src-tauri/src/engines/kokoro.rs`.
- Perf numbers are measured, never invented; a bench example and `docs/perf.md` hold them; `src-tauri/examples/bench.rs`, `docs/perf.md`, `CLAUDE.md`.

- Speech requests wait up to 60 s for an engine that is loading (launch, profile switch, idle reload) instead of failing with "still loading"; the wait happens inside the speak worker, never on the caller's thread; `src-tauri/src/hub.rs` (`engine_or_load`, `speak`).
- Audio output follows the system default device: the audio thread polls the default every 500 ms and rebuilds the cpal stream when the device changes or the stream errors, resampling any queued samples; a queue nobody consumes for 3 s ends the job with "No audio output device"; no device at launch is not fatal; `src-tauri/src/playback.rs`.
- The palette's selection snapshot is dropped when the palette closes unless a speak request already claimed it, so the next ⌥⎋ reads the live selection; `src-tauri/src/hub.rs` (`palette_closed`), `src-tauri/src/windows.rs`.
- AX flags (`AXEnhancedUserInterface`, `AXManualAccessibility`) are restored only when they were read as `false`; a flag the app would not report is left alone so VoiceOver's or Voice Control's setting is never cleared; `src-tauri/src/accessibility.rs`.
- The AX messaging timeout for a target app is 1 s and the app name comes from `NSRunningApplication` (no IPC), so a hung target costs the hotkey handler at most a second; `src-tauri/src/accessibility.rs`.

## 7. Security and privacy

- Hardened runtime is on; the only entitlement is microphone (`com.apple.security.device.audio-input`); audio output, Accessibility, and the socket need none; `src-tauri/tauri.conf.json`, `src-tauri/Entitlements.plist`.
- **Self-signed release identity "Speech Hub Release" (20-year cert), no Apple Developer ID, no notarization**; TCC keys the Accessibility grant to `identifier "com.jeremym.speechhub" and certificate leaf = H"b6025fb…"`, so a stable cert means users grant once; ad-hoc/cdhash signing would break the grant every build; the cert must never be rotated; `scripts/create-release-cert.sh`, `docs/release.md`.
- Build scripts refuse a cdhash-pinned designated requirement and verify with `codesign --verify --strict --deep`; `scripts/build-release.sh`, `scripts/install-local.sh`.
- Socket: run dir 0700, socket 0600, `getpeereid` must match our uid, symlinks at the socket path are removed (never followed), a regular file there is a hard error, frames capped at 1 MiB + 64 KiB, schema and version validated before dispatch; `src-tauri/src/ipc.rs`, `crates/speechhub-protocol/src/lib.rs`.
- Text caps: 1 MiB per request (socket/CLI), 2 KB per deep link, 1 MB per Source output/file, 200-char titles, 128-char request and source ids; `crates/speechhub-protocol/src/lib.rs`, `src-tauri/src/deeplink.rs`, `src-tauri/src/sources/exec.rs`.
- **Deep links are untrusted**: tiny allowlist, no paths, no commands, no fetches; `speak?text=` is rate-limited to one per second so a page can't machine-gun the speaker; `source/<id>` always asks in the palette and refuses untrusted executable Sources; `src-tauri/src/deeplink.rs`, `src-tauri/src/sources/mod.rs` (`confirm_and_run_from_link`).
- **`run_source` is the only privileged verb** and references a Source by id/name the user already created; executable kinds (Command, Shortcut) run only when `trusted`; `src-tauri/src/sources/mod.rs` (`fetch`).
- **Trust model**: Sources saved in the Builder are trusted (the user typed the command); imported Sources get regenerated ids and `trusted=false` for executable kinds until the user opens them and clicks "Approve & Save"; `src-tauri/src/sources/mod.rs` (`import_json`), `src-tauri/src/commands.rs` (`sources_save`), `src/components/SourceBuilder/SourceBuilder.tsx`.
- Command Sources take an argv array with no shell by default (`$HOME` stays literal); shell mode (`/bin/zsh -c`) is an explicit advanced opt-in; `src-tauri/src/sources/exec.rs` (`run_command`).
- **Child environment is scrubbed** to a fixed PATH plus HOME, USER, LOGNAME, TMPDIR, LANG, LC_ALL, SHELL, and `SPEECH_HUB=1`; API keys never pass through; `src-tauri/src/sources/exec.rs` (`scrubbed_env`).
- Per-Source timeout 1–120 s (default 15), enforced by polling and `kill`; stdout read on a helper thread so a chatty command can't deadlock; stderr capped at 16 KiB for error messages; `src-tauri/src/sources/exec.rs`.
- File Sources require an absolute path (with `~/` expansion), must be a regular file, and are truncated at 1 MB with a spoken marker; `src-tauri/src/sources/exec.rs` (`read_file`).
- **HTTP via system `curl`, not a Rust TLS stack**: http/https only, `--max-filesize` 1 MB, timeout, custom User-Agent, Accept header preferring text/JSON; honors proxies and keychain roots; no rustls/openssl to bundle; `src-tauri/src/sources/exec.rs` (`fetch_http`).
- HTTP responses are treated as text: JSON path extraction, JSON string values joined if no path, HTML reduced to text (script/style dropped, entities decoded), never rendered as HTML; `src-tauri/src/sources/exec.rs`.
- Shortcut Sources run `/usr/bin/shortcuts run <name> --output-path … --output-type public.plain-text`, with optional input via a temp file; `src-tauri/src/sources/exec.rs` (`run_shortcut`).
- Import files are capped at 2 MB (Sources and pronunciation rules); `src-tauri/src/commands.rs`.
- Sources export contains the user's own full definitions (paths, commands) and is written only to a path chosen in a save dialog; `src-tauri/src/commands.rs` (`sources_export`).
- Reference recordings are decoded with `hound`, re-encoded, trimmed, and require the consent flag; custom voices never leave the disk; `src-tauri/src/engines/pocket.rs`, `docs/model-licenses.md`.
- Services provider reads only the pasteboard macOS hands it inside the service call; nothing is written back; `src-tauri/src/services.rs`.
- All stores (settings, sources, rules, voices index) write atomically via temp file + rename; `src-tauri/src/settings.rs`, `src-tauri/src/sources/mod.rs`, `src-tauri/src/pronunciation.rs`, `src-tauri/src/engines/pocket.rs`.
- Logging policy: text is never logged; the hub logs ids, counts, timings, and error strings; `CLAUDE.md`, `docs/security.md`.
- Security reporting is a GitHub issue tagged `security`, best-effort response; `docs/security.md`.

- `strip_html` lowercases ASCII only, so byte offsets stay valid for pages containing `İ` or the Kelvin sign (Unicode lowercasing changed lengths and aborted the process); percent-decoding slices bytes, not `&str`; `src-tauri/src/sources/exec.rs`, `src-tauri/src/deeplink.rs`.
- curl calls pin redirect protocols (`--proto-redir`) as well as the initial one; `src-tauri/src/sources/exec.rs`, `src-tauri/src/engines/download.rs`.
- Deep links that show a window are limited to one per 2 s, speak links to one per second; `src-tauri/src/deeplink.rs`.
- A bare key (including Escape) cannot be bound as a global shortcut; F-keys are the only modifier-less option; `src/lib/shortcut.ts`.
- The webview capability grants `dialog:allow-open` and `dialog:allow-save` only (opener removed: no caller); `src-tauri/capabilities/default.json`.
- Apple Spoken Content switches are read (never written) from `com.apple.universalaccess` and shown in the Shortcuts pane with a one-click rebind to ⌃⌥S when Apple's Speak selection also uses ⌥⎋; `src-tauri/src/commands.rs` (`system_speech`), `src/components/Preferences/panes.tsx`.

## 8. Packaging and distribution

- Releases are built and signed **locally**; no CI secrets, no notarization (Tauri skips it unless `APPLE_ID`/`APPLE_API_KEY` are set, which stay unset); integrity = sha256 of the zip + the published certificate leaf hash; `docs/release.md`, `scripts/build-release.sh`.
- Bundle targets are `app` and `dmg`; the zip (via `ditto`) is what the cask downloads, the DMG is for people who prefer one; `src-tauri/tauri.conf.json`, `scripts/build-release.sh`.
- Version must match in `package.json`, `tauri.conf.json`, and `src-tauri/Cargo.toml`; the release script refuses otherwise; `scripts/build-release.sh`.
- **arm64 only for now**: the Homebrew Rust toolchain has no `x86_64-apple-darwin` std; a universal build needs rustup; `docs/release.md`, `distribution/speech-hub.rb` (`depends_on arch: :arm64`).
- **Homebrew cask `jeremym/tap/speech-hub`** installs the app, exposes `speak` as a cask `binary`, strips the quarantine flag in `postflight` (because not notarized), quits by bundle id on uninstall, zaps the app data and prefs, `auto_updates false`, livecheck via GitHub latest; `distribution/speech-hub.rb`.
- `scripts/update-tap.sh` rewrites only the `version` and `sha256` lines of the cask in a local tap clone and stops; the user reviews, commits, and pushes (no tokens); `scripts/update-tap.sh`.
- Manual install path: unzip into /Applications, right-click → Open on first launch; verification via `shasum -a 256` and `codesign -d -r-`; `README.md`.
- `scripts/install-local.sh` builds a signed release, installs to /Applications, strips quarantine, launches; it refuses to run without the release identity or the fetched models; `scripts/install-local.sh`.
- `scripts/create-release-cert.sh` creates the identity once, imports it into the login keychain, trusts it for code signing, exports a password-protected `.p12` backup for the password manager; run attended (two password dialogs); `scripts/create-release-cert.sh`.
- `.p12` files, `binaries/`, `dist/`, models, `.cache/`, and `.context/` are gitignored; `.gitignore`.
- The `speak` sidecar is built for the host triple before every `tauri dev`/`build` by `beforeDevCommand`/`beforeBuildCommand`; `src-tauri/tauri.conf.json`, `scripts/build-cli.sh`.
- Bundling copies ~390 MB of models into the app on every build (slow but correct); `.context/handoff.md`.
- Tests: Rust unit tests across crates (`cargo test`), Vitest for the shortcut grammar (`pnpm test`), `tsc` typecheck; native AX tests are manual and documented; `package.json`, `src/lib/shortcut.test.ts`, `PLAN.md` M7.
- Bench and probe examples: `bench` (perf table), `speedtest` (Pocket speed), `axcap` (selection probe); `src-tauri/examples/`.

## 9. Known gaps and explicitly deferred items

- **M7b not done**: GitHub repo, release zip/DMG upload, Homebrew tap publish (Jeremy pushes); `Status.md`, `PLAN.md` §8.
- **Universal (x86_64) build**: needs rustup's `x86_64-apple-darwin` target; decision pending between installing rustup or staying arm64-only; `PLAN.md` §13, `docs/release.md`.
- **Shortcuts App Intents and `AVSpeechSynthesisProvider`** (system-wide voice for VoiceOver/Speak Screen): both need a Swift/Xcode target that pure-Rust Tauri can't produce; decision pending on an Xcode subproject; `PLAN.md` §8 "Later", §13.
- **Pocket TTS speed control**: sherpa ignores `speed` for Pocket; WSOLA time-stretch is backlog; `PLAN.md` §13, `docs/perf.md`.
- **Selection inside TUI apps in Ghostty** (e.g. Claude Code's input box): mouse-reporting TUIs swallow the drag; needs a real repro; workaround is ⌥-drag or the clipboard fallback Source; `PLAN.md` §13.
- **Ghostty `AXStringForRange` returns empty strings** per the probe table, while the Obsidian note says Ghostty's live selection works; the two records disagree and should be reconciled; `docs/architecture.md`, `Speech Hub/Decisions.md`.
- **Kokoro ja/zh quality** unverified by a speaker; phonemizer uses espeak `ja`/`cmn`; `PLAN.md` §13.
- **Hands-on items still unverified** per the handoff: hotkey conflict with Apple's Speak Selection, full wizard flow, Plus download over the real network, deep links from a browser; `.context/handoff.md` (dated 2026-10-24; `Status.md` says most of the hands-on pass has since succeeded).
- **Open perf measurements**: M1 8 GB run, idle memory after unload, cold start to first tray icon; `docs/perf.md`.
- **Queue mode** (append instead of replace) deferred as a later opt-in; `PLAN.md` §5, `docs/public-api.md`.
- **CoreML / ANE execution provider** deferred research, not a dependency; `PLAN.md` §4.
- **TypeScript client helper** for the socket protocol "may follow"; `docs/public-api.md`.
- `examples/` directory (shell, clipboard-manager, Raycast integrations) is in the target repo layout but does not exist yet; `PLAN.md` §10.
- `bench.sh` / `verify-models.sh` scripts named in the plan don't exist; benchmarking is `cargo run --example bench`; `PLAN.md` §10, `README.md`.
- No GitHub Actions workflow exists despite being listed under M7; `PLAN.md` §8.
- The idle-unload default is 0 (never) and was never tuned against measured reload time; `PLAN.md` §11, `src-tauri/src/settings.rs`.
- `HubState::Cancelling` is defined in the protocol but the hub never enters it (stop goes straight to Ready); `crates/speechhub-protocol/src/lib.rs`, `src-tauri/src/hub.rs`.
- The frontend listens for a `settings:changed` event that no Rust code emits; cross-window settings changes (e.g. Reader changing voice while Preferences is open) are not reflected live; `src/services/hub.ts` (`useSettings`), `src-tauri/src/commands.rs`.
- `docs/security.md` "Permissions the OS asks for" section has a merged/garbled Accessibility bullet and lists Microphone twice (once as requested, once as "not requested"); the second is stale since recording landed; `docs/security.md`.
- `.gitignore` whitelists `src-tauri/resources/models/MANIFEST.json`, but manifests are per-profile `manifest.json` files written by the fetch script and not committed; `.gitignore`, `scripts/fetch-models.sh`.
- Linear-interpolation resampler is "good enough for speech"; `rubato` is the named replacement if quality ever matters; `src-tauri/src/playback.rs`.

## Open questions the author flagged

- ~~Universal build~~ Decided 2026-10-25: Apple Silicon only for v1 (no Intel Mac to verify on; doubles the ONNX payload). `docs/release.md`.
- App Intents and `AVSpeechSynthesisProvider`: add an Xcode/Swift subproject, or keep the CLI/deep-link/Services route documented in `docs/shortcuts.md`? `PLAN.md` §13, `Status.md`.
- Kokoro int8 vs fp16 "decide by ear in M2" was resolved as fp32 by measurement; the Obsidian "Open" block still lists it; `Speech Hub/Decisions.md`, `PLAN.md` §11.
- `speak -o file.wav` in v1 ("leaning yes") is implemented as `render`; `PLAN.md` §11 still lists it as open.
- Idle-unload default minutes: "decide after measuring reload time"; still 0; `PLAN.md` §11.
- Flag left on after a successful Chromium warm-up: "flag left on once proven needed"; proven needed and kept on, but the tradeoff (browser keeps a full AX tree) is acknowledged, not resolved; `PLAN.md` §9, `src-tauri/src/accessibility.rs`.
- ~~Ghostty TUI selection~~ Reproduced 2026-10-25: Ghostty exposes the selection, but Claude Code's redraws clear it within seconds. Supported path is the fallback Source = Current clipboard (Ghostty copies on select). `PLAN.md` §13.
- Whether to re-measure on an M1 8 GB before release; `PLAN.md` §4, `docs/perf.md`.
- Pocket TTS license wording: the sherpa tarball README says "non-commercial" while the LICENSE file is plain CC-BY-4.0; the app shows CC-BY-4.0 plus the consent clause; `docs/model-licenses.md`, `src-tauri/resources/models/plus/README.md`.
- `shortcuts run` output quirks were flagged as a risk to "verify early in M4"; the implementation uses `--output-path`/`--output-type` and falls back to stdout, but no verification note was recorded; `PLAN.md` §9, `src-tauri/src/sources/exec.rs`.
- Where the espeak-ng data files are extracted at runtime (a temp dir on first use) and whether that survives hardened-runtime/sandbox changes; `docs/model-licenses.md`.
- The "Apple's built-in speech" card reads `defaults` for `com.apple.universalaccess`; whether those keys are stable across macOS versions is unverified (verified on macOS 26 only); `src-tauri/src/commands.rs`.

## Preferences UI round (2026-10-25)

- Every settings group is a `Section`: title and a one-line description in a shaded header, controls underneath; `src/components/ui.tsx`.
- Keyboard shortcuts are always rendered with `Kbd` in the accent colour, so "keys you press" is one visual idea across all windows; the shortcut recorder itself is the big key-cap chip in the section header; `src/components/ui.tsx`, `src/components/Preferences/panes.tsx`.
- Speak Selection and Quick Speak are two separate sections, each ending with a "Right now: press X → does Y" sentence built from the live settings; `panes.tsx`.
- The fallback Source ("If the text can't be read") moved from Accessibility to Shortcuts → Speak Selection, where the shortcut it modifies lives; Accessibility only points there; `panes.tsx`.
- Vocabulary fixed app-wide and listed under About → "The words Speech Hub uses": Speak Selection, Quick Speak, Quick Speak panel, Source, Reader, Mini player, Engine, Voice, Pronunciation rule. "Palette" is never shown to users; `panes.tsx`.
- Preferences nav is grouped: App (General, Accessibility) · Speech (Engine, Voices, Pronunciation) · Ways to speak (Shortcuts, Sources, Command line) · About; `Preferences.tsx`.
- Engine badges lost their milestone jargon: Included / Installed / Optional download / Missing from this build / In use; `panes.tsx`.
- Services declare `NSStringPboardType` alongside `public.utf8-plain-text`: Chromium's `validRequestorForSendType:` compares against the legacy constant, so UTI-only services never appear in Chrome/Brave context menus; `src-tauri/Info.plist`.
