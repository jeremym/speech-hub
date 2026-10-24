import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { hub, errorMessage } from "../../services/hub";
import type { PublicSource } from "../../types";

interface Item { id: string; icon: string; label: string; hint: string; run: () => Promise<unknown> | void; section: "actions" | "sources" | "typed" }

const TYPE_LABEL: Record<PublicSource["type"], string> = { text: "snippet", file: "file", command: "command", shortcut: "Shortcut", http: "URL" };

/** Quick Speak palette: typed text, built-in actions, and the user's Sources. Keyboard first. */
export default function Palette() {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const [sources, setSources] = useState<PublicSource[]>([]);
  const [confirm, setConfirm] = useState<PublicSource | null>(null);
  const input = useRef<HTMLInputElement>(null);

  const reload = () => hub.sources.listPublic().then(setSources).catch(() => {});
  useEffect(() => {
    const focus = () => { setQ(""); setSel(0); setErr(null); reload(); hub.sources.takePendingConfirm().then((c) => c && setConfirm(c)).catch(() => {}); setTimeout(() => input.current?.focus(), 30); };
    focus();
    const un1 = listen<string>("window:shown", (e) => e.payload === "palette" && focus());
    const un2 = listen("sources:changed", reload);
    const un3 = listen<PublicSource>("palette:confirm-source", (e) => setConfirm(e.payload));
    return () => { [un1, un2, un3].forEach((u) => u.then((f) => f())); };
  }, []);

  const close = () => { setConfirm(null); hub.hideWindow("palette"); };
  const typed = q.trim();
  const items: Item[] = useMemo(() => {
    const ql = typed.toLowerCase();
    const matches = (s: string) => !ql || s.toLowerCase().includes(ql);
    const actions: Item[] = [
      { id: "selection", icon: "🖱️", label: "Speak selected text", hint: "what's highlighted behind this window", run: () => hub.speakSelection(), section: "actions" as const },
      { id: "reader", icon: "📝", label: "Open Reader", hint: "type or paste anything", run: () => hub.openWindow("reader"), section: "actions" as const },
      { id: "new-source", icon: "＋", label: "New Source…", hint: "file, command, Shortcut, URL", run: () => hub.openWindow("settings", "sources-new"), section: "actions" as const },
    ].filter((a) => matches(a.label));
    const src: Item[] = sources.filter((s) => s.enabled && (matches(s.name) || matches(s.description))).map((s) => ({
      id: s.id, icon: s.icon, label: s.name, section: "sources",
      hint: `${s.trusted || (s.type !== "command" && s.type !== "shortcut") ? "" : "needs approval · "}${TYPE_LABEL[s.type]} · ${s.action === "open" ? "opens Reader" : "speaks"}`,
      run: () => hub.sources.run(s.id),
    }));
    const typedItems: Item[] = typed ? [
      { id: "speak-typed", icon: "🔊", label: `Speak “${typed.length > 48 ? typed.slice(0, 48) + "…" : typed}”`, hint: "↩", run: () => hub.speak(typed, { title: "Quick Speak" }), section: "typed" as const },
      { id: "open-typed", icon: "📝", label: "Open this text in the Reader", hint: "⇧↩", run: () => hub.open(typed, "Quick Speak"), section: "typed" as const },
    ] : [];
    // Sources first when searching (that's what people type), actions first when idle.
    return typed ? [...src, ...typedItems, ...actions] : [...actions, ...src];
  }, [typed, sources]);

  const run = async (item: Item) => {
    try { await item.run(); close(); } catch (e) { setErr(errorMessage(e)); }
  };
  const onKey = (e: React.KeyboardEvent) => {
    if (confirm) {
      if (e.key === "Escape") { e.preventDefault(); setConfirm(null); }
      if (e.key === "Enter") { e.preventDefault(); const c = confirm; setConfirm(null); hub.sources.run(c.id).then(close).catch((er) => setErr(errorMessage(er))); }
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

  let lastSection: Item["section"] | null = null;
  return (
    <div className="flex h-full flex-col rounded-2xl border border-[var(--border)] bg-[var(--panel)] shadow-2xl overflow-hidden" onKeyDown={onKey}>
      <div data-tauri-drag-region className="flex items-center gap-2 border-b border-[var(--border)] px-4 py-3">
        <span className="text-[var(--muted)]">›</span>
        <input ref={input} value={q} onChange={(e) => { setQ(e.target.value); setSel(0); }} disabled={!!confirm}
          placeholder="Search Sources, or type text to speak…" className="flex-1 bg-transparent text-[17px] outline-none" />
        <kbd className="text-[11px] text-[var(--muted)] font-mono">esc</kbd>
      </div>
      {confirm ? (
        <div className="flex-1 p-5">
          <div className="text-[15px] font-semibold">A link wants to run “{confirm.icon} {confirm.name}”</div>
          <p className="text-[13px] text-[var(--muted)] mt-1">{confirm.summary}. {confirm.type === "command" || confirm.type === "shortcut" ? "This runs a program on your Mac." : ""} Only continue if you expected this.</p>
          {!confirm.trusted && (confirm.type === "command" || confirm.type === "shortcut") && <p className="text-[13px] text-[var(--danger)] mt-2">This Source has not been approved. Approve it in Preferences → Sources first.</p>}
          <div className="mt-4 flex gap-2">
            <button onClick={() => { const c = confirm; setConfirm(null); hub.sources.run(c.id).then(close).catch((er) => setErr(errorMessage(er))); }} className="rounded-lg bg-[var(--accent)] px-3 py-1.5 text-white font-medium">Run (↩)</button>
            <button onClick={() => setConfirm(null)} className="rounded-lg bg-[var(--panel-2)] px-3 py-1.5">Cancel (esc)</button>
          </div>
        </div>
      ) : (
        <ul className="flex-1 overflow-auto p-2">
          {items.length === 0 && <li className="px-3 py-6 text-center text-[13px] text-[var(--muted)]">Nothing matches.</li>}
          {items.map((it, idx) => {
            const header = it.section !== lastSection && !typed ? it.section : null;
            lastSection = it.section;
            return (
              <li key={it.id}>
                {header === "sources" && sources.length > 0 && <div className="px-3 pt-2 pb-1 text-[11px] uppercase tracking-wide text-[var(--muted)]">Your Sources</div>}
                <button onMouseEnter={() => setSel(idx)} onClick={() => run(it)}
                  className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-left ${idx === sel ? "bg-[var(--accent)] text-white" : "hover:bg-[var(--panel-2)]"}`}>
                  <span className="w-6 text-center">{it.icon}</span>
                  <span className="flex-1 truncate font-medium">{it.label}</span>
                  <span className={`shrink-0 text-[12px] ${idx === sel ? "text-white/80" : "text-[var(--muted)]"}`}>{it.hint}</span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <div className="border-t border-[var(--border)] px-4 py-2 text-[12px] text-[var(--muted)]">
        {err ? <span className="text-[var(--danger)]">{err}</span> : sources.length === 0 ? "No Sources yet. Choose “New Source…” to make your first one." : "↑↓ to choose · ↩ to run · ⇧↩ opens typed text in the Reader"}
      </div>
    </div>
  );
}
