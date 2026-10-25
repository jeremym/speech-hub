import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowLabel } from "./types";
import Wizard from "./components/Onboarding/Wizard";
import Preferences from "./components/Preferences/Preferences";
import Reader from "./components/Reader/Reader";
import MiniPlayer from "./components/Player/MiniPlayer";
import Palette from "./components/QuickSpeak/Palette";
import { hub } from "./services/hub";

/** One React bundle, several windows: the window label picks the screen. */
export default function App() {
  const [label, setLabel] = useState<WindowLabel | null>(null);
  useEffect(() => {
    let l: WindowLabel = "settings";
    try { l = getCurrentWindow().label as WindowLabel; } catch { /* dev server outside Tauri */ }
    setLabel(l);
    // Preferences get a larger type size; the other windows stay compact.
    document.body.style.zoom = l === "settings" ? "1.15" : "";
    // ⌘, opens Preferences from any of our windows, like every Mac app.
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && !e.altKey && !e.ctrlKey && e.key === ",") {
        e.preventDefault();
        hub.openWindow("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  switch (label) {
    case "onboarding": return <Wizard />;
    case "settings": return <Preferences />;
    case "reader": return <Reader />;
    case "player": return <MiniPlayer />;
    case "palette": return <Palette />;
    default: return null;
  }
}
