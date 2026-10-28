import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Empty, Glyph, Tag, type GlyphName } from "@overtone-foundry/app-kit";
import { hub, errorMessage } from "../../services/hub";
import type { PublicSource } from "../../types";

interface Item { id: string; glyph?: GlyphName; icon?: string; label: string; hint: string; run: () => Promise<unknown> | void; section: "selection" | "actions" | "sources" | "typed" }

const TYPE_LABEL: Record<PublicSource["type"], string> = { text: "snippet", file: "file", command: "command", shortcut: "Shortcut", http: "URL" };

/** Quick Speak palette: typed text, built-in actions, and the user's Sources. Keyboard first. */
export default function Palette() {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [sources, setSources] = useState<PublicSource[]>([]);
  const [confirm, setConfirm] = useState<PublicSource | null>(null);
  const [selection, setSelection] = useState<{ preview: string; chars: number; appName: string | null } | null>(null);
  const input = useRef<HTMLInputElement>(null);

  const reload = () => hub.sources.listPublic().then(setSources).catch(() => {});
  useEffect(() => {
    const focus = () => {
      setQ(""); setSel(0); setErr(null); reload();
      hub.paletteSelectionPreview().then(setSelection).catch(() => setSelection(null));
      hub.sources.takePendingConfirm().then((c) => c && setConfirm(c)).catch(() => {});
      setTimeout(() => input.current?.focus(), 30);
    };
    focus();
    const un1 = listen<string>("window:shown", (e) => e.payload === "palette" && focus());
    const un2 = listen("sources:changed", reload);
    const un3 = listen<PublicSource>("palette:confirm-source", (e) => setConfirm(e.payload));
    // The selection is captured in the background (browsers can take seconds to expose it).
    const un4 = listen("palette:selection", () => hub.paletteSelectionPreview().then(setSelection).catch(() => setSelection(null)));
    return () => { [un1, un2, un3, un4].forEach((u) => u.then((f) => f())); };
  }, []);

  const close = () => { setConfirm(null); hub.hideWindow("palette"); };
  const typed = q.trim();
  const items: Item[] = useMemo(() => {
    const ql = typed.toLowerCase();
    const matches = (s: string) => !ql || s.toLowerCase().includes(ql);
    const preview = selection ? `“${selection.preview.length > 40 ? selection.preview.slice(0, 40) + "…" : selection.preview}” · ${selection.chars.toLocaleString()} chars` : "";
    const from = selection?.appName ? ` from ${selection.appName}` : "";
    const selectionItems: Item[] = selection ? ([
      { id: "selection", glyph: "audio", label: `Speak the selected text${from}`, hint: preview, run: () => hub.speakSelection(), section: "selection" as const },
      { id: "selection-open", glyph: "notes", label: "Open the selected text in the Reader", hint: "listen with controls, fix a pronunciation", run: () => hub.openSelection(), section: "selection" as const },
    ] as Item[]).filter((a) => matches(a.label)) : [];
    const actions: Item[] = ([
      ...(selection ? [] : [{ id: "selection", glyph: "text" as const, label: "Speak selected text", hint: "nothing was selected when this opened", run: () => hub.speakSelection(), section: "actions" as const }]),
      { id: "reader", glyph: "notes", label: "Open an empty Reader", hint: "type or paste anything", run: () => hub.openWindow("reader"), section: "actions" as const },
      { id: "new-source", glyph: "plus", label: "New Source…", hint: "file, command, Shortcut, URL", run: () => hub.openWindow("settings", "sources-new"), section: "actions" as const },
    ] as Item[]).filter((a) => matches(a.label));
    const src: Item[] = sources.filter((s) => s.enabled && (matches(s.name) || matches(s.description))).map((s) => ({
      id: s.id, icon: s.icon, label: s.name, section: "sources",
      hint: `${s.trusted || (s.type !== "command" && s.type !== "shortcut") ? "" : "needs approval · "}${TYPE_LABEL[s.type]} · ${s.action === "open" ? "opens Reader" : "speaks"}`,
      run: () => hub.sources.run(s.id),
    }));
    const typedItems: Item[] = typed ? [
      { id: "speak-typed", glyph: "audio", label: `Speak “${typed.length > 48 ? typed.slice(0, 48) + "…" : typed}”`, hint: "↩", run: () => hub.speak(typed, { title: "Quick Speak" }), section: "typed" as const },
      { id: "open-typed", glyph: "notes", label: "Open this text in the Reader", hint: "⇧↩", run: () => hub.open(typed, "Quick Speak"), section: "typed" as const },
    ] : [];
    // Sources first when searching (that's what people type), actions first when idle.
    return typed ? [...src, ...typedItems, ...selectionItems, ...actions] : [...selectionItems, ...actions, ...src];
  }, [typed, sources, selection]);

  const run = async (item: Item) => {
    try { await item.run(); close(); } catch (e) { setErr(errorMessage(e)); }
  };
  const runConfirmed = () => { const c = confirm; if (!c) return; setConfirm(null); hub.sources.run(c.id).then(close).catch((er) => setErr(errorMessage(er))); };
  const onKey = (e: React.KeyboardEvent) => {
    if (confirm) {
      if (e.key === "Escape") { e.preventDefault(); setConfirm(null); }
      if (e.key === "Enter") { e.preventDefault(); runConfirmed(); }
      return;
    }
    if (e.key === "Escape") { e.preventDefault(); close(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(items.length - 1, s + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(0, s - 1)); }
    else if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey && typed) run(items.find((i) => i.id === "open-typed")!);
      else if (items[sel]) run(items[sel]);
    }
  };

  const needsApproval = !!confirm && !confirm.trusted && (confirm.type === "command" || confirm.type === "shortcut");
  let lastSection: Item["section"] | null = null;
  return (
    <div className="palette" onKeyDown={onKey}>
      <div data-tauri-drag-region className="palette-input">
        <span className="prompt" aria-hidden="true">›</span>
        <input ref={input} type="text" value={q} onChange={(e) => { setQ(e.target.value); setSel(0); }} disabled={!!confirm}
          placeholder="Search Sources, or type text to speak…" aria-label="Search Sources, or type text to speak" />
        <kbd>esc</kbd>
      </div>
      {confirm ? (
        <div className="palette-confirm">
          <h2>A link wants to run <span className="src-icon" aria-hidden="true">{confirm.icon}</span>{confirm.name} <Tag tone="amber">needs you</Tag></h2>
          <p>{confirm.summary}. {confirm.type === "command" || confirm.type === "shortcut" ? "This runs a program on your Mac." : ""} Only continue if you expected this.</p>
          {needsApproval && <div className="notice"><Glyph name="warning" /> Not approved yet. Approve it in Preferences → Sources first.</div>}
          <div className="row">
            <button className="amber" onClick={runConfirmed} disabled={needsApproval}>Run it <kbd>↩</kbd></button>
            <button className="quiet" onClick={() => setConfirm(null)}>Cancel <kbd>esc</kbd></button>
          </div>
        </div>
      ) : (
        <div className="palette-list" role="listbox" aria-label="Quick Speak">
          {items.length === 0 && <Empty word="Nothing">No Source or action matches.</Empty>}
          <ul className="rows">
            {items.map((it, idx) => {
              const header = it.section !== lastSection && !typed ? it.section : null;
              lastSection = it.section;
              return (
                <li key={it.id} className={idx === sel ? "here" : ""} role="presentation">
                  {header === "selection" && <h3>Selected text{selection?.appName ? ` in ${selection.appName}` : ""}</h3>}
                  {header === "actions" && selection && <h3>Other</h3>}
                  {header === "sources" && sources.length > 0 && <h3>Your Sources</h3>}
                  <button className="item" role="option" aria-selected={idx === sel} onMouseEnter={() => setSel(idx)} onClick={() => run(it)}>
                    {it.glyph ? <Glyph name={it.glyph} /> : <span className="src-icon" aria-hidden="true">{it.icon}</span>}
                    <span className="label">{it.label}</span>
                    <span className="hint">{it.hint}</span>
                  </button>
                </li>
              );
            })}
          </ul>
        </div>
      )}
      <div className="palette-foot">
        {err ? <span className="red"><Glyph name="warning" /> {err}</span> : sources.length === 0 ? "No Sources yet. Choose “New Source…” to make your first one." : "↑↓ to choose · ↩ to run · ⇧↩ opens typed text in the Reader"}
      </div>
    </div>
  );
}
