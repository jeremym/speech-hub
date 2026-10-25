import { useEffect, useRef, useState } from "react";
import { hub, useDownloadProgress, useHubStatus, useSettings, useSources, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import VoiceRecorder from "./VoiceRecorder";
import SourceBuilder from "../SourceBuilder/SourceBuilder";
import PronunciationStudio from "../PronunciationStudio/PronunciationStudio";
import { eventToShortcut, formatShortcut } from "../../lib/shortcut";
import type { CliStatus, HotkeyStatus, ProfileCard, Settings, SystemInfo, SystemSpeech } from "../../types";
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
      <FallbackSourceCard />
    </div>
  );
}

function FallbackSourceCard() {
  const { settings, save } = useSettings();
  if (!settings) return null;
  return (
    <Card className="mt-4">
      <Field label="When the selection can't be read" hint="Some apps hide their selection from Accessibility. Run a Source instead, e.g. “Current clipboard” after you press ⌘C.">
        <SourcePicker value={settings.fallbackSourceId} onChange={(v) => save({ fallbackSourceId: v })} none="Just tell me" />
      </Field>
    </Card>
  );
}

// ---------------------------------------------------------------- Engine (Handy-style model picker)

export function EnginePane({ wizard }: PaneProps) {
  const { settings, save } = useSettings();
  const status = useHubStatus();
  const [cards, setCards] = useState<ProfileCard[]>([]);
  const [info, setInfo] = useState<SystemInfo | null>(null);
  const [consent, setConsent] = useState(false);
  const [dlErr, setDlErr] = useState<string | null>(null);
  const progress = useDownloadProgress();
  const reloadCards = () => hub.engines().then(setCards).catch(() => {});
  useEffect(() => {
    reloadCards();
    hub.systemInfo().then(setInfo).catch(() => {});
  }, []);
  useEffect(() => {
    if (progress === null) reloadCards();
    if (progress?.phase === "error") setDlErr(progress.message);
  }, [progress]);
  const startDownload = () => { setDlErr(null); hub.plus.download().catch((e) => setDlErr(errorMessage(e))); };
  const removePlus = () => hub.plus.remove().then(reloadCards).catch((e) => setDlErr(errorMessage(e)));
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
              {c.id === "plus" && !c.installed && (
                <div className="mt-3 border-t border-[var(--border)] pt-3" onClick={(e) => e.stopPropagation()}>
                  {progress && progress.profile === "plus" && progress.phase !== "error" ? (
                    <div>
                      <div className="flex justify-between text-[12px] text-[var(--muted)]"><span>{progress.phase === "downloading" ? "Downloading…" : progress.phase === "verifying" ? "Verifying checksum…" : "Installing…"}</span><span>{progress.total ? `${Math.round((progress.bytes / progress.total) * 100)}%` : ""}</span></div>
                      <div className="mt-1 h-1.5 w-full rounded-full bg-[var(--border)]"><div className="h-1.5 rounded-full bg-[var(--accent)] transition-all" style={{ width: `${progress.total ? Math.min(100, (progress.bytes / progress.total) * 100) : 5}%` }} /></div>
                    </div>
                  ) : (
                    <div className="text-[13px]">
                      <p className="text-[var(--muted)]">Pocket TTS is made by Kyutai and licensed <strong>CC BY 4.0</strong>. Its model card prohibits voice impersonation or cloning without the speaker's explicit, lawful consent. Downloaded once from GitHub (sherpa-onnx releases) and verified by checksum; nothing else ever leaves this Mac.</p>
                      <label className="mt-2 flex items-start gap-2"><input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} className="mt-0.5 accent-[var(--accent)]" /><span>I'll only clone voices I have the right to use, and I accept the CC BY 4.0 terms.</span></label>
                      <div className="mt-2 flex items-center gap-3"><Button variant="primary" disabled={!consent} onClick={startDownload}>Download Plus (98 MB)</Button>{dlErr && <span className="text-[var(--danger)]">{dlErr}</span>}</div>
                    </div>
                  )}
                </div>
              )}
              {c.id === "plus" && c.installed && (
                <div className="mt-3 border-t border-[var(--border)] pt-2 flex items-center justify-between text-[13px]" onClick={(e) => e.stopPropagation()}>
                  <span className="text-[var(--muted)]">Add your own voices under Voices while Plus is selected.</span>
                  <Button variant="ghost" onClick={removePlus}>Remove download</Button>
                </div>
              )}
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
  const [adding, setAdding] = useState<{ path: string; wav?: Uint8Array; seconds?: number; name: string; consent: boolean } | null>(null);
  const [recording, setRecording] = useState(false);
  const [msg, setMsg] = useState<string | null>(null);
  const current = settings?.voiceId || status?.voice_id || "";
  const shown = voices.filter((v) => `${v.name} ${v.language}`.toLowerCase().includes(filter.toLowerCase()));
  const isPlus = status?.engine.id === "plus" && status.engine.loaded;
  const pickRecording = async () => {
    const p = await pickFile({ multiple: false, filters: [{ name: "Audio", extensions: ["wav"] }] });
    if (p && !Array.isArray(p)) setAdding({ path: p, name: "", consent: false });
  };
  const addVoice = async () => {
    if (!adding) return;
    try {
      if (adding.wav) await hub.customVoices.addRecording(adding.wav, adding.name, adding.consent);
      else await hub.customVoices.add(adding.path, adding.name, adding.consent);
      setAdding(null); setMsg("Voice added.");
    } catch (e) { setMsg(errorMessage(e)); }
  };
  const deleteVoice = (id: string) => hub.customVoices.delete(id).catch((e) => setMsg(errorMessage(e)));
  return (
    <div className="flex flex-col h-full min-h-0">
      <div className="flex items-start justify-between gap-4">
        <div><H1>Voices</H1><Lead>Pick a default voice. Every app, script, and Source uses it unless they ask for another.</Lead></div>
        {isPlus && !adding && !recording && (
          <div className="flex gap-2 shrink-0">
            <Button variant="primary" onClick={() => setRecording(true)}>⏺ Record a voice…</Button>
            <Button variant="ghost" onClick={pickRecording}>Add from WAV…</Button>
          </div>
        )}
      </div>
      {isPlus && <p className="text-[13px] text-[var(--muted)] mb-2">Plus voices ignore the speed setting: Pocket TTS has no speed control.</p>}
      {recording && (
        <VoiceRecorder
          onCancel={() => setRecording(false)}
          onDone={(wav, seconds) => { setRecording(false); setAdding({ path: `Recording · ${seconds.toFixed(1)} s`, wav, seconds, name: "", consent: false }); }}
        />
      )}
      {adding && (
        <Card className="mb-3 border-[var(--accent)]/60">
          <div className="font-medium">{adding.wav ? "New voice from your recording" : "New voice from a recording"}</div>
          <p className="text-[13px] text-[var(--muted)] mt-0.5 break-all">{adding.path}</p>
          {adding.wav && adding.seconds !== undefined && adding.seconds < 2 && <p className="text-[13px] text-[var(--danger)] mt-1">That's under 2 seconds; record a little longer.</p>}
          <p className="text-[13px] text-[var(--muted)] mt-1">Best results: 5–15 seconds of one person speaking clearly, no music. Longer clips are trimmed to 30 s. The file stays on this Mac.</p>
          <input autoFocus value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} placeholder="Voice name" className="mt-2 w-full rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5" />
          <label className="mt-2 flex items-start gap-2 text-[13px]"><input type="checkbox" checked={adding.consent} onChange={(e) => setAdding({ ...adding, consent: e.target.checked })} className="mt-0.5 accent-[var(--accent)]" /><span>This is my own voice, or I have the speaker's explicit consent to clone it.</span></label>
          <div className="mt-3 flex gap-2 justify-end"><Button variant="ghost" onClick={() => setAdding(null)}>Cancel</Button><Button variant="primary" disabled={!adding.name.trim() || !adding.consent} onClick={addVoice}>Add voice</Button></div>
        </Card>
      )}
      {msg && <p className="text-[13px] text-[var(--muted)] mb-2">{msg}</p>}
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
            {v.custom && <Button variant="ghost" onClick={() => deleteVoice(v.id)} aria-label="Delete voice">✕</Button>}
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
  const [apple, setApple] = useState<SystemSpeech | null>(null);
  const refresh = () => hub.hotkeysApply().then(setStatuses).catch(() => {});
  const refreshApple = () => hub.systemSpeech().then(setApple).catch(() => {});
  useEffect(() => { refresh(); }, [settings?.hotkeys.speakSelection, settings?.hotkeys.quickSpeak]);
  useEffect(() => {
    refreshApple();
    window.addEventListener("focus", refreshApple);
    return () => window.removeEventListener("focus", refreshApple);
  }, []);
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
        <Field label="Quick Speak opens" hint="The palette lists all your Sources; or jump straight to one.">
          <Select value={settings.quickSpeakAction} onChange={(v) => save({ quickSpeakAction: v })} options={[{ value: "palette", label: "Source palette" }, { value: "reader", label: "Blank Reader" }, { value: "source", label: "A specific Source" }]} />
        </Field>
        {settings.quickSpeakAction === "source" && (
          <Field label="Which Source" hint="Runs immediately when you press the shortcut.">
            <SourcePicker value={settings.quickSpeakSourceId} onChange={(v) => save({ quickSpeakSourceId: v })} none="Choose a Source…" />
          </Field>
        )}
      </Card>
      <Card className="mt-4">
        <div className="font-medium">Apple's built-in speech</div>
        {apple && (
          <ul className="text-[13px] mt-2 space-y-1">
            <li>{apple.speakSelectionEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} <span className="ml-1">Speak selection (Apple's ⌥⎋){apple.speakSelectionEnabled && settings.hotkeys.speakSelection === "Alt+Escape" ? " — conflicts with Speech Hub's shortcut. Both will fire." : ""}</span></li>
            <li>{apple.hoverSpeechEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} <span className="ml-1">Speak items under the pointer</span></li>
            <li>{apple.typingFeedbackEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} <span className="ml-1">Typing feedback</span></li>
          </ul>
        )}
        <p className="text-[13px] text-[var(--muted)] mt-2">For a single voice, turn these off in System Settings → Accessibility → Spoken Content. Speech Hub never uses Apple's voices.</p>
        <p className="text-[13px] text-[var(--muted)] mt-1">Right-click → <b>Speech → Start Speaking</b> is Apple's, built into every text field, and can't be removed. Speech Hub's items are under right-click → <b>Services</b> (“Speak with Speech Hub”, “Open in Speech Hub Reader”).</p>
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
        <Field label="Prepare browsers for Speak Selection" hint="Chromium browsers (Brave, Chrome, Edge, Arc…) build their accessibility tree only when asked, which takes a few seconds. When on, Speech Hub asks the moment a browser comes to the front, so ⌥⎋ is instant. Same flag VoiceOver sets.">
          <Toggle checked={settings.prewarmBrowsers} onChange={(v) => save({ prewarmBrowsers: v })} />
        </Field>
        <Field label="Unload the model when idle" hint="Frees ~500 MB on the Standard engine. The next request reloads it (about half a second). 0 = keep loaded.">
          <input type="number" min={0} max={240} value={settings.idleUnloadMinutes} onChange={(e) => save({ idleUnloadMinutes: Math.max(0, Number(e.target.value) || 0) })} className="w-20 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2 py-1 text-right" />
          <span className="text-[13px] text-[var(--muted)]">min</span>
          <Button variant="ghost" onClick={() => hub.unloadEngine()}>Unload now</Button>
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
  return <SourceBuilder />;
}

/** Dropdown of the user's Sources, with a "none" option. */
export function SourcePicker({ value, onChange, none }: { value: string | null; onChange: (v: string | null) => void; none: string }) {
  const { sources } = useSources();
  return (
    <select value={value ?? ""} onChange={(e) => onChange(e.target.value || null)} className="rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2.5 py-1.5 text-[14px] max-w-[240px]">
      <option value="">{none}</option>
      {sources.map((s) => <option key={s.id} value={s.id}>{s.icon} {s.name}</option>)}
    </select>
  );
}

export function PronunciationPane(_: PaneProps) {
  return <PronunciationStudio />;
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
