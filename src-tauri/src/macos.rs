//! Small AppKit touches Tauri doesn't expose: Space behavior and showing windows without
//! stealing focus. Everything here must run on the main thread.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSWindow, NSWindowCollectionBehavior};
use tauri::WebviewWindow;

fn ns_window(w: &WebviewWindow) -> Option<&NSWindow> {
    let ptr = w.ns_window().ok()?;
    if ptr.is_null() {
        return None;
    }
    // SAFETY: Tauri hands us the live NSWindow it owns; we only call AppKit methods on it from
    // the main thread and never retain it past this call.
    unsafe { ptr.cast::<NSWindow>().as_ref() }
}

/// HUD-style: appears on whichever Space is active, floats, and survives full-screen apps.
pub fn float_on_all_spaces(w: &WebviewWindow) {
    if let Some(win) = ns_window(w) {
        win.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        win.setLevel(objc2_app_kit::NSFloatingWindowLevel);
    }
}

/// Regular windows (Reader, Preferences): when re-shown, come to the user's current Space
/// instead of dragging the user back to the Space they were created on.
pub fn move_to_active_space(w: &WebviewWindow) {
    if let Some(win) = ns_window(w) {
        win.setCollectionBehavior(NSWindowCollectionBehavior::MoveToActiveSpace | NSWindowCollectionBehavior::FullScreenAuxiliary);
    }
}

/// Show without making the window key and without activating the app (mini player).
pub fn order_front_without_focus(w: &WebviewWindow) {
    if let Some(win) = ns_window(w) {
        win.orderFrontRegardless();
    }
}

/// Give keyboard focus back to whatever app was active before we took it (after the palette hides).
pub fn deactivate_app() {
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.deactivate();
    }
}
