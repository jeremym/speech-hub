import { useEffect, useRef, useState } from "react";
import { hub, useDownloadProgress, useHubStatus, useSettings, useSources, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { Empty, Glyph, Progress, Segmented, Tag, Toggle, Zone, loadLook, type Look } from "@overtone-foundry/app-kit";
import VoiceRecorder from "./VoiceRecorder";
import SourceBuilder from "../SourceBuilder/SourceBuilder";
import PronunciationStudio from "../PronunciationStudio/PronunciationStudio";
import { eventToShortcut, formatShortcut } from "../../lib/shortcut";
import { followLook, LOOK_KEY, setLook } from "../../look";
import type { CliStatus, HotkeyStatus, ProfileCard, Settings, SystemInfo, SystemSpeech } from "../../types";
import { ConfirmButton, Head, Line, Meter } from "../bits";

type PaneProps = { wizard?: boolean };

/** Same words everywhere: a first-time user should meet each term once and recognise it afterwards. */
const DEFAULT_HOTKEYS: Settings["hotkeys"] = { speakSelection: "Alt+Escape", quickSpeak: "Alt+Shift+Escape" };

/** In the wizard the footer's Continue is the one filled button, so a pane's own action is outlined there. */
const primary = (wizard?: boolean) => (wizard ? "" : "primary");

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
      <Head title="Accessibility permission">
        To read the text you highlight in other apps, macOS requires the Accessibility permission. Speech Hub uses it for one thing: reading the current selection when you press <kbd>Speak Selection</kbd>.
      </Head>
      <Zone title="Permission" aside={trusted === null ? <Tag>checking</Tag> : trusted ? <Tag tone="live">granted</Tag> : <Tag tone="amber">not granted</Tag>}>
        <p>{trusted ? "Granted. Highlight text in any app and press the Speak Selection shortcut." : "Not granted yet. Without it, the Speak Selection shortcut can't see what you highlighted."}</p>
        <Line label="System Settings → Privacy & Security → Accessibility" hint="Turn on Speech Hub in that list.">
          {!trusted && <button className={primary(wizard)} onClick={() => hub.axRequest()}>Grant access…</button>}
          <button className={trusted ? "" : "quiet"} onClick={() => hub.openSystemSettings("accessibility")}>Open System Settings</button>
        </Line>
      </Zone>
      <Zone title="What it is used for" sub="only reading the highlighted text, only when you ask">
        <ul className="rows">
          <li><span className="check"><Glyph name="yesno" /> Reads the selection in the front app when you press the shortcut.</span></li>
          <li><span className="check"><Glyph name="close" /> Never types, clicks, or moves windows for you.</span></li>
          <li><span className="check"><Glyph name="close" /> Never reads the screen in the background.</span></li>
          <li><span className="check"><Glyph name="close" /> Never touches the clipboard, and never sends text off this Mac.</span></li>
        </ul>
        <p className="dim" style={{ marginTop: 10 }}>Some apps hide their selection from macOS. For those, choose a fallback under Shortcuts → Speak Selection → “If the text can't be read”.</p>
      </Zone>
      {wizard && !trusted && <p className="hint-line">You can continue without it, but the main shortcut won't work until it's granted.</p>}
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
      <Head title="Speech engine">
        An <strong>engine</strong> is the AI model that turns text into a voice. It runs entirely on this Mac; nothing is sent anywhere. Pick the one that fits your machine. You can switch any time, and each engine has its own set of voices.
        {info && <> This Mac: <strong>{info.arch === "aarch64" ? "Apple Silicon" : info.arch}</strong>, <strong>{Math.round(info.memoryGb)} GB</strong> memory.</>}
      </Head>
      <div className="engines">
        {cards.map((c) => {
          const isSelected = selected === c.id;
          const isActive = active === c.id && status?.engine.loaded;
          const downloading = progress && progress.profile === "plus" && progress.phase !== "error";
          return (
            <div key={c.id} className={`card${isSelected ? " selected" : ""}`} aria-current={isSelected ? "true" : undefined}>
              <span className="card-title">
                {c.name}
                {isSelected && <Tag tone="live">chosen</Tag>}
                {isActive && <Tag tone="live">in use</Tag>}
                {c.recommended && <Tag>recommended</Tag>}
                {c.devOnly && <Tag tone="amber">dev only</Tag>}
                {c.installed ? (c.bundled ? <Tag>included</Tag> : <Tag>installed</Tag>) : c.bundled ? <Tag tone="bad">missing from this build</Tag> : <Tag tone="amber">optional download</Tag>}
                {c.voiceCloning && <Tag>voice cloning</Tag>}
              </span>
              <span className="dim">{c.tagline}</span>
              <span className="card-meta">
                <span>Quality <Meter value={c.quality} label="Quality" /></span>
                <span>Speed <Meter value={c.speed} label="Speed" /></span>
                <span>{c.sizeLabel}</span>
                <span>RAM {c.ramLabel}</span>
                <span>{c.languages.join(", ")}</span>
                <span>{c.license}</span>
              </span>
              {c.installed && !isSelected && (
                <div className="row"><button className="quiet" onClick={() => save({ engineProfile: c.id, voiceId: "" })}>Use {c.name}</button></div>
              )}
              {c.id === "plus" && !c.installed && (
                <div className="card-foot">
                  {downloading ? (
                    <Progress value={progress.total ? progress.bytes / progress.total : undefined} label="Plus download">
                      {progress.phase === "downloading" ? "Downloading…" : progress.phase === "verifying" ? "Verifying checksum…" : "Installing…"}
                      {progress.total ? ` ${Math.round((progress.bytes / progress.total) * 100)}%` : ""}
                    </Progress>
                  ) : (
                    <>
                      <p className="dim">Pocket TTS is made by Kyutai and licensed <strong>CC BY 4.0</strong>. Its model card prohibits voice impersonation or cloning without the speaker's explicit, lawful consent. Downloaded once from GitHub (sherpa-onnx releases) and verified by checksum; nothing else ever leaves this Mac.</p>
                      <label className="row"><input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} /><span>I'll only clone voices I have the right to use, and I accept the CC BY 4.0 terms.</span></label>
                      <div className="row">
                        <button className="amber" disabled={!consent} onClick={startDownload}><Glyph name="cloud" /> Download Plus (98 MB)</button>
                        <span className="dim">Needs the network once.</span>
                      </div>
                    </>
                  )}
                  {dlErr && <pre className="error">{dlErr}</pre>}
                </div>
              )}
              {c.id === "plus" && c.installed && (
                <div className="card-foot row space">
                  <span className="dim">Add your own voices under Voices while Plus is chosen.</span>
                  <ConfirmButton onConfirm={removePlus} really="Really remove">Remove download</ConfirmButton>
                </div>
              )}
            </div>
          );
        })}
      </div>
      {status && <p className="dim" style={{ marginTop: 12 }}>Engine state: <span className="status-word">{STATE_LABEL[status.state]}</span>{status.error ? <span className="red"> · {status.error}</span> : ""}</p>}
      {wizard && <p className="hint-line">Lite and Standard are included with the app and work offline. Nothing is downloaded unless you ask for Plus.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- Voices

export function VoicesPane({ wizard }: PaneProps) {
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
    <div>
      <Head
        title="Voices"
        actions={isPlus && !adding && !recording && (
          <>
            <button className={primary(wizard)} onClick={() => setRecording(true)}><Glyph name="audio" /> Record a voice…</button>
            <button className="quiet" onClick={pickRecording}>Add from WAV…</button>
          </>
        )}
      >
        A <strong>voice</strong> is one speaker inside the engine you chose. Pick the default here; a Source or the <code>speak</code> command can ask for a different one.{status?.engine.name ? <> Showing voices for <strong>{status.engine.name}</strong>.</> : null}
      </Head>
      {isPlus && <p className="dim">Plus voices ignore the speed setting: Pocket TTS has no speed control.</p>}
      {recording && (
        <VoiceRecorder
          onCancel={() => setRecording(false)}
          onDone={(wav, seconds) => { setRecording(false); setAdding({ path: `Recording · ${seconds.toFixed(1)} s`, wav, seconds, name: "", consent: false }); }}
        />
      )}
      {adding && (
        <Zone className="editing" title={adding.wav ? "New voice from your recording" : "New voice from a recording"}>
          <p className="dim" style={{ wordBreak: "break-all" }}>{adding.path}</p>
          {adding.wav && adding.seconds !== undefined && adding.seconds < 2 && <p className="hint-line">That's under 2 seconds; record a little longer.</p>}
          <p className="dim">Best results: 5–15 seconds of one person speaking clearly, no music. Longer clips are trimmed to 30 s. The file stays on this Mac.</p>
          <div className="field"><label><strong>Voice name</strong></label><input type="text" autoFocus value={adding.name} onChange={(e) => setAdding({ ...adding, name: e.target.value })} /></div>
          <label className="row"><input type="checkbox" checked={adding.consent} onChange={(e) => setAdding({ ...adding, consent: e.target.checked })} /><span>This is my own voice, or I have the speaker's explicit consent to clone it.</span></label>
          <div className="row"><button className="primary" disabled={!adding.name.trim() || !adding.consent} onClick={addVoice}>Add voice</button><button className="quiet" onClick={() => setAdding(null)}>Cancel</button></div>
        </Zone>
      )}
      {msg && <p className="hint-line">{msg}</p>}
      <div className="row space">
        <input type="search" value={filter} onChange={(e) => setFilter(e.target.value)} placeholder="Search voices…" style={{ flex: 1, minWidth: 200 }} aria-label="Search voices" />
        <label>
          <span className="dim">Speaking speed</span>
          <span className="row" style={{ margin: 0 }}>
            <input type="range" min={0.5} max={2.5} step={0.05} value={settings?.speed ?? 1} onChange={(e) => save({ speed: Number(e.target.value) })} aria-label="Speaking speed" />
            <span className="num">{(settings?.speed ?? 1).toFixed(2)}×</span>
          </span>
        </label>
      </div>
      {shown.length === 0
        ? <Empty word={voices.length === 0 ? "No voices" : "No match"}>{voices.length === 0 ? "The engine is still loading or isn't installed." : "Try a shorter search."}</Empty>
        : (
          <ul className="rows actions">
            {shown.map((v) => (
              <li key={v.id}>
                <span className="main">
                  <button className="link" onClick={() => save({ voiceId: v.id })} aria-pressed={v.id === current}>{v.name}</button>
                  {" "}<span className="dim">{v.language}</span>
                  {v.id === current && <Tag tone="live">default</Tag>}
                  {v.custom && <Tag>custom</Tag>}
                </span>
                <span className="act">
                  <button className="quiet" onClick={() => hub.previewVoice(v.id).catch(() => {})}><Glyph name="play" /> Preview</button>
                  {v.custom && <ConfirmButton onConfirm={() => deleteVoice(v.id)} title="Delete this voice">Delete</ConfirmButton>}
                </span>
              </li>
            ))}
          </ul>
        )}
    </div>
  );
}

// ---------------------------------------------------------------- Shortcuts

/** The keys themselves, in the display face. Click to record new ones; Esc cancels. */
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
    <>
      {value !== fallback && !recording && <button className="quiet" onClick={() => onChange(fallback)} title={`Back to ${formatShortcut(fallback)}`}>Reset</button>}
      <button ref={ref} type="button" className={`keys${recording ? " recording pulse" : ""}`} onClick={() => setRecording((r) => !r)} title="Click, then press the new keys">
        {recording ? "press keys · esc cancels" : formatShortcut(value)}
      </button>
    </>
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
  const appleOn = !!apple && (apple.speakSelectionEnabled || apple.hoverSpeechEnabled || apple.typingFeedbackEnabled);
  const registrationError = (id: string) => st(id)?.error && <pre className="error">Couldn't register this shortcut: {st(id)!.error}. Another app probably owns it; pick different keys.</pre>;
  const onOff = (on: boolean) => (on ? <Tag tone="amber">on</Tag> : <Tag>off</Tag>);
  return (
    <div>
      <Head title="Keyboard shortcuts">
        Speech Hub has two shortcuts, and they work in every app, even with no Speech Hub window open. Each block below is one shortcut: the keys sit in its header (click them to change), and what the shortcut does is underneath.
      </Head>

      <Zone title="Speak Selection" aside={<ShortcutRecorder value={settings.hotkeys.speakSelection} fallback={DEFAULT_HOTKEYS.speakSelection} onChange={setHotkey("speakSelection")} />}>
        <p>Highlight text in any app, press the keys, and hear it read aloud. Press again to stop or to read something new.</p>
        {registrationError("speak_selection")}
        <Line label="Pressing it while something is already playing" hint="Choose whether the shortcut interrupts, or acts as a Stop button.">
          <select value={settings.repeatShortcut} onChange={(e) => save({ repeatShortcut: e.target.value as Settings["repeatShortcut"] })}>
            <option value="replace">Read the new selection</option>
            <option value="stop">Stop speaking</option>
          </select>
        </Line>
        <Line label="If the text can't be read" hint="Only when nothing is highlighted, or the app hides its selection from macOS (some terminals while a program is drawing, some Electron apps). A highlighted selection is always read first.">
          <select value={settings.fallbackAction} onChange={(e) => { const v = e.target.value as Settings["fallbackAction"]; save({ fallbackAction: v, fallbackSourceId: v === "source" ? settings.fallbackSourceId : null }); }}>
            <option value="notice">Just tell me</option>
            <option value="quick_speak">Open the Quick Speak panel</option>
            <option value="source">Run a Source</option>
          </select>
        </Line>
        {settings.fallbackAction === "source" && (
          <Line label="Which Source" hint="“Current clipboard” is the usual pick: terminals like Ghostty copy on select, and elsewhere you press ⌘C first.">
            <SourcePicker value={settings.fallbackSourceId} onChange={(v) => save({ fallbackSourceId: v, fallbackAction: v ? "source" : "notice" })} none="Choose a Source…" />
          </Line>
        )}
        <p className="hint-line" style={{ marginTop: 10 }}>
          Right now: press <kbd>{formatShortcut(settings.hotkeys.speakSelection)}</kbd> → speaks the highlighted text.
          {" "}When nothing readable is selected: {settings.fallbackAction === "notice" && "a short “Nothing is selected” notice."}
          {settings.fallbackAction === "quick_speak" && "the Quick Speak panel opens."}
          {settings.fallbackAction === "source" && (fallbackSource ? <>runs <strong>{fallbackSource.icon} {fallbackSource.name}</strong>.</> : <span className="amber">no Source chosen yet.</span>)}
        </p>
      </Zone>

      <Zone title="Quick Speak" aside={<ShortcutRecorder value={settings.hotkeys.quickSpeak} fallback={DEFAULT_HOTKEYS.quickSpeak} onChange={setHotkey("quickSpeak")} />}>
        <p>One shortcut for everything that isn't a selection: type a sentence to hear it, run one of your Sources, or open the Reader. By default it opens the Quick Speak panel, a small search box.</p>
        {registrationError("quick_speak")}
        <Line label="When you press it" hint="The panel lets you search Sources or type text. The Reader is a window for pasting longer text. Or skip both and run one Source straight away.">
          <select value={settings.quickSpeakAction} onChange={(e) => save({ quickSpeakAction: e.target.value as Settings["quickSpeakAction"] })}>
            <option value="palette">Open the Quick Speak panel</option>
            <option value="reader">Open an empty Reader</option>
            <option value="source">Run one Source right away</option>
          </select>
        </Line>
        {settings.quickSpeakAction === "source" && (
          <Line label="Which Source" hint={sources.length === 0 ? "You haven't made a Source yet. Create one under Sources first." : "Runs the moment you press the shortcut, no panel."}>
            <SourcePicker value={settings.quickSpeakSourceId} onChange={(v) => save({ quickSpeakSourceId: v })} none="Choose a Source…" />
          </Line>
        )}
        <p className="hint-line" style={{ marginTop: 10 }}>
          Right now: press <kbd>{formatShortcut(settings.hotkeys.quickSpeak)}</kbd> →{" "}
          {settings.quickSpeakAction === "palette" && "opens the Quick Speak panel."}
          {settings.quickSpeakAction === "reader" && "opens an empty Reader."}
          {settings.quickSpeakAction === "source" && (quickSource ? <>runs <strong>{quickSource.icon} {quickSource.name}</strong>.</> : <span className="amber">nothing yet. Choose a Source above.</span>)}
        </p>
      </Zone>

      <Zone title="Apple's built-in speech" sub="Spoken Content" aside={apple ? (appleOn ? <Tag tone="amber">some on</Tag> : <Tag>all off</Tag>) : <Tag>checking</Tag>}>
        <p>macOS has its own speech features, using Apple's voices rather than Speech Hub's. Here's what is switched on right now, so the two don't talk over each other.</p>
        {apple && (
          <ul className="rows">
            <li>{onOff(apple.speakSelectionEnabled)} Apple's “Speak selection” (also <kbd>⌥⎋</kbd> by default)</li>
            <li>{onOff(apple.hoverSpeechEnabled)} Speak items under the pointer</li>
            <li>{onOff(apple.typingFeedbackEnabled)} Typing feedback</li>
          </ul>
        )}
        {appleConflict && (
          <>
            <div className="notice"><Glyph name="warning" /> Apple's Speak selection and Speech Hub both use ⌥⎋. Both will fire.</div>
            <div className="row"><button onClick={() => setHotkey("speakSelection")("Control+Alt+S")}>Use ⌃⌥S for Speech Hub</button><span className="dim">or turn Apple's off below.</span></div>
          </>
        )}
        <p className="dim" style={{ marginTop: 10 }}>Right-click → <b>Speech → Start Speaking</b> is Apple's. It is built into every text field and can't be removed. Speech Hub's menu items live under right-click → <b>Services</b>: “Speak with Speech Hub” and “Open in Speech Hub Reader”.</p>
        <div className="row"><button className="quiet" onClick={() => hub.openSystemSettings("spoken_content")}>Open Apple's Spoken Content settings</button></div>
      </Zone>

      {wizard && <p className="hint-line">Defaults: <kbd>⌥⎋</kbd> speaks the selection, <kbd>⌥⇧⎋</kbd> opens Quick Speak. Change them any time in Preferences → Shortcuts.</p>}
    </div>
  );
}

// ---------------------------------------------------------------- General

export function GeneralPane(_: PaneProps) {
  const { settings, save } = useSettings();
  const [look, setLookState] = useState<Look>(() => loadLook(LOOK_KEY));
  useEffect(() => followLook(setLookState), []);
  const changeLook = (patch: Partial<Look>) => { const next = { ...look, ...patch }; setLookState(next); setLook(next); };
  if (!settings) return null;
  return (
    <div>
      <Head title="General">
        Speech Hub is a menu-bar app. It stays running in the background so the shortcuts and the <code>speak</code> command always work.
      </Head>
      <Zone title="Look" sub="theme and text size">
        <div className="settings-row">
          <span className="label">Theme</span>
          <Segmented label="Theme" value={look.theme} onChange={(theme) => changeLook({ theme })} options={[{ value: "green", label: "Green" }, { value: "amber", label: "Amber" }, { value: "paper", label: "Paper" }]} />
          <span className="label">Text size</span>
          <Segmented label="Text size" value={look.size} onChange={(size) => changeLook({ size })} options={[{ value: "normal", label: "Normal" }, { value: "larger", label: "Larger" }, { value: "largest", label: "Largest" }]} />
          <span className="label">Scanlines</span>
          <span><Toggle label="Scanlines" on={look.scanlines} onChange={(scanlines) => changeLook({ scanlines })} /> <span className="dim">· faint lines over the screen, off unless you like them</span></span>
        </div>
        <p className="dim" style={{ marginTop: 10 }}>Paper is the light, high-contrast look. The View menu switches these too (⌘+ and ⌘- for text size).</p>
      </Zone>
      <Zone title="Running in the background">
        <p>Closing a window, or pressing ⌘Q in one, only hides windows. To quit for real, use the menu-bar icon → Quit Speech Hub.</p>
        <Line label="Start at login" hint="Adds Speech Hub to your Login Items so it's ready after a restart.">
          <Toggle label="Start at login" on={settings.launchAtLogin} onChange={(v) => save({ launchAtLogin: v })} />
        </Line>
        <Line label="Free memory when idle" hint="Unloads the engine after this many minutes of silence (about 500 MB for Standard). The next request reloads it in about half a second. 0 keeps it loaded.">
          <input type="number" min={0} max={240} value={settings.idleUnloadMinutes} onChange={(e) => save({ idleUnloadMinutes: Math.max(0, Number(e.target.value) || 0) })} aria-label="Minutes" />
          <span className="dim">min</span>
          <button className="quiet" onClick={() => hub.unloadEngine()}>Unload now</button>
        </Line>
      </Zone>
      <Zone title="While speaking">
        <p>What you see on screen while text is being read.</p>
        <Line label="Show the mini player" hint="A small floating bar with Pause and Stop appears while speaking, unless the Reader is already open.">
          <Toggle label="Show the mini player" on={settings.showMiniPlayer} onChange={(v) => save({ showMiniPlayer: v })} />
        </Line>
      </Zone>
      <Zone title="Browsers">
        <p>Chrome, Brave, Edge, Arc and other Chromium browsers only build the data Speech Hub reads when something asks for it, which takes a few seconds the first time.</p>
        <Line label="Get browsers ready in advance" hint="Asks the moment a browser comes to the front, so the first Speak Selection in it is instant. Same request VoiceOver makes; harmless to other apps.">
          <Toggle label="Get browsers ready in advance" on={settings.prewarmBrowsers} onChange={(v) => save({ prewarmBrowsers: v })} />
        </Line>
      </Zone>
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
      <Head title="Command line tool">
        Optional, for people who use the Terminal. The <code>speak</code> command lets any script or app talk through Speech Hub. It hands the text to this running app, so nothing else has to load a model.
      </Head>
      <Zone title="Install speak" aside={<>{st === null ? <Tag>checking</Tag> : st.installed ? <Tag tone="live">installed</Tag> : <Tag>not installed</Tag>}{st?.installed && !st.inPath && <Tag tone="amber">not on PATH</Tag>}</>}>
        <p>{st?.installed ? <>Linked at <code>{st.link}</code>.</> : <>Puts a link at <code>{st?.link ?? "~/.local/bin/speak"}</code>. No password needed, and Remove undoes it.</>}</p>
        <Line label={st?.installed ? "Open a terminal and type speak to try it." : "Install it, then open a new terminal window."}>
          {st?.installed
            ? <ConfirmButton onConfirm={() => act(hub.cliUninstall())}>Remove</ConfirmButton>
            : <button className={primary(wizard)} disabled={!st?.bundled} onClick={() => act(hub.cliInstall())}>{st?.bundled === null ? "Install · needs the binary" : "Install"}</button>}
        </Line>
        {st && !st.bundled && <pre className="error">This build has no bundled speak binary. Run scripts/build-cli.sh.</pre>}
        {st?.installed && !st.inPath && (
          <p className="dim">Your shell doesn't look in <code>~/.local/bin</code> yet. Add this to <code>~/.zshrc</code>, then open a new terminal:
            <pre>export PATH="$HOME/.local/bin:$PATH"</pre></p>
        )}
        {err && <pre className="error">{err}</pre>}
      </Zone>
      <Zone title="Examples" sub="everything speak can do, one line each">
        <pre>{`echo "Build finished" | speak      # speak now
speak -f notes.md                   # speak a file
pbpaste | speak --open              # open in the Reader
speak -v bm_george "Cheers"         # pick a voice
speak -o out.wav "Render me"        # write a WAV, no playback
speak run morning-briefing          # run a saved Source
speak stop`}</pre>
      </Zone>
      <p className="dim">Developers: the same socket protocol is documented in <code>docs/public-api.md</code>. Links like <code>speechhub://palette</code> also work.</p>
      {wizard && <p className="hint-line">Optional. The app works without it.</p>}
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
    <select value={value ?? ""} onChange={(e) => onChange(e.target.value || null)}>
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
      <Head title="About Speech Hub">Give anything on your Mac a voice. Local, private, open source (GPL-3.0).</Head>
      <Zone title="This installation">
        <div className="facts">
          <span className="label">Version</span><span>{info?.appVersion ?? "…"}</span>
          <span className="label">macOS</span><span>{info?.macos ?? "…"}</span>
          <span className="label">Settings</span><span style={{ wordBreak: "break-all" }}>{info?.dataDir ?? "…"}</span>
        </div>
      </Zone>
      <Zone title="The words Speech Hub uses" sub="every screen uses these the same way">
        <div className="facts wide">
          <span className="label">Speak Selection</span><span>The shortcut that reads whatever text you highlighted.</span>
          <span className="label">Quick Speak</span><span>The shortcut for everything else: type text, run a Source, open the Reader.</span>
          <span className="label">Source</span><span>A saved place to get text from: the clipboard, a file, a command, an Apple Shortcut, a web address.</span>
          <span className="label">Reader</span><span>A window to paste or type longer text and listen with Play, Pause and Stop.</span>
          <span className="label">Mini player</span><span>The small floating Pause/Stop bar shown while speaking.</span>
          <span className="label">Engine</span><span>The AI model that turns text into speech, running on this Mac.</span>
          <span className="label">Voice</span><span>One speaker inside an engine.</span>
          <span className="label">Pronunciation rule</span><span>“Written as → spoken as”, applied everywhere before the engine hears the text.</span>
        </div>
      </Zone>
    </div>
  );
}

export function errorText(e: unknown) { return errorMessage(e); }
