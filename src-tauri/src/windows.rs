//! Window management. Every window is created on demand and hidden (not destroyed) on close,
//! so reopening is instant and React state survives.

use std::{collections::HashSet, sync::{Mutex, OnceLock}, time::Duration};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

pub const ONBOARDING: &str = "onboarding";
pub const SETTINGS: &str = "settings";
pub const READER: &str = "reader";
pub const PLAYER: &str = "player";
pub const PALETTE: &str = "palette";

pub const ALL: [&str; 5] = [ONBOARDING, SETTINGS, READER, PLAYER, PALETTE];

/// Labels currently shown, maintained by `show`/`hide`/`show_player` and the close handler.
/// Read this from worker threads instead of `WebviewWindow::is_visible()`: Tauri's window getters
/// are dispatched to the event loop and wait for it, which deadlocks whenever the main thread is
/// itself blocked on the worker (a sync command waiting on a channel).
static SHOWN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

fn shown() -> &'static Mutex<HashSet<String>> {
    SHOWN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn mark_shown(label: &str, visible: bool) {
    let mut set = shown().lock().unwrap();
    if visible {
        set.insert(label.to_string());
    } else {
        set.remove(label);
    }
}

/// Cheap, thread-safe visibility check (see `SHOWN`).
pub fn is_shown(label: &str) -> bool {
    shown().lock().unwrap().contains(label)
}

fn ensure(app: &AppHandle, label: &str) -> tauri::Result<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window(label) {
        return Ok(w);
    }
    let url = WebviewUrl::App("index.html".into());
    let builder = WebviewWindowBuilder::new(app, label, url);
    let builder = match label {
        ONBOARDING => builder.title("Welcome to Speech Hub").inner_size(820.0, 640.0).resizable(false).center(),
        SETTINGS => builder.title("Speech Hub Preferences").inner_size(960.0, 680.0).min_inner_size(860.0, 560.0).center(),
        READER => builder.title("Reader").inner_size(680.0, 500.0).min_inner_size(460.0, 320.0).center(),
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
            .inner_size(340.0, 80.0)
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
    match label {
        PLAYER | PALETTE => crate::macos::float_on_all_spaces(&window),
        // Document windows keep AppKit's default collection behavior. Overriding it (even to add
        // MoveToActiveSpace) drops the implicit FullScreenPrimary, which removes the green
        // full-screen button; tiling managers (AeroSpace) then treat the window as a dialog and
        // float it. Setting FullScreenPrimary explicitly deadlocks tao's window delegate.
        _ => {}
    }
    if label == PLAYER {
        position_player(app, &window);
    }
    Ok(window)
}

fn position_player(app: &AppHandle, window: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let (w, h) = ((340.0 * scale) as i32, (80.0 * scale) as i32);
        let margin = (16.0 * scale) as i32;
        let menubar = (40.0 * scale) as i32;
        let x = monitor.position().x + size.width as i32 - w - margin;
        let y = monitor.position().y + menubar;
        let _ = window.set_position(PhysicalPosition::new(x, y));
        let _ = h;
    }
}

/// Run on the main thread (window creation is main-thread-only on macOS). Callers may be on
/// hotkey, IPC, or synthesis threads.
///
/// Always goes through the event loop, never inline: Tauri runs `run_on_main_thread` closures
/// immediately when already on the main thread, and creating a window from inside a plugin
/// callback (deep link `on_open_url`, global shortcut) then re-enters Tauri's plugin-store
/// mutex and deadlocks the app. Hopping through a thread makes the closure arrive as an event
/// loop message after the callback has returned.
fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    std::thread::spawn(move || {
        let inner = handle.clone();
        if let Err(e) = handle.run_on_main_thread(move || f(&inner)) {
            log::error!("run_on_main_thread: {e}");
        }
    });
}

/// Windows that should behave like a normal app's windows: tiled by window managers, in the
/// Cmd-Tab list, with a Dock icon while open. The menu-bar-only policy returns when they close.
pub const DOCUMENT_WINDOWS: [&str; 3] = [SETTINGS, READER, ONBOARDING];

fn is_document(label: &str) -> bool {
    DOCUMENT_WINDOWS.contains(&label)
}

/// Regular while any document window is visible, Accessory otherwise. Main thread only.
pub fn sync_activation_policy(app: &AppHandle) {
    let any_visible = DOCUMENT_WINDOWS.iter().any(|l| is_visible(app, l));
    let policy = if any_visible { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory };
    if let Err(e) = app.set_activation_policy(policy) {
        log::warn!("set_activation_policy: {e}");
    }
}

/// Show and focus a regular window.
pub fn show(app: &AppHandle, label: &str) {
    let label = label.to_string();
    on_main(app, move |app| match ensure(app, &label) {
        Ok(w) => {
            if is_document(&label) {
                // Before showing, so the window manager sees a regular app's window from the start.
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
            }
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            mark_shown(&label, true);
            let _ = app.emit("window:shown", label);
        }
        Err(e) => log::error!("cannot open window {label}: {e}"),
    });
}

/// Show the mini player without stealing focus or switching Spaces.
pub fn show_player(app: &AppHandle) {
    cancel_player_hide();
    on_main(app, |app| match ensure(app, PLAYER) {
        Ok(w) => {
            if !w.is_visible().unwrap_or(false) {
                position_player(app, &w);
            }
            crate::macos::order_front_without_focus(&w);
            mark_shown(PLAYER, true);
        }
        Err(e) => log::error!("cannot open player: {e}"),
    });
}

pub fn hide(app: &AppHandle, label: &str) {
    let label = label.to_string();
    on_main(app, move |app| {
        if let Some(w) = app.get_webview_window(&label) {
            let _ = w.hide();
        }
        mark_shown(&label, false);
        // The palette took keyboard focus; hand it back to the app the user was in.
        if label == PALETTE {
            crate::macos::deactivate_app();
            crate::hub::get(app).palette_closed();
        }
        if is_document(&label) {
            sync_activation_policy(app);
        }
    });
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
    navigate_with(app, label, pane, None);
}

pub fn navigate_with(app: &AppHandle, label: &str, pane: &str, prefill: Option<serde_json::Value>) {
    show(app, label);
    let app = app.clone();
    let payload = serde_json::json!({ "window": label, "pane": pane, "prefill": prefill });
    // The window may have just been created; give React a moment to mount its listener.
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(250));
        let _ = app.emit("window:navigate", payload);
    });
}
