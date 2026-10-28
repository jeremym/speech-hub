# The look: Speech Hub on the P1 app kit

Speech Hub's windows are drawn with `@overtone-foundry/app-kit` (working name P1): the phosphor
terminal look shared with Stillroom. Dark glass, glowing text with a crisp core and a soft halo,
bracketed fields, status words in small caps, and paper for anything the user reads as content.
Speech Hub uses the kit's default theme, **green**, like Stillroom. Amber and paper remain
selectable under Preferences › General › Look and in the View menu, and text size has three steps.
The screenshots in `docs/screenshots/` were taken in amber, before the default changed to green.

The kit's rules are in its `SKILL.md` (read it before changing any screen). The short version:
glow is the kit's `.glow` classes, never a blur; body text is 17 px minimum; status is a word,
never a colour alone; amber means "needs you" or "leaves this Mac", red is failure only; one filled
primary action per screen; confirmations are inline ("Really remove" replaces the button); the only
popup is `Help`; nothing is fetched at run time.

## Setup

- `package.json`: `@overtone-foundry/app-kit` is linked from `../overtone-foundry-app-kit` (no
  remote yet). pnpm only. If the kit's `dist/` is missing, run `pnpm build` in the kit folder.
- `src/main.tsx` imports `@overtone-foundry/app-kit/kit.css` and applies the saved look (`src/look.ts`).
  With nothing saved the kit's default applies: green, normal size, no scanlines.
- `src/App.css` is the only app CSS. It uses the kit's tokens and nothing else: no colours, no
  pixel font sizes.
- `src/brand.tsx` holds the app's mark (the kit ships none) and the `Logo` wrapper.
- Tailwind was removed: once the kit styles bare HTML, no utility class was left in use.
- `vite.config.ts` dedupes React and Lucide (`resolve.dedupe`) because the linked kit has its own
  `node_modules`; without it the kit's components run on a second React and every hook fails. The
  dev server is also allowed to read the kit folder, where its fonts live.
- Each window is its own webview, so a look change is broadcast on the Tauri event `look:changed`
  and every window applies it. The native View menu also switches theme and text size.

## Screens and what they are built from

| Window | Size | Built from |
|---|---|---|
| Onboarding | 820 × 640, fixed | `.shell`, pane components below, `Progress` for the steps, footer `.row.space` |
| Preferences | 960 × 680 (min 860 × 560) | `.settings-shell`, `.settings-nav` (grouped), one pane at a time |
| Reader | 680 × 500 (min 460 × 320) | `Rail` toolbar, the text in `Paper`, a status `.row` |
| Mini player | 340 × 80, floating | `.player` (App.css) with a `.dot`, a `.status-word`, glyph buttons |
| Quick Speak | 560 × 420, floating | a bracketed input, `ul.rows` of results, inline confirm with `button.amber` |

Preference panes:

| Pane | Pieces |
|---|---|
| General | `Zone`s; `.perm-row` lines (label + hint on the left, control on the right); `Toggle`; Look: `Segmented` theme and text size, `Toggle` scanlines |
| Accessibility | `Zone` with a `Tag` status (`granted`, `not granted`); `button.primary` Grant access |
| Engine | `.cards` of `div.card` per profile with `Tag`s, quality and speed as `blocks()`; Plus download is `button.amber` (it fetches from GitHub) with `Progress` |
| Voices | search `input`, `ul.rows`, `Tag` default/custom, quiet Preview, inline delete confirm; the recorder is a `Zone.editing` with a `live` tag while recording |
| Pronunciation | the try-it line and the rule list as `ul.rows`; the editor is a `Zone.editing` |
| Shortcuts | one `Zone` per shortcut, the keys as a display-face `button` that turns amber while recording; Apple conflicts in `.notice` |
| Sources | `ul.rows`; templates as kit `Card`s in `.cards`; the editor's sections as sibling `Zone`s; untrusted imports get `amber-rim` |
| Command line | `Zone` with a `Tag` status, `pre` examples |
| About | `.facts` grids |

Glyphs: `Glyph` names from the kit, chosen by meaning (`play`, `stop`, `close`, `plus`, `audio`,
`file`, `program`, `sparkles`, `globe`, `text`, `settings`, `help`, `warning`, `key`, `shield`).
A Source's own icon is an emoji the user typed; it is data and is shown as typed.

## What the kit did not have

Built locally in `src/App.css`, worth moving upstream:

- `input[type="range"]` styled on glass (accent colour, track) for the speed slider.
- A setting line: label and hint on the left, the control on the right. `.perm-row` is close and
  is reused here as `.perm-row` with `.perm-main` and `.perm-act`; a general name would help.
- A small floating window (`.player`) with no Rail or shell: compact padding and a drag region.
- A command-palette list: an `ul.rows` whose rows are buttons with a keyboard-selected row.
- `.settings-nav` group labels (`.nav-group`).
- A `Glyph` for a microphone and for a keyboard shortcut would replace two text marks.
