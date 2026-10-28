import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Glyph, type GlyphName } from "@overtone-foundry/app-kit";
import { Logo } from "../../brand";
import { AboutPane, AccessibilityPane, CliPane, EnginePane, GeneralPane, HotkeysPane, PronunciationPane, SourcesPane, VoicesPane } from "./panes";

const PANES = [
  { id: "general", label: "General", group: "App", glyph: "settings", C: GeneralPane },
  { id: "accessibility", label: "Accessibility", group: "App", glyph: "shield", C: AccessibilityPane },
  { id: "engine", label: "Engine", group: "Speech", glyph: "models", C: EnginePane },
  { id: "voices", label: "Voices", group: "Speech", glyph: "audio", C: VoicesPane },
  { id: "pronunciation", label: "Pronunciation", group: "Speech", glyph: "text", C: PronunciationPane },
  { id: "hotkeys", label: "Shortcuts", group: "Ways to speak", glyph: "key", C: HotkeysPane },
  { id: "sources", label: "Sources", group: "Ways to speak", glyph: "in", C: SourcesPane },
  { id: "cli", label: "Command line", group: "Ways to speak", glyph: "code", C: CliPane },
  { id: "about", label: "About", group: "", glyph: "help", C: AboutPane },
] as const satisfies readonly { id: string; label: string; group: string; glyph: GlyphName; C: (props: { wizard?: boolean }) => React.ReactNode }[];
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
    <div className="settings-shell win-settings">
      <nav className="settings-nav" aria-label="Preferences sections">
        <div className="settings-head"><Logo size={26} word={false} /><h1>Preferences</h1></div>
        {PANES.map((p, i) => (
          <div key={p.id} style={{ display: "contents" }}>
            {p.group !== (PANES[i - 1]?.group ?? null) && p.group && <div className="nav-group">{p.group}</div>}
            <button onClick={() => setPane(p.id)} aria-current={pane === p.id ? "true" : undefined}>
              <Glyph name={p.glyph} />{p.label}
            </button>
          </div>
        ))}
      </nav>
      <main className="pane"><Active /></main>
    </div>
  );
}
