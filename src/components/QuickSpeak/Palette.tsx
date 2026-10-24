import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { hub, errorMessage } from "../../services/hub";

interface Item { id: string; label: string; hint: string; run: () => Promise<unknown> | void; keepOpen?: boolean }

/** Quick Speak palette. Milestone 1: built-in actions and typed text. Milestone 4 adds user Sources. */
export default function Palette() {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const [err, setErr] = useState<string | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const focus = () => { setQ(""); setSel(0); setErr(null); setTimeout(() => input.current?.focus(), 30); };
    focus();
    const un = listen<string>("window:shown", (e) => e.payload === "palette" && focus());
    return () => { un.then((u) => u()); };
  }, []);

  const close = () => hub.hideWindow("palette");
  const typed = q.trim();
  const items: Item[] = useMemo(() => {
    const base: Item[] = [
      { id: "selection", label: "Speak selected text", hint: "Reads what's highlighted in the app behind this window", run: () => hub.speakSelection() },
      { id: "reader", label: "Open Reader", hint: "Type or paste anything", run: () => hub.openWindow("reader") },
      { id: "sources", label: "Create a Source…", hint: "Arrives in Milestone 4", run: () => hub.openWindow("settings", "sources") },
    ];
    if (!typed) return base;
    return [
      { id: "speak-typed", label: `Speak “${typed.length > 40 ? typed.slice(0, 40) + "…" : typed}”`, hint: "Enter", run: () => hub.speak(typed, { title: "Quick Speak" }) },
      { id: "open-typed", label: "Open this text in the Reader", hint: "⇧Enter", run: () => hub.open(typed, "Quick Speak") },
      ...base.filter((b) => b.label.toLowerCase().includes(typed.toLowerCase())),
    ];
  }, [typed]);

  const run = async (item: Item) => {
    try { await item.run(); if (!item.keepOpen) close(); } catch (e) { setErr(errorMessage(e)); }
  };
  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); close(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); setSel((s) => Math.min(items.length - 1, s + 1)); }
    else if (e.key === "ArrowUp") { e.preventDefault(); setSel((s) => Math.max(0, s - 1)); }
    else if (e.key === "Enter") {
      e.preventDefault();
      if (e.shiftKey && typed) run(items.find((i) => i.id === "open-typed")!);
      else if (items[sel]) run(items[sel]);
    }
  };

  return (
    <div className="flex h-full flex-col rounded-2xl border border-[var(--border)] bg-[var(--panel)] shadow-2xl overflow-hidden" onKeyDown={onKey}>
      <div data-tauri-drag-region className="flex items-center gap-2 border-b border-[var(--border)] px-4 py-3">
        <span className="text-[var(--muted)]">›</span>
        <input ref={input} value={q} onChange={(e) => { setQ(e.target.value); setSel(0); }}
          placeholder="Type text to speak, or search sources…" className="flex-1 bg-transparent text-[17px] outline-none" />
        <kbd className="text-[11px] text-[var(--muted)] font-mono">esc</kbd>
      </div>
      <ul className="flex-1 overflow-auto p-2">
        {items.map((it, idx) => (
          <li key={it.id}>
            <button onMouseEnter={() => setSel(idx)} onClick={() => run(it)}
              className={`flex w-full items-center justify-between gap-3 rounded-lg px-3 py-2 text-left ${idx === sel ? "bg-[var(--accent)] text-white" : "hover:bg-[var(--panel-2)]"}`}>
              <span className="truncate font-medium">{it.label}</span>
              <span className={`shrink-0 text-[12px] ${idx === sel ? "text-white/80" : "text-[var(--muted)]"}`}>{it.hint}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="border-t border-[var(--border)] px-4 py-2 text-[12px] text-[var(--muted)]">
        {err ? <span className="text-[var(--danger)]">{err}</span> : "Your saved Sources will appear here once the Source Builder lands (Milestone 4)."}
      </div>
    </div>
  );
}
