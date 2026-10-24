import { useEffect, useRef, useState } from "react";
import { hub, useHubStatus, useSettings, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import { eventToShortcut, formatShortcut } from "../../lib/shortcut";
import type { CliStatus, HotkeyStatus, ProfileCard, Settings, SystemInfo } from "../../types";
import { Badge, Button, Card, Dots, Field, H1, Lead, Select, Toggle } from "../ui";

type PaneProps = { wizard?: boolean };

// ---------------------------------------------------------------- Accessibility

export function AccessibilityPane({ wizard }: PaneProps) {
  const [trusted, setTrusted] = useState<boolean | null>(null);
  useEffect(() => {
    let alive = true;
    const check = () => hub.axIsTrusted().then((t) => alive && setTrusted(t)).catch(() => {});
    check();
    const id = setInterval(check, 1500);
    return () => { alive = false; clearInterval(id); };
  }, []);
  return (
    <div>
      <H1>Accessibility permission</H1>
      <Lead>Speech Hub reads the text you've highlighted in other apps through macOS Accessibility. It never activates a window, never touches the clipboard, and never sends text anywhere.</Lead>
      <Card>
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="font-medium flex items-center gap-2">
              Status {trusted === null ? <Badge>Checking…</Badge> : trusted ? <Badge tone="ok">Granted</Badge> : <Badge tone="warn">Not granted</Badge>}
            </div>
            <div className="text-[13px] text-[var(--muted)] mt-1">
              {trusted ? "Option+Escape will read selected text." : "System Settings → Privacy & Security → Accessibility → enable Speech Hub."}
            </div>
          </div>
          <div className="flex gap-2">
            {!trusted && <Button variant="primary" onClick={() => hub.axRequest()}>Grant access…</Button>}
            <Button onClick={() => hub.openSystemSettings("accessibility")}>Open System Settings</Button>
          </div>
        </div>
      </Card>
      {wizard && !trusted && <p className="text-[13px] text-[var(--muted)] mt-4">You can continue without it, but the main shortcut won't work until it's granted.</p>}
      <p className="text-[13px] text-[var(--muted)] mt-4">Some apps don't expose their selection to Accessibility. In Milestone 4 you'll be able to pick a fallback Source (for example the clipboard via <code>pbpaste</code>) to run in that case.</p>
    </div>
  );
}

// ---------------------------------------------------------------- Engine (Handy-style model picker)

export function EnginePane({ wizard }: PaneProps) {
  const { settings, save } = useSettings();
  const status = useHubStatus();
  const [cards, setCards] = useState<ProfileCard[]>([]);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  useEffect(() => {
    hub.engines().then(setCards).catch(() => {});
    hub.systemInfo().then(setInfo).catch(() => {});
  }, []);
  const active = status?.engine.id;
  const selected = settings?.engineProfile;
  return (
    <div>
      <H1>Speech engine</H1>
      <Lead>
        Everything runs on this Mac. Pick the engine that fits your machine; you can switch any time.
        {info && <> This Mac: <strong>{info.arch === "aarch64" ? "Apple Silicon" : info.arch}</strong>, <strong>{Math.round(info.memoryGb)} GB</strong> memory.</>}
      </Lead>
      <div className="grid gap-3">
        {cards.map((c) => {
          const isSelected = selected === c.id;
          const isActive = active === c.id && status?.engine.loaded;
          return (
            <Card key={c.id} selected={isSelected} disabled={!c.installed} onClick={c.installed ? () => save({ engineProfile: c.id, voiceId: "" }) : undefined}>
              <div className="flex items-start justify-between gap-4">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-semibold text-[16px]">{c.name}</span>
                    {c.recommended && <Badge tone="accent">Recommended</Badge>}
                    {c.devOnly && <Badge tone="warn">Dev only</Badge>}
                    {c.installed ? (c.bundled ? <Badge>Bundled</Badge> : <Badge tone="ok">Installed</Badge>) : c.bundled ? <Badge>Arrives in Milestone 2</Badge> : <Badge>Download · Milestone 6</Badge>}
                    {isActive && <Badge tone="ok">Active</Badge>}
                    {c.voiceCloning && <Badge tone="accent">Voice cloning</Badge>}
                  </div>
                  <p className="text-[13px] text-[var(--muted)] mt-1">{c.tagline}</p>
                  <div className="flex gap-5 mt-2 text-[12px] text-[var(--muted)]">
                    <span>Quality <Dots value={c.quality} /></span>
                    <span>Speed <Dots value={c.speed} /></span>
                    <span>{c.sizeLabel}</span>
                    <span>RAM {c.ramLabel}</span>
                    <span>{c.languages.join(", ")}</span>
                    <span>{c.license}</span>
                  </div>
                </div>
                <input type="radio" readOnly checked={isSelected} disabled={!c.installed} className="mt-1 accent-[var(--accent)]" aria-label={`Use ${c.name}`} />
              </div>
            </Card>
          );
        })}
      </div>
      {status && <p className="text-[13px] text-[var(--muted)] mt-4">Engine state: {STATE_LABEL[status.state]}{status.error ? ` · ${status.error}` : ""}</p>}
      {wizard && <p className="text-[13px] text-[var(--muted)] mt-1">Lite and Standard are bundled with the app and work offline. Nothing is downloaded unless you ask for Plus.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- Voices

export function VoicesPane(_: PaneProps) {
  const { settings, save } = useSettings();
  const status = useHubStatus();
  const voices = useVoices(status?.engine.id);
  const [filter, setFilter] = useState("");
  const current = settings?.voiceId || status?.voice_id || "";
  const shown = voices.filter((v) => `${v.name} ${v.language}`.toLowerCase().includes(filter.toLowerCase()));
  return (
    <div className="flex flex-col h-full min-h-0">
      <H1>Voices</H1>
      <Lead>Pick a default voice. Every app, script, and Source uses it unless they ask for another.</Lead>
      <div className="flex items-center gap-3 mb-3">
        <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search voices…" className="flex-1 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5" />
        <label className="flex items-center gap-2 text-[13px] text-[var(--muted)]">
          Speed <input type="range" min={0.5} max={2.5} step={0.05} value={settings?.speed ?? 1} onChange={(e) => save({ speed: Number(e.target.value) })} className="accent-[var(--accent)]" />
          <span className="w-10 tabular-nums">{(settings?.speed ?? 1).toFixed(2)}×</span>
        </label>
      </div>
      <div className="flex-1 min-h-0 overflow-auto rounded-xl border border-[var(--border)] bg-[var(--panel)] divide-y divide-[var(--border)]">
        {shown.length === 0 && <div className="p-4 text-[var(--muted)] text-[13px]">{voices.length === 0 ? "No voices: the engine is still loading or isn't installed." : "No matches."}</div>}
        {shown.map((v) => (
          <div key={v.id} className={`flex items-center justify-between gap-3 px-3 py-2 ${v.id === current ? "bg-[var(--accent)]/10" : ""}`}>
            <button className="flex-1 text-left flex items-center gap-3" onClick={() => save({ voiceId: v.id })}>
              <span className={`h-2 w-2 rounded-full ${v.id === current ? "bg-[var(--accent)]" : "bg-transparent"}`} />
              <span className="font-medium">{v.name}</span>
              <span className="text-[12px] text-[var(--muted)]">{v.language}</span>
              {v.custom && <Badge tone="accent">Custom</Badge>}
            </button>
            <Button variant="ghost" onClick={() => hub.previewVoice(v.id).catch(() => {})}>▶ Preview</Button>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- Hotkeys

function ShortcutRecorder({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [recording, setRecording] = useState(false);
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape" && !e.altKey && !e.shiftKey && !e.ctrlKey && !e.metaKey) { setRecording(false); return; }
      const sc = eventToShortcut(e);
      if (sc) { onChange(sc); setRecording(false); }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [recording, onChange]);
  return (
    <button ref={ref} type="button" onClick={() => setRecording((r) => !r)}
      className={`min-w-[120px] rounded-lg border px-3 py-1.5 font-mono text-[15px] ${recording ? "border-[var(--accent)] animate-pulse" : "border-[var(--border)] bg-[var(--panel)]"}`}>
      {recording ? "Press keys…" : formatShortcut(value)}
    </button>
  );
}

export function HotkeysPane({ wizard }: PaneProps) {
  const { settings, save } = useSettings();
  const [statuses, setStatuses] = useState<HotkeyStatus[]>([]);
  const refresh = () => hub.hotkeysApply().then(setStatuses).catch(() => {});
  useEffect(() => { refresh(); }, [settings?.hotkeys.speakSelection, settings?.hotkeys.quickSpeak]);
  if (!settings) return null;
  const st = (id: string) => statuses.find((s) => s.id === id);
  const setHotkey = (k: keyof Settings["hotkeys"]) => (v: string) => save({ hotkeys: { ...settings.hotkeys, [k]: v } });
  return (
    <div>
      <H1>Keyboard shortcuts</H1>
      <Lead>Two shortcuts do everything. Click one to change it, then press the new keys.</Lead>
      <Card>
        <Field label="Speak selected text" hint={<>Highlight text in any app and press this. {st("speak_selection")?.error && <span className="text-[var(--danger)]">Not registered: {st("speak_selection")!.error}</span>}</>}>
          <ShortcutRecorder value={settings.hotkeys.speakSelection} onChange={setHotkey("speakSelection")} />
        </Field>
        <Field label="Quick Speak" hint={<>Opens the palette, the Reader, or runs a Source. {st("quick_speak")?.error && <span className="text-[var(--danger)]">Not registered: {st("quick_speak")!.error}</span>}</>}>
          <ShortcutRecorder value={settings.hotkeys.quickSpeak} onChange={setHotkey("quickSpeak")} />
        </Field>
        <Field label="Pressing again while speaking" hint="What the first shortcut does if something is already playing.">
          <Select value={settings.repeatShortcut} onChange={(v) => save({ repeatShortcut: v })} options={[{ value: "replace", label: "Read the new selection" }, { value: "stop", label: "Stop" }]} />
        </Field>
        <Field label="Quick Speak opens" hint="Running a specific Source arrives with the Source Builder (Milestone 4).">
          <Select value={settings.quickSpeakAction} onChange={(v) => save({ quickSpeakAction: v })} options={[{ value: "palette", label: "Source palette" }, { value: "reader", label: "Blank Reader" }, { value: "source", label: "A specific Source (soon)" }]} />
        </Field>
      </Card>
      <Card className="mt-4">
        <div className="font-medium">Already using Option+Escape for Apple's Speak Selection?</div>
        <p className="text-[13px] text-[var(--muted)] mt-1">macOS may grab it first. Turn off Apple's shortcut in System Settings → Accessibility → Spoken Content → Speak selection, or pick a different shortcut above.</p>
        <div className="mt-2"><Button onClick={() => hub.openSystemSettings("spoken_content")}>Open Spoken Content settings</Button></div>
      </Card>
      {wizard && <p className="text-[13px] text-[var(--muted)] mt-4">Defaults: ⌥⎋ speaks the selection, ⌥⇧⎋ opens Quick Speak.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- General

export function GeneralPane(_: PaneProps) {
  const { settings, save } = useSettings();
  if (!settings) return null;
  return (
    <div>
      <H1>General</H1>
      <Lead>Speech Hub lives in the menu bar. Close any window and it keeps running.</Lead>
      <Card>
        <Field label="Launch at login" hint="Registers a Login Item so the shortcuts are always available.">
          <Toggle checked={settings.launchAtLogin} onChange={(v) => save({ launchAtLogin: v })} />
        </Field>
        <Field label="Show the mini player" hint="A small floating controller appears while speaking when the Reader isn't open.">
          <Toggle checked={settings.showMiniPlayer} onChange={(v) => save({ showMiniPlayer: v })} />
        </Field>
        <Field label="Unload the model when idle" hint="Frees memory on smaller Macs. The next request reloads it (a short delay). 0 = keep loaded.">
          <input type="number" min={0} max={240} value={settings.idleUnloadMinutes} onChange={(e) => save({ idleUnloadMinutes: Math.max(0, Number(e.target.value) || 0) })} className="w-20 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2 py-1 text-right" />
          <span className="text-[13px] text-[var(--muted)]">min</span>
        </Field>
      </Card>
    </div>
  );
}

// ---------------------------------------------------------------- Command line

export function CliPane({ wizard }: PaneProps) {
  const [st, setSt] = useState<CliStatus | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const refresh = () => hub.cliStatus().then(setSt).catch((e) => setErr(errorMessage(e)));
  useEffect(() => { refresh(); }, []);
  const act = (p: Promise<CliStatus>) => { setErr(null); p.then(setSt).catch((e) => setErr(errorMessage(e))); };
  return (
    <div>
      <H1>Command line tool</H1>
      <Lead>Any script or app can speak through Speech Hub with the <code>speak</code> command. It talks to this running app over a private socket, so nothing else loads a model.</Lead>
      <Card>
        <pre className="text-[13px] leading-6 overflow-auto select-text">{`echo "Build finished" | speak      # speak now
speak -f notes.md                   # speak a file
pbpaste | speak --open              # open in the Reader
speak -v bm_george "Cheers"         # pick a voice
speak -o out.wav "Render me"        # write a WAV, no playback
speak run morning-briefing          # run a saved Source
speak stop`}</pre>
      </Card>
      <Card className="mt-4">
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <div className="font-medium flex items-center gap-2">
              Status {st === null ? <Badge>Checking…</Badge> : st.installed ? <Badge tone="ok">Installed</Badge> : <Badge>Not installed</Badge>}
              {st?.installed && !st.inPath && <Badge tone="warn">Not on PATH</Badge>}
            </div>
            <div className="text-[13px] text-[var(--muted)] mt-1 break-all select-text">
              {st?.installed ? <>Linked at <code>{st.link}</code></> : <>Installs a link at <code>{st?.link ?? "~/.local/bin/speak"}</code>. No password needed.</>}
            </div>
          </div>
          <div className="flex gap-2 shrink-0">
            {st?.installed
              ? <Button onClick={() => act(hub.cliUninstall())}>Remove</Button>
              : <Button variant="primary" disabled={!st?.bundled} onClick={() => act(hub.cliInstall())}>Install</Button>}
          </div>
        </div>
        {st && !st.bundled && <p className="text-[13px] text-[var(--danger)] mt-2">This build has no bundled <code>speak</code> binary. Run <code>scripts/build-cli.sh</code>.</p>}
        {st?.installed && !st.inPath && (
          <p className="text-[13px] text-[var(--muted)] mt-3">Your shell doesn't look in <code>~/.local/bin</code> yet. Add this to <code>~/.zshrc</code>, then open a new terminal:
            <pre className="mt-1 select-text">export PATH="$HOME/.local/bin:$PATH"</pre></p>
        )}
        {err && <p className="text-[13px] text-[var(--danger)] mt-2">{err}</p>}
      </Card>
      <p className="text-[13px] text-[var(--muted)] mt-4">Developers: the same socket protocol is documented in <code>docs/public-api.md</code>. Links like <code>speechhub://palette</code> also work.</p>
      {wizard && <p className="text-[13px] text-[var(--muted)] mt-1">Optional. The app works without it.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- Placeholders (M4, M5)

export function SourcesPane(_: PaneProps) {
  return (
    <div>
      <H1>Sources</H1>
      <Lead>A Source tells Speech Hub where to get text: a file, a command's output, an Apple Shortcut, or a URL. Build them here without writing code.</Lead>
      <Card><p className="text-[var(--muted)]">The Source Builder arrives in Milestone 4. Until then, use the Reader or wait for the <code>speak</code> CLI (Milestone 3).</p></Card>
    </div>
  );
}

export function PronunciationPane(_: PaneProps) {
  return (
    <div>
      <H1>Pronunciation Studio</H1>
      <Lead>Teach Speech Hub how to say a name or a technical term once; the fix applies everywhere.</Lead>
      <Card><p className="text-[var(--muted)]">Arrives in Milestone 5.</p></Card>
    </div>
  );
}

export function AboutPane(_: PaneProps) {
  const [info, setInfo] = useState<SystemInfo | null>(null);
  useEffect(() => { hub.systemInfo().then(setInfo).catch(() => {}); }, []);
  return (
    <div>
      <H1>About Speech Hub</H1>
      <Lead>Give anything on your Mac a voice. Local, private, open source (GPL-3.0).</Lead>
      <Card>
        <dl className="grid grid-cols-[140px_1fr] gap-y-2 text-[14px]">
          <dt className="text-[var(--muted)]">Version</dt><dd className="select-text">{info?.appVersion ?? "…"}</dd>
          <dt className="text-[var(--muted)]">macOS</dt><dd>{info?.macos ?? "…"}</dd>
          <dt className="text-[var(--muted)]">Settings folder</dt><dd className="select-text break-all text-[13px]">{info?.dataDir ?? "…"}</dd>
        </dl>
      </Card>
    </div>
  );
}

export function errorText(e: unknown) { return errorMessage(e); }
