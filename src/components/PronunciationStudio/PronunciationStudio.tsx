import { useEffect, useState } from "react";
import { open as pickFile, save as pickSave } from "@tauri-apps/plugin-dialog";
import { Empty, Glyph, Tag, Toggle, Zone } from "@overtone-foundry/app-kit";
import { hub, errorMessage, useRules } from "../../services/hub";
import type { Rule } from "../../types";
import { ConfirmButton, Head } from "../bits";

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

  return (
    <div>
      <Head
        title="Pronunciation Studio"
        actions={<>
          {!draft && <button className="primary" onClick={() => setDraft(blankRule())}><Glyph name="plus" /> Add rule</button>}
          <button className="quiet" onClick={doImport}>Import…</button>
          <button className="quiet" onClick={doExport} disabled={rules.length === 0}>Export…</button>
        </>}
      >
        A <strong>pronunciation rule</strong> is “written as → spoken as”. Teach Speech Hub how to say a name or a term once, and the fix applies to every app, Source, and the <code>speak</code> command. What you see in the Reader never changes; only what the engine hears.
      </Head>

      <Zone title="Try it" sub="live preview">
        <div className="row field-row">
          <div className="field"><label><strong>Sample</strong></label><input type="text" value={sample} onChange={(e) => setSample(e.target.value)} /></div>
          <button onClick={speakSample}><Glyph name="play" /> Speak</button>
        </div>
        <div className="facts"><span className="label">Engine hears</span><span>{spoken}</span></div>
      </Zone>

      {draft && (
        <Zone className="editing" title={draft.id ? "Edit rule" : "New rule"}>
          <div className="row field-row">
            <div className="field"><label><strong>Written as</strong> (what appears in the text)</label><input type="text" autoFocus value={draft.matchText} onChange={(e) => setDraft({ ...draft, matchText: e.target.value })} placeholder="LangGraph" onKeyDown={(e) => e.key === "Enter" && save()} /></div>
            <span className="disp dim">→</span>
            <div className="field"><label><strong>Spoken as</strong> (what the engine says)</label><input type="text" value={draft.replacement} onChange={(e) => setDraft({ ...draft, replacement: e.target.value })} placeholder="Lang Graph" onKeyDown={(e) => e.key === "Enter" && save()} /></div>
          </div>
          <div className="row">
            <span className="dim">Whole words only</span><Toggle label="Whole words only" on={draft.wholeWord} onChange={(v) => setDraft({ ...draft, wholeWord: v })} />
            <span className="dim">Match case</span><Toggle label="Match case" on={draft.caseSensitive} onChange={(v) => setDraft({ ...draft, caseSensitive: v })} />
          </div>
          <div className="field"><label><strong>Note</strong> (optional)</label><input type="text" value={draft.note} onChange={(e) => setDraft({ ...draft, note: e.target.value })} /></div>
          <p className="dim">Tip: spell it the way you'd say it: “Lang Graph”, “sequel”, “en jin ex”. Hyphens add small pauses.</p>
          <div className="row">
            <button className="primary" onClick={save} disabled={!draft.matchText.trim()}>{draft.matchText.trim() ? "Save rule" : "Save · needs the written form"}</button>
            <button className="quiet" onClick={() => setDraft(null)}>Cancel</button>
          </div>
        </Zone>
      )}

      {msg && <p className="hint-line">{msg}</p>}
      <Zone title="Rules" sub={`${rules.length} in the dictionary`}>
        {rules.length === 0
          ? <Empty word="No rules">Add one, or select a word in the Reader and choose “Fix pronunciation”.</Empty>
          : (
            <ul className="rows actions">
              {rules.map((r) => (
                <li key={r.id} className={r.enabled ? "" : "off"}>
                  <span className="main">
                    <button className="link" onClick={() => setDraft({ ...r })}><code>{r.matchText}</code></button>
                    {" "}<span className="dim">→</span>{" "}
                    <strong>{r.replacement || <em className="dim">(silence)</em>}</strong>
                    {!r.wholeWord && <Tag>partial</Tag>}
                    {r.caseSensitive && <Tag>case</Tag>}
                    {!r.enabled && <Tag>off</Tag>}
                    {r.note && <span className="sub">{r.note}</span>}
                  </span>
                  <span className="act">
                    <Toggle label={`Rule ${r.matchText}`} on={r.enabled} onChange={() => toggle(r)} />
                    <ConfirmButton onConfirm={() => del(r)} title="Delete this rule">Delete</ConfirmButton>
                  </span>
                </li>
              ))}
            </ul>
          )}
      </Zone>
    </div>
  );
}
