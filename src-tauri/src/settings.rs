//! User settings: one JSON file in the app data dir, written atomically.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub onboarding_complete: bool,
    pub hotkeys: Hotkeys,
    /// Engine profile id: "lite" | "standard" | "plus" | "dev-say".
    pub engine_profile: String,
    /// Voice id within the engine. Empty = engine default.
    pub voice_id: String,
    pub speed: f32,
    pub launch_at_login: bool,
    pub show_mini_player: bool,
    /// What Option+Escape does while something is already playing.
    pub repeat_shortcut: RepeatBehavior,
    pub quick_speak_action: QuickSpeakAction,
    pub quick_speak_source_id: Option<String>,
    /// What Speak Selection does when Accessibility can't read the selection (PLAN.md §1.4).
    pub fallback_action: FallbackAction,
    /// Source to run for `FallbackAction::Source`.
    pub fallback_source_id: Option<String>,
    /// Unload the model after this many idle minutes. 0 = never.
    pub idle_unload_minutes: u32,
    /// Ask browsers to build their accessibility tree as soon as they come to the front, so the
    /// first Option+Escape in them is instant (see prewarm.rs).
    pub prewarm_browsers: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Hotkeys {
    pub speak_selection: String,
    pub quick_speak: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RepeatBehavior {
    /// Pressing again replaces the current reading with the new selection.
    #[default]
    Replace,
    /// Pressing again stops; press once more to read the new selection.
    Stop,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuickSpeakAction {
    #[default]
    Palette,
    Reader,
    Source,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FallbackAction {
    /// Show "Nothing is selected" (or the real reason) in the mini player.
    #[default]
    Notice,
    /// Open the Quick Speak panel so the user can pick a Source or type instead.
    QuickSpeak,
    /// Run `fallback_source_id` (e.g. "Current clipboard").
    Source,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self { speak_selection: "Alt+Escape".into(), quick_speak: "Alt+Shift+Escape".into() }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarding_complete: false,
            hotkeys: Hotkeys::default(),
            engine_profile: "standard".into(),
            voice_id: String::new(),
            speed: 1.0,
            launch_at_login: false,
            show_mini_player: true,
            repeat_shortcut: RepeatBehavior::Replace,
            quick_speak_action: QuickSpeakAction::Palette,
            quick_speak_source_id: None,
            fallback_action: FallbackAction::Notice,
            fallback_source_id: None,
            idle_unload_minutes: 0,
            prewarm_browsers: true,
        }
    }
}

#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    inner: Arc<Mutex<Settings>>,
}

impl SettingsStore {
    pub fn load(dir: &Path) -> Self {
        let path = dir.join("settings.json");
        let settings = fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Settings>(&bytes).ok())
            .unwrap_or_default();
        Self { path, inner: Arc::new(Mutex::new(settings)) }
    }

    pub fn get(&self) -> Settings {
        self.inner.lock().expect("settings poisoned").clone()
    }

    pub fn replace(&self, new: Settings) -> Result<Settings, String> {
        let mut guard = self.inner.lock().expect("settings poisoned");
        *guard = sanitize(new);
        let snapshot = guard.clone();
        drop(guard);
        self.save(&snapshot)?;
        Ok(snapshot)
    }

    pub fn update(&self, f: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut current = self.get();
        f(&mut current);
        self.replace(current)
    }

    fn save(&self, s: &Settings) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
        fs::write(&tmp, json).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &self.path).map_err(|e| e.to_string())
    }
}

fn sanitize(mut s: Settings) -> Settings {
    s.speed = s.speed.clamp(0.5, 3.0);
    if s.hotkeys.speak_selection.trim().is_empty() {
        s.hotkeys.speak_selection = Hotkeys::default().speak_selection;
    }
    if s.hotkeys.quick_speak.trim().is_empty() {
        s.hotkeys.quick_speak = Hotkeys::default().quick_speak;
    }
    // Settings written before `fallback_action` existed: a chosen Source meant "run it".
    if s.fallback_action == FallbackAction::Notice && s.fallback_source_id.as_deref().is_some_and(|id| !id.is_empty()) {
        s.fallback_action = FallbackAction::Source;
    }
    if s.fallback_action == FallbackAction::Source && s.fallback_source_id.as_deref().is_none_or(|id| id.is_empty()) {
        s.fallback_action = FallbackAction::Notice;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_sanitize() {
        let dir = std::env::temp_dir().join(format!("speechhub-settings-{}", uuid::Uuid::new_v4()));
        let store = SettingsStore::load(&dir);
        assert!(!store.get().onboarding_complete);
        let saved = store
            .update(|s| {
                s.onboarding_complete = true;
                s.speed = 99.0;
            })
            .unwrap();
        assert_eq!(saved.speed, 3.0);
        let again = SettingsStore::load(&dir);
        assert!(again.get().onboarding_complete);
        let _ = fs::remove_dir_all(dir);
    }
}
