import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { WindowLabel } from "./types";
import Wizard from "./components/Onboarding/Wizard";
import Preferences from "./components/Preferences/Preferences";
import Reader from "./components/Reader/Reader";
import MiniPlayer from "./components/Player/MiniPlayer";
import Palette from "./components/QuickSpeak/Palette";

/** One React bundle, several windows: the window label picks the screen. */
export default function App() {
  const [label, setLabel] = useState<WindowLabel | null>(null);
  useEffect(() => {
    try { setLabel(getCurrentWindow().label as WindowLabel); } catch { setLabel("settings"); }
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
