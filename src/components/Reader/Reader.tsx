import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { hub, useHubStatus, useSettings, useVoices, errorMessage, STATE_LABEL } from "../../services/hub";
import type { ReaderText } from "../../types";
import { Button } from "../ui";

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
  const onKey = (e: React.KeyboardEvent) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "Enter") { e.preventDefault(); speak(); }
    if (e.key === "Escape" && busy) { hub.stop(); }
  };

  return (
    <div className="flex h-full flex-col" onKeyDown={onKey}>
      <div className="flex items-center gap-2 border-b border-[var(--border)] px-3 py-2">
        {busy && status?.state !== "paused" ? (
          <Button onClick={() => hub.pause()}>❙❙ Pause</Button>
        ) : status?.state === "paused" ? (
          <Button variant="primary" onClick={() => hub.resume()}>▶ Resume</Button>
        ) : (
          <Button variant="primary" onClick={speak} disabled={!text.trim()}>▶ Speak</Button>
        )}
        <Button onClick={() => hub.stop()} disabled={!busy}>■ Stop</Button>
        <span className="mx-1 h-5 w-px bg-[var(--border)]" />
        <select value={settings?.voiceId ?? ""} onChange={(e) => save({ voiceId: e.target.value })}
          className="max-w-[180px] rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2 py-1 text-[13px]">
          {voices.length === 0 && <option value="">Loading voices…</option>}
          {voices.map((v) => <option key={v.id} value={v.id}>{v.name} · {v.language}</option>)}
        </select>
        <label className="flex items-center gap-1 text-[12px] text-[var(--muted)]">
          <input type="range" min={0.5} max={2.5} step={0.05} value={settings?.speed ?? 1} onChange={(e) => save({ speed: Number(e.target.value) })} className="w-20 accent-[var(--accent)]" />
          {(settings?.speed ?? 1).toFixed(2)}×
        </label>
        <span className="flex-1" />
        <Button variant="ghost" onClick={() => { setText(""); setTitle(null); }} disabled={!text}>Clear</Button>
      </div>
      <textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Type or paste text here, then press ⌘↩ to speak. Other apps can also send text here."
        className="flex-1 resize-none bg-transparent p-4 text-[17px] leading-relaxed outline-none"
        autoFocus
        spellCheck={false}
      />
      <div className="flex items-center justify-between border-t border-[var(--border)] px-3 py-1.5 text-[12px] text-[var(--muted)]">
        <span>{title ?? "Reader"} · {text.length.toLocaleString()} chars</span>
        <span className={err || status?.state === "error" ? "text-[var(--danger)]" : ""}>
          {err ?? status?.error ?? (status ? STATE_LABEL[status.state] : "")}
          {status && status.progress.total > 0 && ` ${status.progress.synthesized}/${status.progress.total}`}
        </span>
      </div>
    </div>
  );
}
