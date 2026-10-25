import { useEffect, useState } from "react";
import { open as pickFile, save as pickSave } from "@tauri-apps/plugin-dialog";
import { hub, errorMessage, useRules } from "../../services/hub";
import type { Rule } from "../../types";
import { Badge, Button, Card, H1, Lead, Toggle } from "../ui";

const blankRule = (matchText = ""): Rule => ({ id: "", matchText, replacement: "", wholeWord: true, caseSensitive: false, enabled: true, note: "", createdAt: 0 });

/** One dictionary for everything: the written form, and what the engine should say instead. */
export default function PronunciationStudio() {
  const { rules } = useRules();
  const [draft, setDraft] = useState<Rule | null>(null);
  const [sample, setSample] = useState("Say LangGraph, Kubernetes, and nginx with a GPU.");
  const [spoken, setSpoken] = useState<string>("");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => {
    const onNav = (e: Event) => {
      const d = (e as CustomEvent).detail;
      if (d && typeof d === "object" && d.kind === "pronunciation-prefill") { setDraft(blankRule(String(d.text ?? ""))); setSample(String(d.context ?? sample)); }
    };
    window.addEventListener("speechhub:navigate", onNav);
    return () => window.removeEventListener("speechhub:navigate", onNav);
  }, [sample]);

  useEffect(() => {
    const t = setTimeout(() => {
      hub.rules.preview(sample, draft && draft.matchText.trim() ? draft : undefined).then((p) => setSpoken(p.spoken)).catch(() => {});
    }, 150);
    return () => clearTimeout(t);
  }, [sample, draft, rules]);

  const save = async () => {
    if (!draft) return;
    try { await hub.rules.save(draft); setDraft(null); setMsg(null); } catch (e) { setMsg(errorMessage(e)); }
  };
  const toggle = (r: Rule) => hub.rules.save({ ...r, enabled: !r.enabled }).catch((e) => setMsg(errorMessage(e)));
  const del = (r: Rule) => hub.rules.delete(r.id).catch((e) => setMsg(errorMessage(e)));
  const speakSample = () => hub.speak(sample, { title: "Pronunciation preview" }).catch((e) => setMsg(errorMessage(e)));
  const doExport = async () => {
    try { const p = await pickSave({ defaultPath: "speech-hub-pronunciation.json", filters: [{ name: "JSON", extensions: ["json"] }] }); if (p) { await hub.rules.exportTo(p); setMsg(`Exported ${rules.length} rules.`); } } catch (e) { setMsg(errorMessage(e)); }
  };
  const doImport = async () => {
    try { const p = await pickFile({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] }); if (p && !Array.isArray(p)) { const n = await hub.rules.importFrom(p); setMsg(`Imported ${n} rules.`); } } catch (e) { setMsg(errorMessage(e)); }
  };
  const input = "rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5 text-[14px]";

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-start justify-between gap-4">
        <div><H1>Pronunciation Studio</H1><Lead>A <strong>pronunciation rule</strong> is “written as → spoken as”. Teach Speech Hub how to say a name or a term once, and the fix applies to every app, Source, and the <code>speak</code> command. What you see in the Reader never changes; only what the engine hears.</Lead></div>
        <div className="flex gap-2 shrink-0">
          <Button variant="ghost" onClick={doImport}>Import…</Button>
          <Button variant="ghost" onClick={doExport} disabled={rules.length === 0}>Export…</Button>
          {!draft && <Button variant="primary" onClick={() => setDraft(blankRule())}>＋ Add rule</Button>}
        </div>
      </div>

      <Card className="mb-3">
        <div className="text-[13px] text-[var(--muted)] mb-1">Try it</div>
        <div className="flex gap-2">
          <input value={sample} onChange={(e) => setSample(e.target.value)} className={`${input} flex-1`} />
          <Button onClick={speakSample}>▶ Speak</Button>
        </div>
        <div className="mt-2 text-[13px]"><span className="text-[var(--muted)]">Engine hears: </span><span className="select-text">{spoken}</span></div>
      </Card>

      {draft && (
        <Card className="mb-3 border-[var(--accent)]/60">
          <div className="grid gap-2 sm:grid-cols-[1fr_auto_1fr]">
            <input autoFocus value={draft.matchText} onChange={(e) => setDraft({ ...draft, matchText: e.target.value })} placeholder="Written as… (e.g. LangGraph)" className={input} onKeyDown={(e) => e.key === "Enter" && save()} />
            <span className="self-center text-[var(--muted)]">→</span>
            <input value={draft.replacement} onChange={(e) => setDraft({ ...draft, replacement: e.target.value })} placeholder="Spoken as… (e.g. Lang Graph)" className={input} onKeyDown={(e) => e.key === "Enter" && save()} />
          </div>
          <div className="mt-2 flex flex-wrap items-center gap-4 text-[13px]">
            <label className="flex items-center gap-2">Whole words only <Toggle checked={draft.wholeWord} onChange={(v) => setDraft({ ...draft, wholeWord: v })} /></label>
            <label className="flex items-center gap-2">Match case <Toggle checked={draft.caseSensitive} onChange={(v) => setDraft({ ...draft, caseSensitive: v })} /></label>
            <input value={draft.note} onChange={(e) => setDraft({ ...draft, note: e.target.value })} placeholder="Note (optional)" className={`${input} flex-1 min-w-[160px] text-[13px]`} />
            <span className="flex-1" />
            <Button variant="ghost" onClick={() => setDraft(null)}>Cancel</Button>
            <Button variant="primary" onClick={save} disabled={!draft.matchText.trim()}>Save</Button>
          </div>
          <p className="text-[12px] text-[var(--muted)] mt-2">Tip: spell it the way you'd say it: “Lang Graph”, “sequel”, “en jin ex”. Hyphens add small pauses.</p>
        </Card>
      )}

      {msg && <p className="text-[13px] text-[var(--muted)] mb-2">{msg}</p>}
      <div className="flex-1 min-h-0 overflow-auto rounded-xl border border-[var(--border)] bg-[var(--panel)] divide-y divide-[var(--border)]">
        {rules.length === 0 && <div className="p-4 text-[13px] text-[var(--muted)]">No rules yet. Add one, or select a word in the Reader and choose “Fix pronunciation”.</div>}
        {rules.map((r) => (
          <div key={r.id} className={`flex items-center gap-3 px-3 py-2 ${r.enabled ? "" : "opacity-55"}`}>
            <button className="flex-1 min-w-0 text-left" onClick={() => setDraft({ ...r })}>
              <div className="flex items-center gap-2 flex-wrap">
                <span className="font-medium font-mono text-[14px]">{r.matchText}</span>
                <span className="text-[var(--muted)]">→</span>
                <span className="font-medium">{r.replacement || <em className="text-[var(--muted)]">(silence)</em>}</span>
                {!r.wholeWord && <Badge>partial</Badge>}
                {r.caseSensitive && <Badge>case</Badge>}
              </div>
              {r.note && <div className="text-[12px] text-[var(--muted)] truncate">{r.note}</div>}
            </button>
            <Toggle checked={r.enabled} onChange={() => toggle(r)} />
            <Button variant="ghost" onClick={() => del(r)} aria-label="Delete">✕</Button>
          </div>
        ))}
      </div>
    </div>
  );
}
