import { useEffect, useMemo, useState } from "react";
import { open as pickFile, save as pickSave } from "@tauri-apps/plugin-dialog";
import { Card, Empty, Glyph, Tag, Toggle, Zone, type GlyphName } from "@overtone-foundry/app-kit";
import { hub, errorMessage, useHubStatus, useSettings, useSources, useVoices } from "../../services/hub";
import type { Source, SourceKind, SourceTemplate, TestResult } from "../../types";
import { formatShortcut } from "../../lib/shortcut";
import { ConfirmButton, Head, Line } from "../bits";

const KIND_LABEL: Record<SourceKind["type"], string> = { text: "Saved text", file: "File", command: "Command", shortcut: "Apple Shortcut", http: "URL" };
const KIND_GLYPH: Record<SourceKind["type"], GlyphName> = { text: "text", file: "file", command: "program", shortcut: "sparkles", http: "globe" };

function blank(template: SourceTemplate): Source {
  return {
    id: "", name: template.id === "text" ? "" : template.name, description: "", icon: template.icon, kind: template.kind,
    voiceId: null, speed: null, action: template.action, enabled: true, trusted: true, favorite: false,
    timeoutSecs: 15, runCount: 0, lastRunAt: null, createdAt: 0,
  };
}

/** The Source's kind as a tag: a URL leaves this Mac, so it is amber; programs are marked too. */
function KindTag({ type }: { type: SourceKind["type"] }) {
  return <Tag tone={type === "http" ? "amber" : undefined}>{KIND_LABEL[type]}</Tag>;
}

export default function SourceBuilder({ initialNew }: { initialNew?: boolean }) {
  const { sources } = useSources();
  const { settings } = useSettings();
  const quickKeys = formatShortcut(settings?.hotkeys.quickSpeak ?? "Alt+Shift+Escape");
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
        <Head title="New Source" actions={<button className="quiet" onClick={() => setMode("list")}><Glyph name="back" /> Sources</button>}>
          Pick where the text should come from. Each choice is a starting point; you can change every detail afterwards.
        </Head>
        <div className="cards">
          {templates.map((t) => (
            <Card key={t.id} title={t.name} glyph={KIND_GLYPH[t.kind.type]} onClick={() => startNew(t)} tags={t.kind.type === "http" ? [{ label: "network", tone: "amber" }] : undefined}>
              {t.description}
            </Card>
          ))}
        </div>
      </div>
    );
  }
  return (
    <div>
      <Head
        title="Sources"
        actions={<>
          <button className="primary" onClick={() => setMode("templates")}><Glyph name="plus" /> New Source</button>
          <button className="quiet" onClick={doImport}>Import…</button>
          <button className="quiet" onClick={doExport} disabled={sources.length === 0}>Export…</button>
        </>}
      >
        A <strong>Source</strong> is a saved place to get text from: your clipboard, a file, a command, an Apple Shortcut, or a web address. Run one from the Quick Speak panel <kbd>{quickKeys}</kbd>, the menu-bar icon, or <code>speak run</code>.
      </Head>
      {msg && <p className="hint-line">{msg}</p>}
      {sources.length === 0 ? (
        <Empty word="No Sources">Try <strong>New Source → Current clipboard</strong>: it takes about thirty seconds. Then copy any text, press <kbd>{quickKeys}</kbd>, and pick it.</Empty>
      ) : (
        <ul className="rows actions">
          {sources.map((s) => (
            <li key={s.id} className={s.enabled ? "" : "off"}>
              <span className="src-icon" aria-hidden="true">{s.icon}</span>
              <span className="main">
                <button className="link" onClick={() => startEdit(s)}>{s.name}</button>
                <KindTag type={s.kind.type} />
                {s.favorite && <Tag>favorite</Tag>}
                {!s.trusted && (s.kind.type === "command" || s.kind.type === "shortcut") && <Tag tone="amber">needs approval</Tag>}
                {!s.enabled && <Tag>off</Tag>}
                <span className="sub">{summary(s)}</span>
              </span>
              <span className="act">
                <button className="quiet" onClick={() => hub.sources.run(s.id).catch((e) => setMsg(errorMessage(e)))} disabled={!s.enabled}><Glyph name="play" /> Run</button>
              </span>
            </li>
          ))}
        </ul>
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
  const missing = !draft.name.trim() && k.type !== "text" ? "a name" : k.type === "text" ? "the text" : k.type === "file" ? "a file" : k.type === "command" ? "a program" : k.type === "shortcut" ? "a Shortcut" : "a web address";

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
  const approve = !draft.trusted && executable;

  return (
    <div>
      <Head title={isNew ? "New Source" : draft.name || "Untitled Source"} actions={<button className="quiet" onClick={onBack}><Glyph name="back" /> Sources</button>}>
        <KindTag type={k.type} />{approve && <Tag tone="amber">needs approval</Tag>}
      </Head>
      {approve && (
        <div className="notice"><Glyph name="warning" /> This Source runs a program and hasn't been approved. It came from an import or a link. Check what it runs below; saving approves it.</div>
      )}
      <Zone title="Name" sub="what you'll search for in Quick Speak">
        <div className="row field-row">
          <div className="field" style={{ flex: "none", minWidth: 0, width: "5em" }}><label><strong>Icon</strong></label><input type="text" value={draft.icon} onChange={(e) => setDraft({ ...draft, icon: e.target.value })} aria-label="Icon" style={{ textAlign: "center" }} /></div>
          <div className="field"><label><strong>Name</strong></label><input type="text" value={draft.name} onChange={(e) => setDraft({ ...draft, name: e.target.value })} autoFocus /></div>
        </div>
        <div className="field"><label><strong>Description</strong> (optional)</label><input type="text" value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} /></div>
      </Zone>

      <Zone title="Where the text comes from" sub={KIND_LABEL[k.type]} className={approve ? "amber-rim" : ""}>
        {k.type === "text" && (
          <div className="field"><label><strong>Text</strong></label><textarea value={k.text} onChange={(e) => setKind({ ...k, text: e.target.value })} rows={6} placeholder="Type or paste the text to save…" /></div>
        )}
        {k.type === "file" && (
          <div className="row field-row">
            <div className="field"><label><strong>File</strong></label><input type="text" value={k.path} onChange={(e) => setKind({ ...k, path: e.target.value })} placeholder="/Users/you/Documents/notes.md" /></div>
            <button onClick={chooseFile}>Choose…</button>
          </div>
        )}
        {k.type === "command" && (
          <>
            <div className="field">
              <label><strong>{k.shell ? "Shell command line" : "Program"}</strong> ({k.shell ? "runs with /bin/zsh -c" : "full path"})</label>
              <input type="text" value={k.program} onChange={(e) => setKind({ ...k, program: e.target.value })} placeholder={k.shell ? "pbpaste | head -n 20" : "/usr/bin/pbpaste"} />
            </div>
            {!k.shell && (
              <div className="field">
                <label><strong>Arguments</strong> (one per line; passed exactly, no shell expansion)</label>
                <textarea value={k.args.join("\n")} onChange={(e) => setKind({ ...k, args: e.target.value.split("\n").filter((a, i, arr) => a.length > 0 || i < arr.length - 1) })} rows={3} />
              </div>
            )}
            <div className="row field-row">
              <div className="field"><label><strong>Working folder</strong> (optional)</label><input type="text" value={k.cwd ?? ""} onChange={(e) => setKind({ ...k, cwd: e.target.value || null })} /></div>
              <button onClick={chooseDir}>Choose…</button>
            </div>
            <Line label="Run as a shell command" hint="Pipes, globs, $VARS. Advanced.">
              <Toggle label="Run as a shell command" on={!!k.shell} onChange={(v) => setKind({ ...k, shell: v })} />
            </Line>
          </>
        )}
        {k.type === "shortcut" && (
          <>
            <div className="field">
              <label><strong>Shortcut</strong></label>
              {shortcuts && shortcuts.length > 0 ? (
                <select value={k.name} onChange={(e) => setKind({ ...k, name: e.target.value })}>
                  <option value="">Choose a Shortcut…</option>
                  {shortcuts.map((n) => <option key={n} value={n}>{n}</option>)}
                </select>
              ) : (
                <input type="text" value={k.name} onChange={(e) => setKind({ ...k, name: e.target.value })} placeholder="Exact Shortcut name" />
              )}
            </div>
            <div className="field"><label><strong>Text input</strong> (optional, passed to the Shortcut)</label><input type="text" value={k.input ?? ""} onChange={(e) => setKind({ ...k, input: e.target.value || null })} /></div>
            <p className="dim">The Shortcut must end with a text result. Tip: add a “Text” action as the last step.</p>
          </>
        )}
        {k.type === "http" && (
          <>
            <div className="field"><label><strong>Web address</strong> (fetched only when this Source runs)</label><input type="url" value={k.url} onChange={(e) => setKind({ ...k, url: e.target.value })} placeholder="https://example.com/status.txt" /></div>
            <div className="field"><label><strong>JSON field path</strong> (optional, e.g. items.0.title)</label><input type="text" value={k.json_path ?? ""} onChange={(e) => setKind({ ...k, json_path: e.target.value || null })} /></div>
            <p className="dim">HTML pages are reduced to their text. This is the one kind of Source that uses the network.</p>
          </>
        )}
      </Zone>

      <Zone title="When it runs">
        <Line label="Then" hint="Speak right away, or open the text in the Reader first.">
          <select value={draft.action} onChange={(e) => setDraft({ ...draft, action: e.target.value as Source["action"] })}>
            <option value="speak">Speak immediately</option>
            <option value="open">Open in Reader</option>
          </select>
        </Line>
        <Line label="Voice" hint="Leave on Default to follow your global voice.">
          <select value={draft.voiceId ?? ""} onChange={(e) => setDraft({ ...draft, voiceId: e.target.value || null })}>
            <option value="">Default</option>
            {voices.map((v) => <option key={v.id} value={v.id}>{v.name} · {v.language}</option>)}
          </select>
        </Line>
        <Line label="Speed" hint={draft.speed ? `${draft.speed.toFixed(2)}×` : "Default"}>
          <input type="range" min={0.5} max={2.5} step={0.05} value={draft.speed ?? 1} onChange={(e) => setDraft({ ...draft, speed: Number(e.target.value) })} aria-label="Speed" />
          {draft.speed !== null && <button className="quiet" onClick={() => setDraft({ ...draft, speed: null })}>Reset</button>}
        </Line>
        <Line label="Favorite" hint="Shown first in Quick Speak."><Toggle label="Favorite" on={draft.favorite} onChange={(v) => setDraft({ ...draft, favorite: v })} /></Line>
        <Line label="Enabled"><Toggle label="Enabled" on={draft.enabled} onChange={(v) => setDraft({ ...draft, enabled: v })} /></Line>
        <div className="row"><button className="link" onClick={() => setShowAdvanced((a) => !a)}>{showAdvanced ? "Hide" : "Show"} advanced</button></div>
        {showAdvanced && (
          <Line label="Time limit" hint="Commands, Shortcuts and URLs are stopped after this many seconds.">
            <input type="number" min={1} max={120} value={draft.timeoutSecs} onChange={(e) => setDraft({ ...draft, timeoutSecs: Math.min(120, Math.max(1, Number(e.target.value) || 15)) })} aria-label="Seconds" />
            <span className="dim">s</span>
          </Line>
        )}
      </Zone>

      {(test || err || testing) && (
        <Zone title="Test result" aside={testing ? <Tag tone="live">running</Tag> : test ? <Tag>{test.chars.toLocaleString()} chars · {test.fetchMs} ms</Tag> : <Tag tone="bad">failed</Tag>}>
          {err && <pre className="error">{err}</pre>}
          {test && <pre style={{ maxHeight: 220, overflow: "auto" }}>{test.preview}{test.truncated ? "\n…" : ""}</pre>}
        </Zone>
      )}

      <div className="row space">
        <span className="row" style={{ margin: 0 }}>
          <button className="primary" onClick={save} disabled={!canSave}>{canSave ? (approve ? "Approve and save" : "Save") : `Save · needs ${missing}`}</button>
          <button onClick={runTest} disabled={!canSave || testing}>Test</button>
        </span>
        <span className="row" style={{ margin: 0 }}>
          {!isNew && <button className="quiet" onClick={dup}>Duplicate</button>}
          {!isNew && <ConfirmButton onConfirm={del} really="Really delete">Delete</ConfirmButton>}
        </span>
      </div>
    </div>
  );
}
