# Security model

Speech Hub speaks text. It should never become a way to run code the user didn't ask for, or to
move text off the machine.

## Trust boundaries

| Untrusted input | Where it enters | What we do |
|---|---|---|
| Text to speak | hotkey, CLI, socket, Sources, links | Size caps (1 MiB; 2 KB for links). Rendered as text only, never as HTML. Never logged. |
| Socket requests | `run/hub.sock` | Dir 0700, socket 0600, peer uid must equal ours (`getpeereid`). Versioned, schema-validated, length-prefixed frames with a hard cap. One request per connection. |
| `speechhub://` links | any app or web page | Only `open`, `palette`, `preferences`, `stop`, `speak?text=` (rate-limited), and `source/<id>` which **asks** in the palette. No paths, no commands, no fetches. |
| Imported Sources | JSON files | Ids regenerated; executable kinds marked untrusted until approved in the Builder. |
| Command / Shortcut Sources | user-configured | Run only when trusted. argv array, no shell unless opted in. Scrubbed env. Timeout. 1 MB output. |
| HTTP responses | URL Sources | System `curl`, http/https only, 1 MB cap, timeout. Treated as text. |
| Model downloads | Plus | Fixed URL on GitHub releases, SHA-256 pinned in code, verified before extraction, extracted into a staging dir then swapped in. |
| Reference recordings | Plus voices | Decoded with `hound`, re-encoded, trimmed to 30 s. Consent flag required. |

## What is deliberately absent

- No HTTP server, no listening TCP port, ever.
- No telemetry, accounts, update pings. The only network calls are user-configured URL Sources and
  the one-click Plus download.
- No clipboard code. No clipboard history.
- No public API verb that executes a command or reads an arbitrary path. `run_source` references a
  Source the user already created and approved.
- No logging of spoken text or Source output. Logs carry ids, sizes, and timings.

## Privileged vs benign

**Benign**: speaking supplied text, opening the Reader, status, voices, stop/pause.
**Privileged**: running a Source of an executable kind. Reachable only by id, only when trusted,
only from same-user callers or after an in-app confirmation.

## Permissions the OS asks for

- **Accessibility**: to read the selection in other apps. Requested in onboarding, never required for the CLI or Sources. Capture may set `AXEnhancedUserInterface` on a browser so it builds its accessibility tree (the same thing VoiceOver does); that flag is left on only for apps that needed it to answer, and it is never set on an app that answered directly.
- **Microphone**: only when you click "Record a voice" in Preferences → Voices (custom Plus voices). Hardened-runtime entitlement `com.apple.security.device.audio-input`; the recording is encoded to WAV in the window and handed to the voice library, nothing is kept beyond the trimmed reference clip.
- **Services menu**: macOS passes the selected text on a private Services pasteboard. Speech Hub reads that pasteboard only inside the service call; it never reads or writes the general clipboard.

## Reporting

Open an issue tagged `security`. The maintainer will respond on a best-effort basis.
