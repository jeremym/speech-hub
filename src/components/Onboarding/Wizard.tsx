import { useEffect, useState } from "react";
import { hub } from "../../services/hub";
import { AccessibilityPane, CliPane, EnginePane, HotkeysPane, VoicesPane } from "../Preferences/panes";
import { Button, H1, Lead } from "../ui";

function Welcome() {
  return (
    <div>
      <H1>Welcome to Speech Hub</H1>
      <Lead>Give anything on your Mac a voice.</Lead>
      <ul className="space-y-3 text-[15px]">
        <li><strong>Select text anywhere, press ⌥⎋,</strong> and hear it read by a natural AI voice.</li>
        <li><strong>Everything stays on this Mac.</strong> No account, no cloud, no telemetry.</li>
        <li><strong>Any app can speak through it.</strong> Scripts, Shortcuts, your clipboard manager, your own tools.</li>
        <li><strong>Build your own Sources</strong> that pull text from files, commands, or URLs. No code needed.</li>
      </ul>
      <p className="text-[13px] text-[var(--muted)] mt-6">This short setup takes about a minute. Every step is also in Preferences later.</p>
    </div>
  );
}

function Done() {
  return (
    <div>
      <H1>You're set</H1>
      <Lead>Speech Hub is running in your menu bar.</Lead>
      <ul className="space-y-3 text-[15px]">
        <li>Highlight some text in any app and press <kbd className="font-mono">⌥⎋</kbd>.</li>
        <li>Press <kbd className="font-mono">⌥⇧⎋</kbd> for Quick Speak.</li>
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
    <div className="flex h-full flex-col">
      <ol className="flex gap-1 px-6 pt-4">
        {STEPS.map((s, idx) => (
          <li key={s.id} className={`h-1.5 flex-1 rounded-full ${idx <= i ? "bg-[var(--accent)]" : "bg-[var(--border)]"}`} aria-label={s.title} />
        ))}
      </ol>
      <main className="flex-1 min-h-0 overflow-auto p-6"><step.C wizard /></main>
      <footer className="flex items-center justify-between border-t border-[var(--border)] px-6 py-3">
        <span className="text-[13px] text-[var(--muted)]">Step {i + 1} of {STEPS.length} · {step.title}</span>
        <div className="flex gap-2">
          {i > 0 && !last && <Button onClick={() => setI(i - 1)}>Back</Button>}
          {!last && i > 0 && <Button variant="ghost" onClick={finish}>Skip setup</Button>}
          {last ? <Button variant="primary" onClick={finish}>Start using Speech Hub</Button> : <Button variant="primary" onClick={() => setI(i + 1)}>Continue</Button>}
        </div>
      </footer>
    </div>
  );
}
