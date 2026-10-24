//! Window management. Every window is created on demand and hidden (not destroyed) on close,
//! so reopening is instant and React state survives.

use std::{sync::{Mutex, OnceLock}, time::Duration};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

pub const ONBOARDING: &str = "onboarding";
pub const SETTINGS: &str = "settings";
pub const READER: &str = "reader";
pub const PLAYER: &str = "player";
pub const PALETTE: &str = "palette";

pub const ALL: [&str; 5] = [ONBOARDING, SETTINGS, READER, PLAYER, PALETTE];

fn ensure(app: &AppHandle, label: &str) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    let url = WebviewUrl::App("index.html".into());
    let builder = WebviewWindowBuilder::new(app, label, url);
    let builder = match label {
        ONBOARDING => builder.title("Welcome to Speech Hub").inner_size(760.0, 580.0).resizable(false).center(),
        SETTINGS => builder.title("Speech Hub Preferences").inner_size(760.0, 540.0).min_inner_size(640.0, 440.0).center(),
        READER => builder.title("Reader").inner_size(640.0, 480.0).min_inner_size(420.0, 300.0).center(),
        PALETTE => builder
            .title("Quick Speak")
            .inner_size(560.0, 420.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .center(),
        PLAYER => builder
            .title("Now Speaking")
            .inner_size(340.0, 76.0)
            .resizable(false)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(false)
            .accept_first_mouse(true)
            .visible_on_all_workspaces(true),
        _ => builder,
    };
    let window = builder.visible(false).build()?;
    if label == PLAYER {
        position_player(app, &window);
    }
    Ok(window)
}

fn position_player(app: &AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let (w, h) = ((340.0 * scale) as i32, (76.0 * scale) as i32);
        let margin = (16.0 * scale) as i32;
        let menubar = (40.0 * scale) as i32;
        let x = monitor.position().x + size.width as i32 - w - margin;
        let y = monitor.position().y + menubar;
        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = h;
    }
}

/// Show and focus a regular window.
pub fn show(app: &AppHandle, label: &str) {
    match ensure(app, label) {
        Ok(w) => {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            let _ = app.emit("window:shown", label);
        }
        Err(e) => log::error!("cannot open window {label}: {e}"),
    }
}

/// Show the mini player without stealing focus.
pub fn show_player(app: &AppHandle) {
    cancel_player_hide();
    match ensure(app, PLAYER) {
        Ok(w) => {
            position_player(app, &w);
            let _ = w.show();
        }
        Err(e) => log::error!("cannot open player: {e}"),
    }
}

pub fn hide(app: &AppHandle, label: &str) {
    if let Some(w) = app.get_webview_window(label) {
        let _ = w.hide();
    }
}

pub fn is_visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label).and_then(|w| w.is_visible().ok()).unwrap_or(false)
}

static HIDE_TOKEN: OnceLock<Mutex<u64>> = OnceLock::new();

fn hide_token() -> &'static Mutex<u64> {
    HIDE_TOKEN.get_or_init(|| Mutex::new(0))
}

fn cancel_player_hide() {
    *hide_token().lock().unwrap() += 1;
}

/// Hide the player after a delay unless something shows it again first.
pub fn schedule_hide_player(app: &AppHandle, delay: Duration) {
    let token = {
        let mut t = hide_token().lock().unwrap();
        *t += 1;
        *t
    };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        if *hide_token().lock().unwrap() == token {
            hide(&app, PLAYER);
        }
    });
}

/// Ask a window's React tree to navigate (e.g. Preferences → "sources" pane).
pub fn navigate(app: &AppHandle, label: &str, pane: &str) {
    show(app, label);
    let _ = app.emit("window:navigate", serde_json::json!({ "window": label, "pane": pane }));
}
