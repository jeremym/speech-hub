//! Global shortcuts. Registration lives in Rust so the frontend never needs shortcut permissions.

use crate::{hub, settings::{QuickSpeakAction, Settings}, windows};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyStatus {
    pub id: String,
    pub shortcut: String,
    pub registered: bool,
    pub error: Option<String>,
}

pub fn handler(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let settings = app.state::<crate::settings::SettingsStore>().get();
    let is = |s: &str| s.parse::<Shortcut>().map(|k| &k == shortcut).unwrap_or(false);
    if is(&settings.hotkeys.speak_selection) {
        let _ = hub::get(app).speak_selection();
    } else if is(&settings.hotkeys.quick_speak) {
        on_quick_speak(app, &settings);
    }
}

fn on_quick_speak(app: &AppHandle, settings: &Settings) {
    match settings.quick_speak_action {
        QuickSpeakAction::Palette => hub::get(app).open_palette(),
        QuickSpeakAction::Reader => windows::show(app, windows::READER),
        QuickSpeakAction::Source => {
            let store = app.state::<crate::settings::SettingsStore>().inner().clone();
            crate::sources::run_quick_speak_source(app, &store)
        }
    }
}

/// (Re)register both shortcuts from settings. Never panics on a bad or taken shortcut;
/// reports per-shortcut status so the UI can explain.
pub fn apply(app: &AppHandle, settings: &Settings) -> Vec<HotkeyStatus> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut out = Vec::new();
    for (id, text) in [("speak_selection", &settings.hotkeys.speak_selection), ("quick_speak", &settings.hotkeys.quick_speak)] {
        let status = match text.parse::<Shortcut>() {
            Ok(sc) => match gs.register(sc) {
                Ok(()) => HotkeyStatus { id: id.into(), shortcut: text.clone(), registered: true, error: None },
                Err(e) => HotkeyStatus { id: id.into(), shortcut: text.clone(), registered: false, error: Some(e.to_string()) },
            },
            Err(e) => HotkeyStatus {
                id: id.into(),
                shortcut: text.clone(),
                registered: false,
                error: Some(format!("not a valid shortcut: {e}")),
            },
        };
        if let Some(err) = &status.error {
            log::warn!("hotkey {id} ({text}) not registered: {err}");
        }
        out.push(status);
    }
    out
}
