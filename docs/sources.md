# Sources

A **Source** is a saved answer to "where does the text come from?". Build one in
Preferences → Sources, then run it from Quick Speak (⌥⇧⎋), the menu bar, the second shortcut, a
`speechhub://source/<name>` link (with confirmation), or `speak run <name>`.

## Kinds

| Kind | What it does | Needs approval? |
|---|---|---|
| Saved text | Speaks a fixed snippet. | no |
| File | Reads a `.txt`/`.md` file you picked (1 MB cap, Markdown syntax is read as prose). | no |
| Command | Runs a program with an argument list; stdout becomes the text. No shell unless you turn on *Run as a shell command*. | **yes** |
| Apple Shortcut | Runs a Shortcut by name and reads its text result. | **yes** |
| URL | HTTP GET when it runs. Plain text as-is, JSON via a field path (`items.0.title`), HTML reduced to text. | no |

Every Source can set its own voice, speed, and whether it **speaks immediately** or **opens in the
Reader** first.

## Templates (one click)

- **Current clipboard**: `/usr/bin/pbpaste`. This is how clipboard support works: there is no
  clipboard code in Speech Hub, no history, no monitoring. Press ⌘C, then run the Source.
- **Output of a command**: `/bin/date` with a friendly format, as a worked example.
- **A text or Markdown file**, **An Apple Shortcut**, **Text from a URL**, **A saved snippet**.

## Approval ("trusted")

Command and Shortcut Sources run programs. Ones you build in the app are approved when you save
them. Ones that arrive by **import** are not: they show *Needs approval* and refuse to run until
you open them, read what they do, and click **Approve & Save**. A `speechhub://source/...` link
never runs anything without a confirmation in the palette.

## Limits and safety

- Timeout per run (default 15 s, max 120 s). Output capped at 1 MB.
- Child processes get a minimal environment (`PATH`, `HOME`, `USER`, `TMPDIR`, `LANG`, `SHELL`,
  plus `SPEECH_HUB=1`). Your API keys and other variables are not passed through.
- Commands take an argv array: `$HOME` in an argument is the literal string `$HOME`.
- URLs are fetched with the system `curl`, `http`/`https` only, 1 MB cap, when the Source runs
  and never otherwise.
- The public listing (`speak sources list`, the palette) shows names and kinds only, not
  arguments, paths, or URLs.

## Recipes

**Read my daily note**: File → `~/Notes/Today.md`, action *Open in Reader*.

**Speak what my clipboard manager picked**: in the manager, run `printf '%s' "$ITEM" | speak`.
You don't need a Source for that; the CLI is simpler.

**Morning briefing from a Shortcut**: make a Shortcut that gathers weather + calendar and ends
with a *Text* action; create a Shortcut Source named "Morning briefing"; set the second shortcut to
run it, or say `speak run morning-briefing` from a cron job.

**Latest headline from a JSON API**: URL `https://example.com/feed.json`, path `items.0.title`.
