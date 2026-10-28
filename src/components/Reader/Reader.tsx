import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Glyph, Paper, Rail } from "@overtone-foundry/app-kit";
import { hub, useHubStatus, useSettings, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import { Logo } from "../../brand";
import type { ReaderText } from "../../types";

export default function Reader() {
  const [text, setText] = useState("");
  const [title, setTitle] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const status = useHubStatus();
  const { settings, save } = useSettings();
  const voices = useVoices(status?.engine.id);

  useEffect(() => {
    const apply = (p: ReaderText | null) => { if (p) { setText(p.text); setTitle(p.title); } };
    hub.readerTakePending().then(apply).catch(() => {});
    const un = listen<ReaderText>("reader:text", (e) => apply(e.payload));
    return () => { un.then((u) => u()); };
  }, []);

  const busy = status && ["synthesizing", "playing", "paused"].includes(status.state);
  const speak = () => {
    setErr(null);
    hub.speak(text, { title: title ?? "Reader" }).catch((e) => setErr(errorMessage(e)));
  };
  const fixPronunciation = () => {
    const ta = document.querySelector("textarea");
    const sel = ta ? text.slice(ta.selectionStart, ta.selectionEnd).trim() : "";
    const start = ta ? Math.max(0, ta.selectionStart - 60) : 0;
    const context = ta ? text.slice(start, ta.selectionEnd + 60).replace(/\s+/g, " ").trim() : "";
    hub.openWindowWithPrefill("settings", "pronunciation", { text: sel, context: context || sel });
  };
  const onKey = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") { e.preventDefault(); speak(); }
    if (e.key === "Escape" && busy) { hub.stop(); }
  };
  const problem = err ?? (status?.state === "error" ? status.error : null);
  const state = status ? STATE_LABEL[status.state] : "";

  return (
    <div className="win reader" onKeyDown={onKey}>
      <Rail
        left={
          <div className="reader-tools">
            <Logo size={24} word={false} />
            {busy && status?.state !== "paused" ? (
              <button onClick={() => hub.pause()}><Glyph name="timer" /> Pause</button>
            ) : status?.state === "paused" ? (
              <button className="primary" onClick={() => hub.resume()}><Glyph name="play" /> Resume</button>
            ) : (
              <button className="primary" onClick={speak} disabled={!text.trim()}><Glyph name="play" /> {text.trim() ? "Speak" : "Speak · needs text"}</button>
            )}
            <button className="stop" onClick={() => hub.stop()} disabled={!busy}><Glyph name="stop" /> Stop</button>
            <select value={settings?.voiceId ?? ""} onChange={(e) => save({ voiceId: e.target.value })} aria-label="Voice">
              {voices.length === 0 && <option value="">Loading voices…</option>}
              {voices.map((v) => <option key={v.id} value={v.id}>{v.name} · {v.language}</option>)}
            </select>
            <label>
              <input type="range" min={0.5} max={2.5} step={0.05} value={settings?.speed ?? 1} onChange={(e) => save({ speed: Number(e.target.value) })} aria-label="Speed" style={{ width: 80 }} />
              <span className="num">{(settings?.speed ?? 1).toFixed(2)}×</span>
            </label>
          </div>
        }
        right={
          <>
            <button className="icon" onClick={fixPronunciation} title="Select a word first"><Glyph name="edit" /> Fix pronunciation…</button>
            <button className="icon" onClick={() => { setText(""); setTitle(null); }} disabled={!text}><Glyph name="close" /> Clear</button>
          </>
        }
      />
      <Paper label={title ?? "Reader"}>
        <textarea
          className="sheet"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Type or paste text here, then press ⌘↩ to speak. Other apps can also send text here."
          autoFocus
          spellCheck={false}
        />
      </Paper>
      <div className="reader-foot">
        <span>{title ?? "Reader"} · {text.length.toLocaleString()} chars</span>
        <span>
          {problem
            ? <span className="red"><Glyph name="warning" /> {problem}</span>
            : <span className="status-word">{state}{status && status.progress.total > 0 ? ` ${status.progress.synthesized}/${status.progress.total}` : ""}</span>}
        </span>
      </div>
    </div>
  );
}
