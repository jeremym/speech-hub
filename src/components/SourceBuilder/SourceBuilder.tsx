import { useEffect, useMemo, useState } from "react";
import { open as pickFile, save as pickSave } from "@tauri-apps/plugin-dialog";
import { hub, errorMessage, useHubStatus, useSources, useVoices } from "../../services/hub";
import type { Source, SourceKind, SourceTemplate, TestResult } from "../../types";
import { Badge, Button, Card, Field, H1, Lead, Select, Toggle } from "../ui";

const KIND_LABEL: Record<SourceKind["type"], string> = { text: "Saved text", file: "File", command: "Command", shortcut: "Apple Shortcut", http: "URL" };

function blank(template: SourceTemplate): Source {
  return {
    id: "", name: template.id === "text" ? "" : template.name, description: "", icon: template.icon, kind: template.kind,
    voiceId: null, speed: null, action: template.action, enabled: true, trusted: true, favorite: false,
    timeoutSecs: 15, runCount: 0, lastRunAt: null, createdAt: 0,
  };
}

export default function SourceBuilder({ initialNew }: { initialNew?: boolean }) {
  const { sources } = useSources();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [draft, setDraft] = useState<Source | null>(null);
  const [templates, setTemplates] = useState<SourceTemplate[]>([]);
  const [mode, setMode] = useState<"list" | "templates" | "edit">(initialNew ? "templates" : "list");
  const [msg, setMsg] = useState<string | null>(null);

  useEffect(() => { hub.sources.templates().then(setTemplates).catch(() => {}); }, []);
  useEffect(() => {
    const onNew = (e: Event) => { if ((e as CustomEvent).detail === "sources-new") { setMode("templates"); } };
    window.addEventListener("speechhub:navigate", onNew);
    return () => window.removeEventListener("speechhub:navigate", onNew);
  }, []);

  const startEdit = (s: Source) => { setSelectedId(s.id); setDraft({ ...s }); setMode("edit"); setMsg(null); };
  const startNew = (t: SourceTemplate) => { setSelectedId(null); setDraft(blank(t)); setMode("edit"); setMsg(null); };

  const doExport = async () => {
    try {
      const path = await pickSave({ defaultPath: "speech-hub-sources.json", filters: [{ name: "JSON", extensions: ["json"] }] });
      if (!path) return;
      await hub.sources.exportTo(path);
      setMsg(`Exported ${sources.length} Sources to ${path}.`);
    } catch (e) { setMsg(errorMessage(e)); }
  };
  const doImport = async () => {
    try {
      const path = await pickFile({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
      if (!path || Array.isArray(path)) return;
      const n = await hub.sources.importFrom(path);
      setMsg(`Imported ${n} Sources. Command and Shortcut Sources need your approval before they run.`);
    } catch (e) { setMsg(errorMessage(e)); }
  };

  if (mode === "edit" && draft) {
    return <Editor draft={draft} setDraft={setDraft} isNew={!selectedId} onBack={() => { setMode("list"); setDraft(null); }} />;
  }
  if (mode === "templates") {
    return (
      <div>
        <H1>New Source</H1>
        <Lead>Pick where the text should come from. You can change every detail afterwards.</Lead>
        <div className="grid gap-3 sm:grid-cols-2">
          {templates.map((t) => (
            <Card key={t.id} onClick={() => startNew(t)}>
              <div className="flex gap-3">
                <span className="text-2xl leading-none">{t.icon}</span>
                <div>
                  <div className="font-semibold">{t.name}</div>
                  <p className="text-[13px] text-[var(--muted)] mt-0.5">{t.description}</p>
                </div>
              </div>
            </Card>
          ))}
        </div>
        <div className="mt-4"><Button variant="ghost" onClick={() => setMode("list")}>Cancel</Button></div>
      </div>
    );
  }
  return (
    <div className="flex h-full flex-col">
      <div className="flex items-start justify-between gap-4">
        <div><H1>Sources</H1><Lead>Where Speech Hub gets text: files, commands, Shortcuts, URLs. Run them from Quick Speak (⌥⇧⎋), the menu bar, or <code>speak run</code>.</Lead></div>
        <div className="flex gap-2 shrink-0">
          <Button variant="ghost" onClick={doImport}>Import…</Button>
          <Button variant="ghost" onClick={doExport} disabled={sources.length === 0}>Export…</Button>
          <Button variant="primary" onClick={() => setMode("templates")}>＋ New Source</Button>
        </div>
      </div>
      {msg && <p className="text-[13px] text-[var(--muted)] mb-2">{msg}</p>}
      {sources.length === 0 ? (
        <Card><p className="text-[var(--muted)]">No Sources yet. Try <strong>New Source → Current clipboard</strong>: it takes about thirty seconds.</p></Card>
      ) : (
        <div className="flex-1 min-h-0 overflow-auto rounded-xl border border-[var(--border)] bg-[var(--panel)] divide-y divide-[var(--border)]">
          {sources.map((s) => (
            <div key={s.id} className="flex items-center gap-3 px-3 py-2">
              <span className="text-xl w-7 text-center">{s.icon}</span>
              <button className="flex-1 min-w-0 text-left" onClick={() => startEdit(s)}>
                <div className="flex items-center gap-2">
                  <span className={`font-medium truncate ${s.enabled ? "" : "opacity-50"}`}>{s.name}</span>
                  <Badge>{KIND_LABEL[s.kind.type]}</Badge>
                  {s.favorite && <Badge tone="accent">★</Badge>}
                  {!s.trusted && (s.kind.type === "command" || s.kind.type === "shortcut") && <Badge tone="warn">Needs approval</Badge>}
                  {!s.enabled && <Badge>Disabled</Badge>}
                </div>
                <div className="text-[12px] text-[var(--muted)] truncate">{summary(s)}</div>
              </button>
              <Button variant="ghost" onClick={() => hub.sources.run(s.id).catch((e) => setMsg(errorMessage(e)))} disabled={!s.enabled}>▶ Run</Button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export function summary(s: Source): string {
  const k = s.kind;
  switch (k.type) {
    case "text": return `Text · ${k.text.length} chars`;
    case "file": return `Reads ${k.path || "(no file chosen)"}`;
    case "command": return k.shell ? `Shell: ${k.program}` : `Runs ${k.program} ${k.args.join(" ")}`.trim();
    case "shortcut": return `Shortcut “${k.name || "(none chosen)"}”`;
    case "http": return `Fetches ${k.url || "(no URL)"}`;
  }
}

// ---------------------------------------------------------------- editor

function Editor({ draft, setDraft, isNew, onBack }: { draft: Source; setDraft: (s: Source) => void; isNew: boolean; onBack: () => void }) {
  const status = useHubStatus();
  const voices = useVoices(status?.engine.id);
  const [test, setTest] = useState<TestResult | null>(null);
  const [testing, setTesting] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [shortcuts, setShortcuts] = useState<string[] | null>(null);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const k = draft.kind;
  const setKind = (kind: SourceKind) => setDraft({ ...draft, kind });
  const executable = k.type === "command" || k.type === "shortcut";

  useEffect(() => {
    if (k.type === "shortcut" && shortcuts === null) hub.sources.shortcuts().then(setShortcuts).catch(() => setShortcuts([]));
  }, [k.type, shortcuts]);

  const canSave = useMemo(() => {
    if (!draft.name.trim() && k.type !== "text") return false;
    switch (k.type) {
      case "text": return k.text.trim().length > 0;
      case "file": return k.path.trim().length > 0;
      case "command": return k.program.trim().length > 0;
      case "shortcut": return k.name.trim().length > 0;
      case "http": return /^https?:\/\//.test(k.url.trim());
    }
  }, [draft, k]);

  const runTest = async () => {
    setTesting(true); setErr(null); setTest(null);
    try { setTest(await hub.sources.test(draft)); } catch (e) { setErr(errorMessage(e)); } finally { setTesting(false); }
  };
  const save = async () => {
    setErr(null);
    try {
      const name = draft.name.trim() || (k.type === "text" ? (k.text.trim().slice(0, 40) || "Snippet") : "Untitled Source");
      // Saving in the Builder is the act of approving: the user typed the command.
      await hub.sources.save({ ...draft, name }, true);
      onBack();
    } catch (e) { setErr(errorMessage(e)); }
  };
  const del = async () => { try { await hub.sources.delete(draft.id); onBack(); } catch (e) { setErr(errorMessage(e)); } };
  const dup = async () => { try { await hub.sources.duplicate(draft.id); onBack(); } catch (e) { setErr(errorMessage(e)); } };
  const chooseFile = async () => {
    const p = await pickFile({ multiple: false, filters: [{ name: "Text", extensions: ["txt", "md", "markdown", "text", "log", "csv", "json"] }] });
    if (p && !Array.isArray(p) && k.type === "file") setKind({ ...k, path: p });
  };
  const chooseDir = async () => {
    const p = await pickFile({ directory: true, multiple: false });
    if (p && !Array.isArray(p) && k.type === "command") setKind({ ...k, cwd: p });
  };
  const input = "w-full rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5 font-mono text-[13px]";

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-3 mb-3">
        <Button variant="ghost" onClick={onBack}>‹ Sources</Button>
        <H1>{isNew ? "New Source" : draft.name}</H1>
        <Badge>{KIND_LABEL[k.type]}</Badge>
      </div>
      <div className="flex-1 min-h-0 overflow-auto space-y-4 pr-1">
        {!draft.trusted && executable && (
          <Card className="border-[var(--danger)]/50">
            <div className="font-medium">This Source runs a program and hasn't been approved.</div>
            <p className="text-[13px] text-[var(--muted)] mt-1">It came from an import or a link. Check what it runs below. Saving approves it.</p>
          </Card>
        )}
        <Card>
          <div className="flex gap-3 items-start">
            <input value={draft.icon} onChange={(e) => setDraft({ ...draft, icon: e.target.value })} className="w-14 text-center text-2xl rounded-lg border border-[var(--border)] bg-[var(--panel)] py-1" aria-label="Icon" />
            <div className="flex-1 space-y-2">
              <input value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} placeholder="Name (what you'll search for in Quick Speak)" className="w-full rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5 text-[16px] font-medium" autoFocus />
              <input value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} placeholder="Description (optional)" className="w-full rounded-lg border border-[var(--border)] bg-[var(--panel)] px-3 py-1.5 text-[13px]" />
            </div>
          </div>
        </Card>

        <Card>
          <div className="font-medium mb-2">Where the text comes from</div>
          {k.type === "text" && (
            <textarea value={k.text} onChange={(e) => setKind({ ...k, text: e.target.value })} rows={6} placeholder="Type or paste the text to save…" className={`${input} font-sans text-[15px] resize-y`} />
          )}
          {k.type === "file" && (
            <div className="flex gap-2">
              <input value={k.path} onChange={(e) => setKind({ ...k, path: e.target.value })} placeholder="/Users/you/Documents/notes.md" className={input} />
              <Button onClick={chooseFile}>Choose…</Button>
            </div>
          )}
          {k.type === "command" && (
            <div className="space-y-2">
              <label className="block text-[13px] text-[var(--muted)]">{k.shell ? "Shell command line (runs with /bin/zsh -c)" : "Program (full path)"}
                <input value={k.program} onChange={(e) => setKind({ ...k, program: e.target.value })} placeholder={k.shell ? "pbpaste | head -n 20" : "/usr/bin/pbpaste"} className={`${input} mt-1`} />
              </label>
              {!k.shell && (
                <label className="block text-[13px] text-[var(--muted)]">Arguments, one per line (passed exactly; no shell expansion)
                  <textarea value={k.args.join("\n")} onChange={(e) => setKind({ ...k, args: e.target.value.split("\n").filter((a, i, arr) => a.length > 0 || i < arr.length - 1) })} rows={3} className={`${input} mt-1 resize-y`} />
                </label>
              )}
              <div className="flex items-center gap-2 text-[13px] text-[var(--muted)]">
                <input value={k.cwd ?? ""} onChange={(e) => setKind({ ...k, cwd: e.target.value || null })} placeholder="Working folder (optional)" className={input} />
                <Button onClick={chooseDir}>Choose…</Button>
              </div>
              <label className="flex items-center justify-between text-[13px] pt-1">
                <span>Run as a shell command <span className="text-[var(--muted)]">(pipes, globs, $VARS; advanced)</span></span>
                <Toggle checked={!!k.shell} onChange={(v) => setKind({ ...k, shell: v })} />
              </label>
            </div>
          )}
          {k.type === "shortcut" && (
            <div className="space-y-2">
              {shortcuts && shortcuts.length > 0 ? (
                <select value={k.name} onChange={(e) => setKind({ ...k, name: e.target.value })} className={input}>
                  <option value="">Choose a Shortcut…</option>
                  {shortcuts.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              ) : (
                <input value={k.name} onChange={(e) => setKind({ ...k, name: e.target.value })} placeholder="Exact Shortcut name" className={input} />
              )}
              <input value={k.input ?? ""} onChange={(e) => setKind({ ...k, input: e.target.value || null })} placeholder="Text input to pass to the Shortcut (optional)" className={input} />
              <p className="text-[12px] text-[var(--muted)]">The Shortcut must end with a text result. Tip: add a “Text” action as the last step.</p>
            </div>
          )}
          {k.type === "http" && (
            <div className="space-y-2">
              <input value={k.url} onChange={(e) => setKind({ ...k, url: e.target.value })} placeholder="https://example.com/status.txt" className={input} />
              <input value={k.json_path ?? ""} onChange={(e) => setKind({ ...k, json_path: e.target.value || null })} placeholder="JSON field path, e.g. items.0.title (optional)" className={input} />
              <p className="text-[12px] text-[var(--muted)]">Fetched only when this Source runs. HTML pages are reduced to their text.</p>
            </div>
          )}
        </Card>

        <Card>
          <Field label="When it runs" hint="Speak right away, or open the text in the Reader first.">
            <Select value={draft.action} onChange={(v) => setDraft({ ...draft, action: v })} options={[{ value: "speak", label: "Speak immediately" }, { value: "open", label: "Open in Reader" }]} />
          </Field>
          <Field label="Voice" hint="Leave on Default to follow your global voice.">
            <select value={draft.voiceId ?? ""} onChange={(e) => setDraft({ ...draft, voiceId: e.target.value || null })} className="rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2.5 py-1.5 text-[14px] max-w-[220px]">
              <option value="">Default</option>
              {voices.map((v) => <option key={v.id} value={v.id}>{v.name} · {v.language}</option>)}
            </select>
          </Field>
          <Field label="Speed" hint={draft.speed ? `${draft.speed.toFixed(2)}×` : "Default"}>
            <input type="range" min={0.5} max={2.5} step={0.05} value={draft.speed ?? 1} onChange={(e) => setDraft({ ...draft, speed: Number(e.target.value) })} className="accent-[var(--accent)]" />
            {draft.speed !== null && <Button variant="ghost" onClick={() => setDraft({ ...draft, speed: null })}>Reset</Button>}
          </Field>
          <Field label="Favorite" hint="Shown first in Quick Speak."><Toggle checked={draft.favorite} onChange={(v) => setDraft({ ...draft, favorite: v })} /></Field>
          <Field label="Enabled"><Toggle checked={draft.enabled} onChange={(v) => setDraft({ ...draft, enabled: v })} /></Field>
          <button className="text-[13px] text-[var(--accent)] pt-2" onClick={() => setShowAdvanced((a) => !a)}>{showAdvanced ? "Hide" : "Show"} advanced</button>
          {showAdvanced && (
            <Field label="Time limit" hint="Commands, Shortcuts and URLs are stopped after this many seconds.">
              <input type="number" min={1} max={120} value={draft.timeoutSecs} onChange={(e) => setDraft({ ...draft, timeoutSecs: Math.min(120, Math.max(1, Number(e.target.value) || 15)) })} className="w-20 rounded-lg border border-[var(--border)] bg-[var(--panel)] px-2 py-1 text-right" />
              <span className="text-[13px] text-[var(--muted)]">s</span>
            </Field>
          )}
        </Card>

        {(test || err || testing) && (
          <Card>
            <div className="font-medium flex items-center gap-2">Test result {testing && <Badge>Running…</Badge>} {test && <Badge tone="ok">{test.chars.toLocaleString()} chars · {test.fetchMs} ms</Badge>}</div>
            {err && <p className="text-[13px] text-[var(--danger)] mt-1 whitespace-pre-wrap">{err}</p>}
            {test && <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap text-[13px] select-text">{test.preview}{test.truncated ? "\n…" : ""}</pre>}
          </Card>
        )}
      </div>
      <div className="flex items-center gap-2 border-t border-[var(--border)] pt-3 mt-3">
        <Button onClick={runTest} disabled={!canSave || testing}>Test</Button>
        <span className="flex-1" />
        {!isNew && <Button variant="ghost" onClick={dup}>Duplicate</Button>}
        {!isNew && <Button variant="danger" onClick={del}>Delete</Button>}
        <Button variant="primary" onClick={save} disabled={!canSave}>{draft.trusted || !executable ? "Save" : "Approve & Save"}</Button>
      </div>
    </div>
  );
}
