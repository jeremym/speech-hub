import { useEffect, useRef, useState } from "react";
import { hub, useDownloadProgress, useHubStatus, useSettings, useSources, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import VoiceRecorder from "./VoiceRecorder";
import SourceBuilder from "../SourceBuilder/SourceBuilder";
import PronunciationStudio from "../PronunciationStudio/PronunciationStudio";
import { eventToShortcut, formatShortcut } from "../../lib/shortcut";
import type { CliStatus, HotkeyStatus, ProfileCard, Settings, SystemInfo, SystemSpeech } from "../../types";
import { Badge, Button, Card, Dots, Field, H1, Kbd, Lead, Note, Section, Select, Toggle } from "../ui";

type PaneProps = { wizard?: boolean };

/** Same words everywhere: a first-time user should meet each term once and recognise it afterwards. */
const DEFAULT_HOTKEYS: Settings["hotkeys"] = { speakSelection: "Alt+Escape", quickSpeak: "Alt+Shift+Escape" };

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
    <div className="space-y-4">
      <div>
        <H1>Accessibility permission</H1>
        <Lead>To read the text you highlight in other apps, macOS requires the Accessibility permission. Speech Hub uses it for one thing: reading the current selection when you press <Kbd>Speak Selection</Kbd>.</Lead>
      </div>
      <Section
        title="Permission"
        description={trusted ? "Granted. Highlight text in any app and press the Speak Selection shortcut." : "Not granted yet. Without it, the Speak Selection shortcut can't see what you highlighted."}
        aside={trusted === null ? <Badge>Checking…</Badge> : trusted ? <Badge tone="ok">Granted</Badge> : <Badge tone="warn">Not granted</Badge>}
      >
        <div className="flex items-center justify-between gap-4 py-3">
          <Note>System Settings → Privacy &amp; Security → Accessibility → turn on Speech Hub.</Note>
          <div className="flex gap-2 shrink-0">
            {!trusted && <Button variant="primary" onClick={() => hub.axRequest()}>Grant access…</Button>}
            <Button onClick={() => hub.openSystemSettings("accessibility")}>Open System Settings</Button>
          </div>
        </div>
      </Section>
      <Section title="What this permission is used for" description="Only reading the highlighted text, only when you ask.">
        <ul className="py-3 space-y-1.5 text-[14px]">
          <li>✓ Reads the selection in the front app when you press the shortcut.</li>
          <li>✗ Never types, clicks, or moves windows for you.</li>
          <li>✗ Never reads the screen in the background.</li>
          <li>✗ Never touches the clipboard, and never sends text off this Mac.</li>
        </ul>
        <Note className="pb-3">Some apps hide their selection from macOS. For those, choose a fallback under Shortcuts → Speak Selection → “If the text can't be read”.</Note>
      </Section>
      {wizard && !trusted && <Note>You can continue without it, but the main shortcut won't work until it's granted.</Note>}
    </div>
  );
}

// ---------------------------------------------------------------- Engine

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
        An <strong>engine</strong> is the AI model that turns text into a voice. It runs entirely on this Mac; nothing is sent anywhere. Pick the one that fits your machine. You can switch any time, and each engine has its own set of voices.
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
                    {c.installed ? (c.bundled ? <Badge>Included</Badge> : <Badge tone="ok">Installed</Badge>) : c.bundled ? <Badge tone="warn">Missing from this build</Badge> : <Badge>Optional download</Badge>}
                    {isActive && <Badge tone="ok">In use</Badge>}
                    {c.voiceCloning && <Badge tone="accent">Voice cloning</Badge>}
                  </div>
                  <p className="text-[13px] text-[var(--muted)] mt-1">{c.tagline}</p>
                  <div className="flex gap-5 mt-2 text-[12px] text-[var(--muted)] flex-wrap">
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
      {status && <Note className="mt-4">Engine state: {STATE_LABEL[status.state]}{status.error ? ` · ${status.error}` : ""}</Note>}
      {wizard && <Note className="mt-1">Lite and Standard are included with the app and work offline. Nothing is downloaded unless you ask for Plus.</Note>}
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
        <div>
          <H1>Voices</H1>
          <Lead>A <strong>voice</strong> is one speaker inside the engine you chose. Pick the default here; a Source or the <code>speak</code> command can ask for a different one.{status?.engine.name ? <> Showing voices for <strong>{status.engine.name}</strong>.</> : null}</Lead>
        </div>
        {isPlus && !adding && !recording && (
          <div className="flex gap-2 shrink-0">
            <Button variant="primary" onClick={() => setRecording(true)}>⏺ Record a voice…</Button>
            <Button variant="ghost" onClick={pickRecording}>Add from WAV…</Button>
          </div>
        )}
      </div>
      {isPlus && <Note className="mb-2">Plus voices ignore the speed setting: Pocket TTS has no speed control.</Note>}
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
          {adding.wav && adding.seconds !== undefined && adding.seconds < 2 && <Note tone="warn" className="mt-1">That's under 2 seconds; record a little longer.</Note>}
          <Note className="mt-1">Best results: 5–15 seconds of one person speaking clearly, no music. Longer clips are trimmed to 30 s. The file stays on this Mac.</Note>
          <input autoFocus value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} placeholder="Voice name" className="mt-2 w-full rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5" />
          <label className="mt-2 flex items-start gap-2 text-[13px]"><input type="checkbox" checked={adding.consent} onChange={(e) => setAdding({ ...adding, consent: e.target.checked })} className="mt-0.5 accent-[var(--accent)]" /><span>This is my own voice, or I have the speaker's explicit consent to clone it.</span></label>
          <div className="mt-3 flex gap-2 justify-end"><Button variant="ghost" onClick={() => setAdding(null)}>Cancel</Button><Button variant="primary" disabled={!adding.name.trim() || !adding.consent} onClick={addVoice}>Add voice</Button></div>
        </Card>
      )}
      {msg && <Note className="mb-2">{msg}</Note>}
      <div className="flex items-center gap-3 mb-3">
        <input value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search voices…" className="flex-1 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5" />
        <label className="flex items-center gap-2 text-[13px] text-[var(--muted)]">
          Speaking speed <input type="range" min={0.5} max={2.5} step={0.05} value={settings?.speed ?? 1} onChange={(e) => save({ speed: Number(e.target.value) })} className="accent-[var(--accent)]" />
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
              {v.id === current && <Badge tone="accent">Default</Badge>}
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

// ---------------------------------------------------------------- Shortcuts

/** The shortcut itself, shown as key caps in the accent colour. Click to record a new one; Esc cancels. */
function ShortcutRecorder({ value, fallback, onChange }: { value: string; fallback: string; onChange: (v: string) => void }) {
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
    <div className="flex items-center gap-2">
      {value !== fallback && !recording && <Button variant="ghost" className="text-[12px]" onClick={() => onChange(fallback)} title={`Back to ${formatShortcut(fallback)}`}>Reset</Button>}
      <button ref={ref} type="button" onClick={() => setRecording((r) => !r)} title="Click, then press the new keys"
        className={`min-w-[96px] rounded-lg border-2 px-3 py-1 font-mono text-[18px] font-semibold tracking-wide transition ${recording ? "border-[var(--accent)] text-[var(--accent)] animate-pulse" : "border-[var(--accent)]/40 bg-[var(--accent)]/12 text-[var(--accent)] hover:border-[var(--accent)]"}`}>
        {recording ? <span className="text-[13px] font-sans font-medium">Press keys… (esc cancels)</span> : formatShortcut(value)}
      </button>
    </div>
  );
}

export function HotkeysPane({ wizard }: PaneProps) {
  const { settings, save } = useSettings();
  const { sources } = useSources();
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
  const quickSource = sources.find((s) => s.id === settings.quickSpeakSourceId);
  const fallbackSource = sources.find((s) => s.id === settings.fallbackSourceId);
  const appleConflict = !!apple?.speakSelectionEnabled && settings.hotkeys.speakSelection === "Alt+Escape";
  const registrationError = (id: string) => st(id)?.error && <Note tone="warn" className="py-2">Couldn't register this shortcut: {st(id)!.error}. Another app probably owns it; pick different keys.</Note>;
  return (
    <div className="space-y-4">
      <div>
        <H1>Keyboard shortcuts</H1>
        <Lead>Speech Hub has two shortcuts, and they work in every app, even with no Speech Hub window open. Each block below is one shortcut: the keys sit in its header (click them to change), and what the shortcut does is underneath.</Lead>
      </div>

      <Section
        title="Speak Selection"
        description="Highlight text in any app, press the keys, and hear it read aloud. Press again to stop or to read something new."
        aside={<ShortcutRecorder value={settings.hotkeys.speakSelection} fallback={DEFAULT_HOTKEYS.speakSelection} onChange={setHotkey("speakSelection")} />}
      >
        {registrationError("speak_selection")}
        <Field label="Pressing it while something is already playing" hint="Choose whether the shortcut interrupts, or acts as a Stop button.">
          <Select value={settings.repeatShortcut} onChange={(v) => save({ repeatShortcut: v })} options={[{ value: "replace", label: "Read the new selection" }, { value: "stop", label: "Stop speaking" }]} />
        </Field>
        <Field label="If the text can't be read" hint={<>A few apps hide their selection from macOS (some terminals while a program is drawing, some Electron apps). Instead of giving up, run a Source. “Current clipboard” is the usual pick: terminals like Ghostty copy on select, and elsewhere you press ⌘C first.</>}>
          <SourcePicker value={settings.fallbackSourceId} onChange={(v) => save({ fallbackSourceId: v })} none="Just tell me nothing was selected" />
        </Field>
        <Note className="py-3">Right now: press <Kbd>{formatShortcut(settings.hotkeys.speakSelection)}</Kbd> → speaks the highlighted text{fallbackSource ? <>, or runs <strong>{fallbackSource.icon} {fallbackSource.name}</strong> when nothing readable is selected</> : ""}.</Note>
      </Section>

      <Section
        title="Quick Speak"
        description="One shortcut for everything that isn't a selection: type a sentence to hear it, run one of your Sources, or open the Reader. By default it opens the Quick Speak panel, a small search box."
        aside={<ShortcutRecorder value={settings.hotkeys.quickSpeak} fallback={DEFAULT_HOTKEYS.quickSpeak} onChange={setHotkey("quickSpeak")} />}
      >
        {registrationError("quick_speak")}
        <Field label="When you press it" hint="The panel lets you search Sources or type text. The Reader is a window for pasting longer text. Or skip both and run one Source straight away.">
          <Select value={settings.quickSpeakAction} onChange={(v) => save({ quickSpeakAction: v })} options={[{ value: "palette", label: "Open the Quick Speak panel" }, { value: "reader", label: "Open an empty Reader" }, { value: "source", label: "Run one Source right away" }]} />
        </Field>
        {settings.quickSpeakAction === "source" && (
          <Field label="Which Source" hint={sources.length === 0 ? "You haven't made a Source yet. Create one under Sources first." : "Runs the moment you press the shortcut, no panel."}>
            <SourcePicker value={settings.quickSpeakSourceId} onChange={(v) => save({ quickSpeakSourceId: v })} none="Choose a Source…" />
          </Field>
        )}
        <Note className="py-3">
          Right now: press <Kbd>{formatShortcut(settings.hotkeys.quickSpeak)}</Kbd> →{" "}
          {settings.quickSpeakAction === "palette" && "opens the Quick Speak panel."}
          {settings.quickSpeakAction === "reader" && "opens an empty Reader."}
          {settings.quickSpeakAction === "source" && (quickSource ? <>runs <strong>{quickSource.icon} {quickSource.name}</strong>.</> : <span className="text-[var(--danger)]">nothing yet. Choose a Source above.</span>)}
        </Note>
      </Section>

      <Section
        title="Apple's built-in speech"
        description="macOS has its own speech features, using Apple's voices rather than Speech Hub's. Here's what is switched on right now, so the two don't talk over each other."
        aside={apple && (apple.speakSelectionEnabled || apple.hoverSpeechEnabled || apple.typingFeedbackEnabled) ? <Badge tone="warn">Some on</Badge> : apple ? <Badge tone="ok">All off</Badge> : <Badge>Checking…</Badge>}
      >
        {apple && (
          <ul className="py-3 space-y-1.5 text-[14px]">
            <li className="flex items-center gap-2">{apple.speakSelectionEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} Apple's “Speak selection” (also <Kbd>⌥⎋</Kbd> by default)</li>
            <li className="flex items-center gap-2">{apple.hoverSpeechEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} Speak items under the pointer</li>
            <li className="flex items-center gap-2">{apple.typingFeedbackEnabled ? <Badge tone="warn">On</Badge> : <Badge tone="ok">Off</Badge>} Typing feedback</li>
          </ul>
        )}
        {appleConflict && (
          <div className="pb-3 flex items-center justify-between gap-4">
            <Note tone="warn">Apple's Speak selection and Speech Hub both use <Kbd>⌥⎋</Kbd>. Both will fire. Turn Apple's off, or move Speech Hub.</Note>
            <Button onClick={() => setHotkey("speakSelection")("Control+Alt+S")}>Use ⌃⌥S for Speech Hub</Button>
          </div>
        )}
        <Note className="pb-2">Right-click → <b>Speech → Start Speaking</b> is Apple's. It is built into every text field and can't be removed. Speech Hub's menu items live under right-click → <b>Services</b>: “Speak with Speech Hub” and “Open in Speech Hub Reader”.</Note>
        <div className="pb-3"><Button onClick={() => hub.openSystemSettings("spoken_content")}>Open Apple's Spoken Content settings</Button></div>
      </Section>

      {wizard && <Note>Defaults: <Kbd>⌥⎋</Kbd> speaks the selection, <Kbd>⌥⇧⎋</Kbd> opens Quick Speak. Change them any time in Preferences → Shortcuts.</Note>}
    </div>
  );
}

// ---------------------------------------------------------------- General

export function GeneralPane(_: PaneProps) {
  const { settings, save } = useSettings();
  if (!settings) return null;
  return (
    <div className="space-y-4">
      <div>
        <H1>General</H1>
        <Lead>Speech Hub is a menu-bar app. It stays running in the background so the shortcuts and the <code>speak</code> command always work.</Lead>
      </div>
      <Section title="Running in the background" description="Closing a window, or pressing ⌘Q in one, only hides windows. To quit for real, use the menu-bar icon → Quit Speech Hub.">
        <Field label="Start at login" hint="Adds Speech Hub to your Login Items so it's ready after a restart.">
          <Toggle checked={settings.launchAtLogin} onChange={(v) => save({ launchAtLogin: v })} />
        </Field>
        <Field label="Free memory when idle" hint="Unloads the engine after this many minutes of silence (about 500 MB for Standard). The next request reloads it in about half a second. 0 keeps it loaded.">
          <input type="number" min={0} max={240} value={settings.idleUnloadMinutes} onChange={(e) => save({ idleUnloadMinutes: Math.max(0, Number(e.target.value) || 0) })} className="w-20 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2 py-1 text-right" />
          <span className="text-[13px] text-[var(--muted)]">min</span>
          <Button variant="ghost" onClick={() => hub.unloadEngine()}>Unload now</Button>
        </Field>
      </Section>
      <Section title="While speaking" description="What you see on screen while text is being read.">
        <Field label="Show the mini player" hint="A small floating bar with Pause and Stop appears while speaking, unless the Reader is already open.">
          <Toggle checked={settings.showMiniPlayer} onChange={(v) => save({ showMiniPlayer: v })} />
        </Field>
      </Section>
      <Section title="Browsers" description="Chrome, Brave, Edge, Arc and other Chromium browsers only build the data Speech Hub reads when something asks for it, which takes a few seconds the first time.">
        <Field label="Get browsers ready in advance" hint="Asks the moment a browser comes to the front, so the first Speak Selection in it is instant. Same request VoiceOver makes; harmless to other apps.">
          <Toggle checked={settings.prewarmBrowsers} onChange={(v) => save({ prewarmBrowsers: v })} />
        </Field>
      </Section>
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
    <div className="space-y-4">
      <div>
        <H1>Command line tool</H1>
        <Lead>Optional, for people who use the Terminal. The <code>speak</code> command lets any script or app talk through Speech Hub. It hands the text to this running app, so nothing else has to load a model.</Lead>
      </div>
      <Section
        title="Install speak"
        description={st?.installed ? <>Linked at <code className="select-text">{st.link}</code>.</> : <>Puts a link at <code className="select-text">{st?.link ?? "~/.local/bin/speak"}</code>. No password needed, and Remove undoes it.</>}
        aside={<>{st === null ? <Badge>Checking…</Badge> : st.installed ? <Badge tone="ok">Installed</Badge> : <Badge>Not installed</Badge>}{st?.installed && !st.inPath && <Badge tone="warn">Not on PATH</Badge>}</>}
      >
        <div className="flex items-center justify-between gap-4 py-3">
          <Note>{st?.installed ? "Open a terminal and type speak to try it." : "Install it, then open a new terminal window."}</Note>
          <div className="flex gap-2 shrink-0">
            {st?.installed
              ? <Button onClick={() => act(hub.cliUninstall())}>Remove</Button>
              : <Button variant="primary" disabled={!st?.bundled} onClick={() => act(hub.cliInstall())}>Install</Button>}
          </div>
        </div>
        {st && !st.bundled && <Note tone="warn" className="pb-3">This build has no bundled <code>speak</code> binary. Run <code>scripts/build-cli.sh</code>.</Note>}
        {st?.installed && !st.inPath && (
          <Note className="pb-3">Your shell doesn't look in <code>~/.local/bin</code> yet. Add this to <code>~/.zshrc</code>, then open a new terminal:
            <pre className="mt-1 select-text">export PATH="$HOME/.local/bin:$PATH"</pre></Note>
        )}
        {err && <Note tone="warn" className="pb-3">{err}</Note>}
      </Section>
      <Section title="Examples" description="Everything speak can do, one line each.">
        <pre className="py-3 text-[13px] leading-6 overflow-auto select-text">{`echo "Build finished" | speak      # speak now
speak -f notes.md                   # speak a file
pbpaste | speak --open              # open in the Reader
speak -v bm_george "Cheers"         # pick a voice
speak -o out.wav "Render me"        # write a WAV, no playback
speak run morning-briefing          # run a saved Source
speak stop`}</pre>
      </Section>
      <Note>Developers: the same socket protocol is documented in <code>docs/public-api.md</code>. Links like <code>speechhub://palette</code> also work.</Note>
      {wizard && <Note>Optional. The app works without it.</Note>}
    </div>
  );
}

// ---------------------------------------------------------------- Sources, Pronunciation, About

export function SourcesPane(_: PaneProps) {
  return <SourceBuilder />;
}

/** Dropdown of the user's Sources, with a "none" option. */
export function SourcePicker({ value, onChange, none }: { value: string | null; onChange: (v: string | null) => void; none: string }) {
  const { sources } = useSources();
  return (
    <select value={value ?? ""} onChange={(e) => onChange(e.target.value || null)} className="rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2.5 py-1.5 text-[14px] max-w-[260px]">
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
    <div className="space-y-4">
      <div>
        <H1>About Speech Hub</H1>
        <Lead>Give anything on your Mac a voice. Local, private, open source (GPL-3.0).</Lead>
      </div>
      <Section title="This installation">
        <dl className="grid grid-cols-[150px_1fr] gap-y-2 py-3 text-[14px]">
          <dt className="text-[var(--muted)]">Version</dt><dd className="select-text">{info?.appVersion ?? "…"}</dd>
          <dt className="text-[var(--muted)]">macOS</dt><dd>{info?.macos ?? "…"}</dd>
          <dt className="text-[var(--muted)]">Settings folder</dt><dd className="select-text break-all text-[13px]">{info?.dataDir ?? "…"}</dd>
        </dl>
      </Section>
      <Section title="The words Speech Hub uses" description="Every screen uses these the same way.">
        <dl className="grid grid-cols-[150px_1fr] gap-y-2 py-3 text-[14px]">
          <dt className="font-medium">Speak Selection</dt><dd className="text-[var(--muted)]">The shortcut that reads whatever text you highlighted.</dd>
          <dt className="font-medium">Quick Speak</dt><dd className="text-[var(--muted)]">The shortcut for everything else: type text, run a Source, open the Reader.</dd>
          <dt className="font-medium">Source</dt><dd className="text-[var(--muted)]">A saved place to get text from: the clipboard, a file, a command, an Apple Shortcut, a web address.</dd>
          <dt className="font-medium">Reader</dt><dd className="text-[var(--muted)]">A window to paste or type longer text and listen with Play, Pause and Stop.</dd>
          <dt className="font-medium">Mini player</dt><dd className="text-[var(--muted)]">The small floating Pause/Stop bar shown while speaking.</dd>
          <dt className="font-medium">Engine</dt><dd className="text-[var(--muted)]">The AI model that turns text into speech, running on this Mac.</dd>
          <dt className="font-medium">Voice</dt><dd className="text-[var(--muted)]">One speaker inside an engine.</dd>
          <dt className="font-medium">Pronunciation rule</dt><dd className="text-[var(--muted)]">“Written as → spoken as”, applied everywhere before the engine hears the text.</dd>
        </dl>
      </Section>
    </div>
  );
}

export function errorText(e: unknown) { return errorMessage(e); }
