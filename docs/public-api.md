# Public API: the `speak` command and the socket protocol

Speech Hub is a shared speech service. Two ways in, same engine, same voices, same pronunciation rules.

## 1. `speak` (recommended)

Install from **Preferences → Command Line → Install**. It symlinks `~/.local/bin/speak` to the binary
inside the app, so updates stay in sync and no password is needed.

```bash
echo "Build finished" | speak                 # speak now
speak "Hello, world"                          # positional text
speak -f ~/notes.md                           # a file
speak -v bm_george --speed 1.2 "Cheers"       # voice + speed
pbpaste | speak --open                        # open in the Reader instead of speaking
speak -o out.wav "Render me"                  # render to WAV, no playback
speak run morning-briefing                    # run a saved Source (by name or id)
speak palette                                 # open Quick Speak
speak stop | pause | resume
speak status --json
speak voices --json
speak sources list
```

`say` compatibility: `-v`, `-r <wpm>`, `-f`, `-o` behave like macOS `say`, so `alias say=speak` works
for the common cases.

If the app isn't running, `speak` launches it in the background and waits (default 10 s). `status`,
`stop`, `pause`, `resume`, and `version` never launch it.

Exit codes: `0` ok · `1` request rejected · `2` app unavailable · `3` engine not ready or permission
missing · `64` usage error. Errors go to stderr; `--json` puts them on stdout as `{"ok":false,...}`.

Limits: 1 MiB of UTF-8 text per request. Text is never logged by the app or the CLI.

## 2. Socket protocol (for apps that don't want to shell out)

- Socket: `~/Library/Application Support/com.jeremym.speechhub/run/hub.sock`, mode 0600, same-uid only.
- Framing: 4-byte big-endian length, then UTF-8 JSON. One request per connection.
- Max frame: 1 MiB + 64 KiB.

Request:

```json
{ "version": 1, "request_id": "any-unique-string", "action": "speak",
  "text": "Hello", "options": { "voice_id": "af_heart", "speed": 1.1, "title": "My app" } }
```

Actions: `speak`, `open`, `render` (`path` must end in `.wav`), `run_source` (`source_id`), `stop`,
`pause`, `resume`, `status`, `list_voices`, `list_sources`, `palette`, `version`.

Response:

```json
{ "ok": "true",  "request_id": "...", "data": { ... } }
{ "ok": "false", "request_id": "...", "code": "text_too_large", "message": "..." }
```

Error codes: `invalid_request`, `unsupported_version`, `text_too_large`, `engine_not_ready`,
`permission_required`, `selection_unavailable`, `source_not_found`, `source_not_trusted`,
`source_failed`, `internal`.

There is deliberately **no** action that executes a command or reads a file path. `run_source` only
references a Source the user already created and approved in the app.

Types live in `crates/speechhub-protocol` (Rust, serde). A tiny TypeScript helper may follow.

## 3. `speechhub://` links

For apps that can open URLs but not run commands. Links are untrusted; they can only:

- `speechhub://open`, `speechhub://palette`, `speechhub://preferences`, `speechhub://stop`
- `speechhub://speak?text=...` (≤ 2 KB, max one per second)
- `speechhub://source/<id-or-name>` — asks the user to confirm before running (Milestone 4)

Put real text through `speak`, not through links.

## Playback policy

A new `speak` replaces whatever is playing. `stop` cancels synthesis and playback immediately.
Obsolete audio is discarded, never played late. Queueing may come later as an opt-in.
