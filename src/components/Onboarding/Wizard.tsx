import { useEffect, useState } from "react";
import { Progress } from "@overtone-foundry/app-kit";
import { hub } from "../../services/hub";
import { Logo } from "../../brand";
import { AccessibilityPane, CliPane, EnginePane, HotkeysPane, VoicesPane } from "../Preferences/panes";
import { Head } from "../bits";

function Welcome() {
  return (
    <div>
      <div className="row" style={{ marginBottom: 12 }}><Logo size={40} /></div>
      <Head title="Welcome">Give anything on your Mac a voice.</Head>
      <ul className="rows">
        <li><strong>Highlight text anywhere, press <kbd>⌥⎋</kbd>,</strong> and hear it read by a natural AI voice. That shortcut is called <strong>Speak Selection</strong>.</li>
        <li><strong>Everything stays on this Mac.</strong> No account, no cloud, no telemetry.</li>
        <li><strong>Any app can speak through it.</strong> Scripts, Shortcuts, your clipboard manager, your own tools.</li>
        <li><strong>Build your own Sources</strong>: saved places to get text from, like your clipboard, a file, a command, or a web address. No code needed.</li>
      </ul>
      <p className="dim" style={{ marginTop: 14 }}>This short setup takes about a minute. Every step is also in Preferences later.</p>
    </div>
  );
}

function Done() {
  return (
    <div>
      <Head title="You're set">Speech Hub is running in your menu bar.</Head>
      <ul className="rows">
        <li>Highlight some text in any app and press <kbd>⌥⎋</kbd> (Speak Selection).</li>
        <li>Press <kbd>⌥⇧⎋</kbd> for Quick Speak: type text, run a Source, or open the Reader.</li>
        <li>Click the waveform icon in the menu bar for everything else.</li>
      </ul>
    </div>
  );
}

const STEPS = [
  { id: "welcome", title: "Welcome", C: Welcome },
  { id: "accessibility", title: "Permission", C: AccessibilityPane },
  { id: "engine", title: "Engine", C: EnginePane },
  { id: "voices", title: "Voice", C: VoicesPane },
  { id: "hotkeys", title: "Shortcuts", C: HotkeysPane },
  { id: "cli", title: "Command line", C: CliPane },
  { id: "done", title: "Done", C: Done },
];

export default function Wizard() {
  const [i, setI] = useState(0);
  const step = STEPS[i];
  const last = i === STEPS.length - 1;
  // Pre-select the recommended engine once, so the picker shows a sensible default.
  useEffect(() => {
    Promise.all([hub.getSettings(), hub.systemInfo(), hub.engines()]).then(([s, info, cards]) => {
      const rec = cards.find((c) => c.id === info.recommendedProfile && c.installed) ?? cards.find((c) => c.installed);
      if (rec && !cards.some((c) => c.id === s.engineProfile && c.installed)) hub.updateSettings({ ...s, engineProfile: rec.id, voiceId: "" });
    }).catch(() => {});
  }, []);
  const finish = () => hub.completeOnboarding().catch(() => {});
  return (
    <div className="win">
      <div className="wizard-head">
        <Progress value={(i + 1) / STEPS.length} width={STEPS.length} label="Setup progress">step {i + 1} of {STEPS.length} · {step.title}</Progress>
      </div>
      <main className="scroll shell pane"><step.C wizard /></main>
      <footer className="wizard-foot">
        <span className="step">{last ? "Every step is also in Preferences." : `Next: ${STEPS[i + 1].title}`}</span>
        {i > 0 && !last && <button className="quiet" onClick={() => setI(i - 1)}>Back</button>}
        {!last && i > 0 && <button className="quiet" onClick={finish}>Skip setup</button>}
        {last ? <button className="primary" onClick={finish}>Start using Speech Hub</button> : <button className="primary" onClick={() => setI(i + 1)}>Continue</button>}
      </footer>
    </div>
  );
}
