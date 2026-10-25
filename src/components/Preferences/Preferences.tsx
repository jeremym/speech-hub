import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AboutPane, AccessibilityPane, CliPane, EnginePane, GeneralPane, HotkeysPane, PronunciationPane, SourcesPane, VoicesPane } from "./panes";

const PANES = [
  { id: "general", label: "General", group: "App", C: GeneralPane },
  { id: "accessibility", label: "Accessibility", group: "App", C: AccessibilityPane },
  { id: "engine", label: "Engine", group: "Speech", C: EnginePane },
  { id: "voices", label: "Voices", group: "Speech", C: VoicesPane },
  { id: "pronunciation", label: "Pronunciation", group: "Speech", C: PronunciationPane },
  { id: "hotkeys", label: "Shortcuts", group: "Ways to speak", C: HotkeysPane },
  { id: "sources", label: "Sources", group: "Ways to speak", C: SourcesPane },
  { id: "cli", label: "Command line", group: "Ways to speak", C: CliPane },
  { id: "about", label: "About", group: "", C: AboutPane },
] as const;
type PaneId = (typeof PANES)[number]["id"];

export default function Preferences() {
  const [pane, setPane] = useState<PaneId>("general");
  useEffect(() => {
    const un = listen<{ window: string; pane: string; prefill?: { text: string; context: string } }>("window:navigate", (e) => {
      if (e.payload.window !== "settings") return;
      if (e.payload.pane === "pronunciation" && e.payload.prefill) {
        setPane("pronunciation");
        const prefill = e.payload.prefill;
        setTimeout(() => window.dispatchEvent(new CustomEvent("speechhub:navigate", { detail: { kind: "pronunciation-prefill", ...prefill } })), 80);
        return;
      }
      if (e.payload.pane === "sources-new") {
        setPane("sources");
        setTimeout(() => window.dispatchEvent(new CustomEvent("speechhub:navigate", { detail: "sources-new" })), 50);
        return;
      }
      if (PANES.some((p) => p.id === e.payload.pane)) setPane(e.payload.pane as PaneId);
    });
    return () => { un.then((u) => u()); };
  }, []);
  const Active = PANES.find((p) => p.id === pane)!.C;
  return (
    <div className="flex h-full">
      <nav className="w-[180px] shrink-0 border-r border-[var(--border)] bg-[var(--panel-2)]/60 p-3 pt-4 flex flex-col gap-0.5" aria-label="Preferences sections">
        {PANES.map((p, i) => (
          <div key={p.id} className="contents">
            {p.group !== (PANES[i - 1]?.group ?? null) && (
              <div className={`px-3 pb-1 text-[11px] font-semibold uppercase tracking-wide text-[var(--muted)] ${i > 0 ? "mt-3" : ""}`}>{p.group}</div>
            )}
            <button onClick={() => setPane(p.id)} aria-current={pane === p.id ? "page" : undefined}
              className={`text-left rounded-lg px-3 py-1.5 text-[14px] ${pane === p.id ? "bg-[var(--accent)] text-white font-medium" : "hover:bg-[var(--panel-2)]"}`}>
              {p.label}
            </button>
          </div>
        ))}
      </nav>
      <main className="flex-1 min-w-0 overflow-auto p-6"><Active /></main>
    </div>
  );
}
