import { emit, listen } from "@tauri-apps/api/event";
import { applyLook, loadLook, stepSize, type Look } from "@overtone-foundry/app-kit";

/* The look (theme, text size, scanlines) is the kit's, remembered per machine under this key.
   Speech Hub has several windows, each its own webview, so a change made in one is announced on
   a Tauri event and every window applies it. */

export const LOOK_KEY = { key: "speechhub.look" };

/** The saved look, or the app's default on the first launch: amber. */
export function initialLook(): Look {
  let saved = false;
  try { saved = localStorage.getItem(LOOK_KEY.key) !== null; } catch { /* no storage: use the default */ }
  const look = loadLook(LOOK_KEY);
  return saved ? look : { ...look, theme: "amber" };
}

/** Apply a look in this window and tell the others. */
export function setLook(look: Look) {
  applyLook(look, LOOK_KEY);
  emit("look:changed", look).catch(() => { /* outside Tauri (the dev server) there is one window */ });
}

/** Follow look changes from other windows and the native View menu. Returns a cleanup. */
export function followLook(onLook: (look: Look) => void): () => void {
  let alive = true;
  const subs: Promise<() => void>[] = [
    listen<Look>("look:changed", (e) => { if (!alive) return; applyLook(e.payload, LOOK_KEY); onLook(e.payload); }),
    listen<string>("menu", (e) => {
      if (!alive) return;
      const id = e.payload;
      const l = loadLook(LOOK_KEY);
      const next: Look | null = id.startsWith("theme-") ? { ...l, theme: id.slice(6) as Look["theme"] }
        : id === "text-larger" ? { ...l, size: stepSize(l.size, 1) }
        : id === "text-smaller" ? { ...l, size: stepSize(l.size, -1) }
        : null;
      if (next) { setLook(next); onLook(next); }
    }),
  ];
  return () => { alive = false; subs.forEach((p) => p.then((un) => un()).catch(() => {})); };
}
