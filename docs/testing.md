# Manual test matrix

Tick every row on a real Mac before calling a build done. ⌥⎋ = Speak selection, ⌥⇧⎋ = Quick Speak.

## Selection capture (⌥⎋)

| App | Expect | ✓ |
|---|---|---|
| TextEdit / Notes / Mail | instant speech | |
| Safari (page text, text field) | instant | |
| Brave / Chrome, cold launch | instant if "Prepare browsers" is on; else "Waking up…" then speech | |
| Brave / Chrome, warm | instant | |
| Firefox | instant | |
| VS Code / Cursor | speech (Electron = Chromium path) | |
| Ghostty, plain shell output | speech | |
| Ghostty while Claude Code runs | selection clears within seconds (Claude Code redraws); with fallback Source = Current clipboard, ⌥⎋ still speaks it | |
| Terminal.app / iTerm2 | speech | |
| Slack / Discord | speech | |
| PDF in Preview | speech | |
| Nothing selected | per Shortcuts → “If the text can't be read”: notice within ~30 ms / Quick Speak panel opens / fallback Source runs | |
| ⌥⇧⎋ with text selected | panel shows “Selected text in <app>” with Speak and Open in Reader rows | |
| App that hides selection | "doesn't expose its selected text"; fallback Source runs if set | |
| Stop button during "Waking up…" | capture cancels, player resets | |
| ⌥⎋ again while speaking | obeys "Pressing again while speaking" (replace / stop) | |
| ⌥⎋ with Accessibility permission revoked | notice + Open System Settings | |

## Windows & tiling

| Scenario | Expect | ✓ |
|---|---|---|
| AeroSpace: Preferences, Reader | tile (`aerospace list-windows --all --format '%{window-layout}'` → h_tiles) | |
| AeroSpace: mini player, palette | float, on every space | |
| Native full-screen app in front, press ⌥⎋ | player appears over it | |
| Native full-screen app, ⌥⇧⎋ | palette appears over it | |
| Close Preferences (⌘W / red button) | hides; app stays in menu bar; Dock icon disappears | |
| ⌘, from palette / Reader | Preferences opens | |
| Esc in palette | closes | |
| Multiple displays | known gap: player uses the primary display, palette keeps its first position | |
| Stage Manager on | windows still appear | |

## Input paths

| Path | Expect | ✓ |
|---|---|---|
| Right-click text → Services → Speak with Speech Hub (TextEdit, Safari, Chrome) | item listed; speaks. If missing: relaunch the app, then System Settings → Keyboard → Keyboard Shortcuts → Services → Text | |
| Right-click text → Services → Open in Speech Hub Reader | Reader opens with text | |
| `speak "hello"` | speaks | |
| `echo hi \| speak` | speaks | |
| `speak status` | ready, engine, voice | |
| `speak run "Say the date"` | speaks today's date | |
| `open "speechhub://speak?text=hi"` | speaks | |
| `open "speechhub://source/<id>"` | runs Source | |
| Deep link while app not running | app launches, waits for the engine, then speaks | |
| `echo hi \| speak` while app not running | app launches, waits for the engine, then speaks | |
| Palette: Esc, then ⌥⎋ in another app | reads the new selection, not the palette's old one | |
| Run a Speak Source from the palette after the Reader was opened once | speaks; no hang | |
| Palette: type Source name, Enter | runs | |
| Palette with selection pending | reads the selection captured at open time | |

## Engines, voices, audio

| Scenario | Expect | ✓ |
|---|---|---|
| Switch Standard ↔ Lite ↔ Plus | first speech after switch < 2 s | |
| Speed slider (Standard/Lite) | audibly faster/slower | |
| Speed slider (Plus) | ignored; pane says so | |
| Record a voice (Plus) | mic TCC prompt once; voice appears; speaks in that voice | |
| Add from WAV | voice appears | |
| Pause / resume / stop from player | immediate | |
| Long text (10k chars) in Reader | starts < 1 s, no gaps between chunks | |
| Idle unload (set 1 min) | RSS drops after idle; next speech reloads | |
| Headphones unplugged / AirPods connected mid-speech | output follows the new default device within ~1 s | |
| No output device at all | job ends with "No audio output device"; app keeps running | |
| ⌘Q in Preferences | windows close, speech service keeps running, player shows a notice | |
| Model download (fresh machine) | consent dialog first; progress; restarts from zero if interrupted | |

## System & lifecycle

| Scenario | Expect | ✓ |
|---|---|---|
| Launch at login on → log out/in | app in menu bar | |
| Second launch of the app | focuses existing, no second instance | |
| Quit from tray | socket file removed; `speak status` says not running | |
| Apple Spoken Content "Speak selection" on | Shortcuts pane shows On + conflict note | |
| Sleep / wake | hotkeys still work | |
| Fast user switch | hotkeys still work in the original session | |

## Security

| Check | Expect | ✓ |
|---|---|---|
| `codesign -dv --entitlements - "/Applications/Speech Hub.app"` | hardened runtime, audio-input only | |
| `ls -l ~/Library/Application\ Support/com.jeremym.speechhub/*.sock` | 0600 | |
| Log file after speaking a secret | no text, only ids/sizes | |
| `speechhub://` with junk payload | rejected, no crash | |
