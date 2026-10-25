// Shortcut strings use the tauri-plugin-global-shortcut grammar: "Alt+Shift+Escape".

const MOD_ORDER = ["Control", "Alt", "Shift", "Super"] as const;

/** Build a shortcut string from a keydown event. Returns null for modifier-only presses. */
export function eventToShortcut(e: { code: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }): string | null {
  const key = codeToKey(e.code);
  if (!key) return null;
  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (e.metaKey) mods.push("Super");
  if (mods.length === 0 && !/^F\d+$/.test(key)) return null; // a bare key (even Escape) would be swallowed system-wide
  return [...mods, key].join("+");
}

function codeToKey(code: string): string | null {
  if (/^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/.test(code)) return null;
  const letter = /^Key([A-Z])$/.exec(code);
  if (letter) return letter[1];
  const digit = /^Digit(\d)$/.exec(code);
  if (digit) return digit[1];
  if (/^(F\d{1,2}|Escape|Space|Enter|Tab|Backspace|Delete|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|Minus|Equal|Comma|Period|Slash|Backquote|BracketLeft|BracketRight|Backslash|Semicolon|Quote)$/.test(code)) return code;
  return null;
}

const SYMBOL: Record<string, string> = {
  Control: "⌃", Alt: "⌥", Shift: "⇧", Super: "⌘", Escape: "⎋", Space: "␣", Enter: "↩", Tab: "⇥",
  Backspace: "⌫", Delete: "⌦", ArrowUp: "↑", ArrowDown: "↓", ArrowLeft: "←", ArrowRight: "→",
  Minus: "-", Equal: "=", Comma: ",", Period: ".", Slash: "/", Backquote: "`", BracketLeft: "[", BracketRight: "]",
  Backslash: "\\", Semicolon: ";", Quote: "'",
};

/** "Alt+Shift+Escape" → "⌥⇧⎋" for display. */
export function formatShortcut(s: string): string {
  const parts = s.split("+").map((p) => p.trim());
  const mods = parts.slice(0, -1).map(normalizeMod);
  mods.sort((a, b) => MOD_ORDER.indexOf(a as never) - MOD_ORDER.indexOf(b as never));
  const key = parts[parts.length - 1] ?? "";
  return [...mods, key].map((p) => SYMBOL[p] ?? p).join("");
}

function normalizeMod(m: string): string {
  const l = m.toLowerCase();
  if (l === "option" || l === "alt") return "Alt";
  if (l === "ctrl" || l === "control") return "Control";
  if (l === "cmd" || l === "command" || l === "super" || l === "meta") return "Super";
  if (l === "shift") return "Shift";
  return m;
}
