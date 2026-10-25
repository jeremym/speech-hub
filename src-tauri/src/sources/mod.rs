//! User-defined Sources: saved definitions of *where text comes from*.
//! Text, file, command, Apple Shortcut, or URL. Built in the GUI, run from the palette,
//! the second hotkey, the CLI (`speak run`), or a confirmed deep link.
//!
//! Safety model (PLAN.md §6): executable kinds (command, shortcut) run only when `trusted`.
//! Sources created in the app are trusted; imported ones are not until the user approves.

pub mod exec;

use crate::{hub, settings::SettingsStore, windows};
use serde::{Deserialize, Serialize};
use speechhub_protocol::{ErrorCode, SpeakOptions};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SourceKind {
    /// A saved snippet.
    Text { text: String },
    /// A local text or Markdown file the user picked.
    File { path: String },
    /// Run an executable with an argument array; stdout becomes the text.
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        cwd: Option<String>,
        /// Opt-in: interpret `program` as a shell line via /bin/zsh -c (args ignored).
        #[serde(default)]
        shell: bool,
    },
    /// Run an Apple Shortcut by name; its text output becomes the text.
    Shortcut {
        name: String,
        #[serde(default)]
        input: Option<String>,
    },
    /// HTTP GET. Plain text, or JSON with a simple field path.
    Http {
        url: String,
        #[serde(default)]
        json_path: Option<String>,
    },
}

impl SourceKind {
    pub fn type_name(&self) -> &'static str {
        match self {
            SourceKind::Text { .. } => "text",
            SourceKind::File { .. } => "file",
            SourceKind::Command { .. } => "command",
            SourceKind::Shortcut { .. } => "shortcut",
            SourceKind::Http { .. } => "http",
        }
    }
    /// Kinds that run code on the user's machine.
    pub fn is_executable(&self) -> bool {
        matches!(self, SourceKind::Command { .. } | SourceKind::Shortcut { .. })
    }
    #[allow(dead_code)]
    pub fn uses_network(&self) -> bool {
        matches!(self, SourceKind::Http { .. })
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SourceAction {
    #[default]
    Speak,
    Open,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    pub kind: SourceKind,
    #[serde(default)]
    pub voice_id: Option<String>,
    #[serde(default)]
    pub speed: Option<f32>,
    #[serde(default)]
    pub action: SourceAction,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Executable kinds run only when true. Set by the user, never by an import or a link.
    #[serde(default)]
    pub trusted: bool,
    #[serde(default)]
    pub favorite: bool,
    /// Seconds a command/shortcut/http may take. Clamped 1..=120.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    #[serde(default)]
    pub run_count: u32,
    /// Unix millis.
    #[serde(default)]
    pub last_run_at: Option<u64>,
    #[serde(default)]
    pub created_at: u64,
}

fn default_icon() -> String {
    "💬".into()
}
fn yes() -> bool {
    true
}
fn default_timeout() -> u32 {
    15
}
fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

impl Source {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(name: &str, kind: SourceKind) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: String::new(),
            icon: default_icon(),
            kind,
            voice_id: None,
            speed: None,
            action: SourceAction::Speak,
            enabled: true,
            trusted: true,
            favorite: false,
            timeout_secs: default_timeout(),
            run_count: 0,
            last_run_at: None,
            created_at: now_ms(),
        }
    }

    fn sanitize(mut self) -> Self {
        self.name = self.name.trim().chars().take(80).collect();
        if self.name.is_empty() {
            self.name = "Untitled Source".into();
        }
        self.description = self.description.chars().take(400).collect();
        if self.icon.trim().is_empty() || self.icon.chars().count() > 4 {
            self.icon = default_icon();
        }
        self.timeout_secs = self.timeout_secs.clamp(1, 120);
        if let Some(s) = self.speed {
            self.speed = Some(s.clamp(0.5, 3.0));
        }
        if self.id.is_empty() {
            self.id = uuid::Uuid::new_v4().to_string();
        }
        self
    }

    /// Short description safe to show outside the Builder (palette, `speak sources list`):
    /// program and file *names* only, never arguments, full paths, or query strings.
    pub fn summary(&self) -> String {
        let base = |p: &str| Path::new(p).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string());
        match &self.kind {
            SourceKind::Text { text } => format!("Text · {} chars", text.chars().count()),
            SourceKind::File { path } => format!("Reads {}", base(path)),
            SourceKind::Command { program, shell: false, .. } => format!("Runs {}", base(program)),
            SourceKind::Command { shell: true, .. } => "Runs a shell command".into(),
            SourceKind::Shortcut { name, .. } => format!("Shortcut “{name}”"),
            SourceKind::Http { url, .. } => {
                let host = url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or("a URL");
                format!("Fetches from {host}")
            }
        }
    }
}

/// What external callers (CLI, palette) may see: no args, paths, or URLs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSource {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub action: SourceAction,
    pub enabled: bool,
    pub trusted: bool,
    pub favorite: bool,
    pub last_run_at: Option<u64>,
    pub run_count: u32,
    pub summary: String,
}

impl From<&Source> for PublicSource {
    fn from(s: &Source) -> Self {
        PublicSource {
            id: s.id.clone(),
            name: s.name.clone(),
            description: s.description.clone(),
            icon: s.icon.clone(),
            type_name: s.kind.type_name().into(),
            action: s.action,
            enabled: s.enabled,
            trusted: s.trusted,
            favorite: s.favorite,
            last_run_at: s.last_run_at,
            run_count: s.run_count,
            summary: s.summary(),
        }
    }
}

// ---------------------------------------------------------------- store

#[derive(Clone)]
pub struct SourceStore {
    path: PathBuf,
    inner: Arc<Mutex<Vec<Source>>>,
}

pub const CHANGED_EVENT: &str = "sources:changed";

impl SourceStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("sources.json");
        let list = fs::read(&path).ok().and_then(|b| serde_json::from_slice::<Vec<Source>>(&b).ok()).unwrap_or_default();
        Self { path, inner: Arc::new(Mutex::new(list)) }
    }

    pub fn list(&self) -> Vec<Source> {
        self.inner.lock().unwrap().clone()
    }

    pub fn get(&self, id: &str) -> Option<Source> {
        self.inner.lock().unwrap().iter().find(|s| s.id == id).cloned()
    }

    /// Find by id, or by name (case-insensitive, then by slug "morning-briefing" == "Morning Briefing").
    pub fn resolve(&self, reference: &str) -> Option<Source> {
        let list = self.inner.lock().unwrap();
        let r = reference.trim();
        let slug = |s: &str| s.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '-' }).collect::<String>().split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
        list.iter()
            .find(|s| s.id == r)
            .or_else(|| list.iter().find(|s| s.name.eq_ignore_ascii_case(r)))
            .or_else(|| list.iter().find(|s| slug(&s.name) == slug(r)))
            .cloned()
    }

    pub fn upsert(&self, source: Source) -> Result<Source, String> {
        let source = source.sanitize();
        {
            let mut list = self.inner.lock().unwrap();
            match list.iter_mut().find(|s| s.id == source.id) {
                Some(slot) => *slot = source.clone(),
                None => list.push(source.clone()),
            }
        }
        self.save()?;
        Ok(source)
    }

    pub fn delete(&self, id: &str) -> Result<(), String> {
        self.inner.lock().unwrap().retain(|s| s.id != id);
        self.save()
    }

    pub fn duplicate(&self, id: &str) -> Result<Source, String> {
        let mut copy = self.get(id).ok_or("no such Source")?;
        copy.id = uuid::Uuid::new_v4().to_string();
        copy.name = format!("{} copy", copy.name);
        copy.run_count = 0;
        copy.last_run_at = None;
        copy.created_at = now_ms();
        self.upsert(copy)
    }

    pub fn mark_run(&self, id: &str) {
        if let Some(s) = self.inner.lock().unwrap().iter_mut().find(|s| s.id == id) {
            s.run_count += 1;
            s.last_run_at = Some(now_ms());
        }
        let _ = self.save();
    }

    /// Export everything as pretty JSON (the user's own data; may contain paths and commands).
    pub fn export_json(&self) -> String {
        serde_json::to_string_pretty(&self.list()).unwrap_or_else(|_| "[]".into())
    }

    /// Import. Imported executable Sources are never trusted; ids are regenerated to avoid clobbering.
    pub fn import_json(&self, json: &str) -> Result<usize, String> {
        let incoming: Vec<Source> = serde_json::from_str(json).map_err(|e| format!("not a Sources export: {e}"))?;
        let n = incoming.len();
        for mut s in incoming {
            s.id = uuid::Uuid::new_v4().to_string();
            s.trusted = !s.kind.is_executable();
            s.run_count = 0;
            s.last_run_at = None;
            self.upsert(s)?;
        }
        Ok(n)
    }

    fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(&*self.inner.lock().unwrap()).map_err(|e| e.to_string())?;
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------- errors

#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("no Source named '{0}'")]
    NotFound(String),
    #[error("Source '{0}' is disabled")]
    Disabled(String),
    #[error("'{0}' runs a command and hasn't been approved yet. Open Preferences → Sources and click Trust.")]
    NotTrusted(String),
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    Hub(#[from] hub::HubError),
}

impl SourceError {
    pub fn code(&self) -> ErrorCode {
        match self {
            SourceError::NotFound(_) | SourceError::Disabled(_) => ErrorCode::SourceNotFound,
            SourceError::NotTrusted(_) => ErrorCode::SourceNotTrusted,
            SourceError::Failed(_) => ErrorCode::SourceFailed,
            SourceError::Hub(e) => e.code(),
        }
    }
}

// ---------------------------------------------------------------- running

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunResult {
    pub source_id: String,
    pub name: String,
    pub action: SourceAction,
    pub chars: usize,
    pub fetch_ms: u128,
    /// Playback request id when the action was Speak.
    pub playback_id: Option<String>,
}

pub fn store(app: &AppHandle) -> SourceStore {
    app.state::<SourceStore>().inner().clone()
}

fn emit_changed(app: &AppHandle) {
    let _ = app.emit(CHANGED_EVENT, ());
}

/// Fetch a Source's text (no playback). Used by Run and by the Builder's Test button.
pub fn fetch(source: &Source) -> Result<(String, u128), SourceError> {
    if source.kind.is_executable() && !source.trusted {
        return Err(SourceError::NotTrusted(source.name.clone()));
    }
    let t = Instant::now();
    let text = exec::fetch_text(&source.kind, source.timeout_secs)?;
    Ok((text, t.elapsed().as_millis()))
}

/// Run a Source: fetch its text, then speak or open according to its action.
pub fn run(app: &AppHandle, source: &Source) -> Result<RunResult, SourceError> {
    if !source.enabled {
        return Err(SourceError::Disabled(source.name.clone()));
    }
    let hub = hub::get(app);
    hub.set_loading_source(Some(&source.name));
    let fetched = fetch(source);
    hub.set_loading_source(None);
    let (text, fetch_ms) = match fetched {
        Ok(v) => v,
        Err(e) => {
            hub.notice(format!("{}: {e}", source.name));
            return Err(e);
        }
    };
    if text.trim().is_empty() {
        let e = SourceError::Failed(format!("“{}” produced no text", source.name));
        hub.notice(e.to_string());
        return Err(e);
    }
    let options = SpeakOptions { voice_id: source.voice_id.clone(), speed: source.speed, title: Some(source.name.clone()) };
    let playback_id = match source.action {
        SourceAction::Speak => Some(hub.speak(&text, options)?),
        SourceAction::Open => {
            hub.open(&text, Some(source.name.clone()));
            None
        }
    };
    let store = store(app);
    store.mark_run(&source.id);
    emit_changed(app);
    Ok(RunResult { source_id: source.id.clone(), name: source.name.clone(), action: source.action, chars: text.chars().count(), fetch_ms, playback_id })
}

/// Run by id or name. Public entry point for CLI, palette, hotkey, fallback.
pub fn run_by_ref(app: &AppHandle, reference: &str) -> Result<RunResult, SourceError> {
    let source = store(app).resolve(reference).ok_or_else(|| SourceError::NotFound(reference.to_string()))?;
    run(app, &source)
}

/// Public listing for `speak sources list` and the palette.
pub fn list_public(app: &AppHandle) -> Vec<PublicSource> {
    let mut v: Vec<PublicSource> = store(app).list().iter().map(PublicSource::from).collect();
    // Favorites first, then most recently run, then name.
    v.sort_by(|a, b| b.favorite.cmp(&a.favorite).then(b.last_run_at.cmp(&a.last_run_at)).then(a.name.cmp(&b.name)));
    v
}

/// A deep link asked to run a Source. Never run it directly: show the palette with a confirmation.
pub fn confirm_and_run_from_link(app: &AppHandle, reference: &str) {
    match store(app).resolve(reference) {
        Some(source) => {
            windows::show(app, windows::PALETTE);
            let payload = PublicSource::from(&source);
            // The palette may still be loading; it also re-asks via `palette_take_pending_confirm`.
            *app.state::<PendingConfirm>().0.lock().unwrap() = Some(payload.clone());
            let _ = app.emit("palette:confirm-source", payload);
        }
        None => hub::get(app).notice(format!("A link asked for a Source named “{reference}”, which doesn't exist.")),
    }
}

/// Deep-link confirmation waiting for the palette to pick up.
#[derive(Default)]
pub struct PendingConfirm(pub Mutex<Option<PublicSource>>);

/// Hotkey 2 configured to run a specific Source.
pub fn run_quick_speak_source(app: &AppHandle, settings: &SettingsStore) {
    let s = settings.get();
    match s.quick_speak_source_id.as_deref() {
        Some(id) if !id.is_empty() => {
            if let Err(e) = run_by_ref(app, id) {
                log::warn!("quick speak source: {e}");
                hub::get(app).open_palette();
            }
        }
        _ => hub::get(app).open_palette(),
    }
}

/// Selection capture failed: run the user's fallback Source if one is configured.
pub fn run_fallback(app: &AppHandle, settings: &SettingsStore) -> bool {
    let s = settings.get();
    match s.fallback_source_id.as_deref() {
        Some(id) if !id.is_empty() => match run_by_ref(app, id) {
            Ok(_) => true,
            Err(e) => {
                // `Failed` carries command stderr; log only the kind so no text reaches the log.
                log::warn!("fallback source failed: {:?}", e.code());
                false
            }
        },
        _ => false,
    }
}

// ---------------------------------------------------------------- templates

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub kind: SourceKind,
    pub action: SourceAction,
    pub needs: Vec<String>,
}

pub fn templates() -> Vec<Template> {
    let t = |id: &str, name: &str, desc: &str, icon: &str, kind: SourceKind, action: SourceAction, needs: &[&str]| Template {
        id: id.into(),
        name: name.into(),
        description: desc.into(),
        icon: icon.into(),
        kind,
        action,
        needs: needs.iter().map(|s| s.to_string()).collect(),
    };
    vec![
        t("clipboard", "Current clipboard", "Speaks whatever you last copied, via the macOS pbpaste command. No clipboard history is kept.", "📋",
          SourceKind::Command { program: "/usr/bin/pbpaste".into(), args: vec![], cwd: None, shell: false }, SourceAction::Speak, &[]),
        t("file", "A text or Markdown file", "Reads a file you choose each time it runs. Good for a daily notes file.", "📄",
          SourceKind::File { path: String::new() }, SourceAction::Open, &["path"]),
        t("command", "Output of a command", "Runs a program with arguments and speaks what it prints. No shell, so arguments are passed exactly.", "⚙️",
          SourceKind::Command { program: "/bin/date".into(), args: vec!["+%A, %B %e. It is %l:%M %p.".into()], cwd: None, shell: false }, SourceAction::Speak, &[]),
        t("shortcut", "An Apple Shortcut", "Runs a Shortcut and speaks its text output. Make the Shortcut end with a text result.", "🔗",
          SourceKind::Shortcut { name: String::new(), input: None }, SourceAction::Speak, &["name"]),
        t("url", "Text from a URL", "Fetches a web address when it runs. Plain text works directly; for JSON, give a field path like items.0.title.", "🌐",
          SourceKind::Http { url: String::new(), json_path: None }, SourceAction::Speak, &["url"]),
        t("text", "A saved snippet", "A fixed piece of text, for example a reminder you want read back on demand.", "✏️",
          SourceKind::Text { text: String::new() }, SourceAction::Speak, &["text"]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_by_id_name_and_slug() {
        let dir = std::env::temp_dir().join(format!("speechhub-sources-{}", uuid::Uuid::new_v4()));
        let store = SourceStore::load(&dir);
        let s = store.upsert(Source::new("Morning Briefing", SourceKind::Text { text: "hi".into() })).unwrap();
        assert_eq!(store.resolve(&s.id).unwrap().id, s.id);
        assert_eq!(store.resolve("morning briefing").unwrap().id, s.id);
        assert_eq!(store.resolve("morning-briefing").unwrap().id, s.id);
        assert!(store.resolve("nope").is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn imports_are_untrusted_when_executable() {
        let dir = std::env::temp_dir().join(format!("speechhub-sources-{}", uuid::Uuid::new_v4()));
        let store = SourceStore::load(&dir);
        let cmd = Source::new("Clip", SourceKind::Command { program: "/usr/bin/pbpaste".into(), args: vec![], cwd: None, shell: false });
        let txt = Source::new("Note", SourceKind::Text { text: "x".into() });
        let json = serde_json::to_string(&vec![cmd, txt]).unwrap();
        assert_eq!(store.import_json(&json).unwrap(), 2);
        let list = store.list();
        assert!(!list.iter().find(|s| s.name == "Clip").unwrap().trusted);
        assert!(list.iter().find(|s| s.name == "Note").unwrap().trusted);
        assert!(matches!(fetch(list.iter().find(|s| s.name == "Clip").unwrap()), Err(SourceError::NotTrusted(_))));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn public_view_hides_details() {
        let s = Source::new("Secret", SourceKind::Command { program: "/usr/bin/security".into(), args: vec!["find-generic-password".into()], cwd: None, shell: false });
        let p = PublicSource::from(&s);
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"type\":\"command\""));
        assert!(!json.contains("find-generic-password"));
    }
}
