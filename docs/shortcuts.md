# Using Speech Hub from Shortcuts, Automator, and other tools

Speech Hub doesn't ship App Intents (those need a Swift target; see PLAN.md §13). Everything below works today through the `speak` command line and `speechhub://` links, so Apple Shortcuts, Automator, Keyboard Maestro, Raycast, Alfred, BetterTouchTool, and plain shell scripts can all speak through the Hub.

## Apple Shortcuts

Install the CLI first: Preferences → Command Line → Install (it links `~/.local/bin/speak` to the copy inside the app).

**Speak text** — add a *Run Shell Script* action:

```sh
~/.local/bin/speak --stdin
```

Set *Input* to the previous action's output and *Pass input* to **stdin**. Shortcuts runs scripts with a minimal `PATH`, so use the full path to `speak`.

**Speak with a specific voice or speed**

```sh
~/.local/bin/speak --voice bf_emma --speed 1.2 --stdin
```

**Open text in the Reader instead of speaking**

```sh
~/.local/bin/speak open --stdin
```

**Run a saved Source** (file, command, URL, or another Shortcut you set up in Preferences → Sources):

```sh
~/.local/bin/speak run "Morning briefing"
```

**Without the CLI** — an *Open URLs* action with a `speechhub://` link also works, with limits (2 KB of text, one per second):

```
speechhub://speak?text=Hello%20there
speechhub://open
speechhub://palette
speechhub://stop
```

Stopping, pausing, resuming: `speak stop`, `speak pause`, `speak resume`.

## Services menu

Select text in any app, then right-click → Services → **Speak with Speech Hub**, or **Open in Speech Hub Reader**. Assign a keyboard shortcut to either in System Settings → Keyboard → Keyboard Shortcuts → Services → Text. This path uses the text the app hands to the Services pasteboard, so it works in apps whose Accessibility selection Speech Hub can't read.

## Shell scripts, cron, Makefiles

```sh
echo "Build finished" | speak
speak -o summary.wav -f notes.md     # render to a file instead of playing
speak status --json
```

Exit codes: 0 ok · 1 request rejected · 2 Speech Hub unavailable · 3 engine not ready or permission missing · 64 usage. `speak` launches the app if it isn't running; add `--no-launch` to fail instead.

## Keyboard Maestro, Raycast, Alfred, BetterTouchTool

Any "run shell script" or "open URL" step works the same way as the Shortcuts examples above. Raycast script commands: `#!/bin/sh` + `~/.local/bin/speak "$1"`.
