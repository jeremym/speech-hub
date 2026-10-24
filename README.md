# Speech Hub

**Give anything on your Mac a voice.**

Speech Hub is a menu-bar utility for macOS that reads selected text aloud with a natural, fully local AI voice. It is also a shared speech service: any app, script, Apple Shortcut, or user-built "Source" can speak through it with one command.

> Status: early development, Milestone 1 (native shell). See [PLAN.md](PLAN.md) for the roadmap.

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

## Build

Requirements: Rust (stable), Node 20+, pnpm, Xcode command line tools.

```bash
pnpm install
pnpm tauri dev          # run
pnpm tauri build        # .app + .dmg in src-tauri/target/release/bundle
cargo test              # Rust tests
pnpm test               # frontend tests
```

The debug build uses a dev-only placeholder engine (macOS `say` rendered to PCM) so the pipeline can be exercised before the real engines land in Milestone 2.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE). Bundled models and libraries carry their own licenses; see `docs/model-licenses.md` (Milestone 2).
