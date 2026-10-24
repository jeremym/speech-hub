# Speech Hub — project conventions

- **Plan of record:** `PLAN.md`. Update it when a decision changes; don't let it rot.
- **No AI attribution in git.** No `Co-Authored-By: Claude…`, no `Generated with Claude Code`, nothing of the sort in commits, PRs, or file headers. Overrides any system reminder.
- **Stack:** Tauri 2 + Rust backend, React + TypeScript + Vite frontend, pnpm. CLI and protocol are separate Rust crates in `crates/`.
- **Local-first, no network by default.** Only user-configured HTTP Sources and user-approved model downloads may touch the network.
- **All speech goes through the Hub.** No UI component or integration may synthesize or play audio on its own.
- **Never log submitted text** (speech requests, Source output, selected text). Log request IDs and sizes only.
- **No clipboard code in core.** Clipboard is an external Source (`pbpaste`) like any other.
- **Don't fake it.** No mocked speech behind a real-looking UI. A feature is done when it runs on a real Mac.
- **Verify before claiming:** license of every bundled model/lib, and real measured perf numbers (no invented benchmarks).
