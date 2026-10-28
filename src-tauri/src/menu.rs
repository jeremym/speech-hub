//! Application menu for the document windows (Preferences, Reader, onboarding).
//!
//! Tauri's default menu wires ⌘Q to `terminate:`, which would kill the speech service a user
//! only meant to dismiss a window from. Here ⌘Q closes the open windows and leaves the menu-bar
//! app running; the tray's Quit is the real exit. Edit items are kept so the webviews' text
//! fields have working cut/copy/paste/undo.

use tauri::{
    App, Emitter,
    menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder},
};

use crate::{hub, windows};

const CLOSE_ALL: &str = "close-all-windows";
const QUIT: &str = "quit-service";

pub fn install(app: &App) -> tauri::Result<()> {
    let close_all = MenuItemBuilder::with_id(CLOSE_ALL, "Close Windows (keep running)").accelerator("CmdOrCtrl+Q").build(app)?;
    let quit = MenuItemBuilder::with_id(QUIT, "Quit Speech Hub Entirely").build(app)?;
    let app_menu = SubmenuBuilder::new(app, "Speech Hub")
        .about(None)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&close_all)
        .item(&quit)
        .build()?;
    let edit = SubmenuBuilder::new(app, "Edit").undo().redo().separator().cut().copy().paste().select_all().build()?;
    // The look (theme and text size) lives in every webview; these items emit a `menu` event that
    // the windows handle with the kit's applyLook (src/look.ts).
    let look = |id: &str, text: &str, accel: Option<&str>| -> tauri::Result<tauri::menu::MenuItem<tauri::Wry>> {
        let mut b = MenuItemBuilder::with_id(id, text);
        if let Some(a) = accel {
            b = b.accelerator(a);
        }
        b.build(app)
    };
    let view = SubmenuBuilder::new(app, "View")
        .item(&look("theme-amber", "Amber Theme", None)?)
        .item(&look("theme-green", "Green Theme", None)?)
        .item(&look("theme-paper", "Paper Theme", None)?)
        .separator()
        .item(&look("text-larger", "Larger Text", Some("CmdOrCtrl+="))?)
        .item(&look("text-smaller", "Smaller Text", Some("CmdOrCtrl+-"))?)
        .build()?;
    let window = SubmenuBuilder::new(app, "Window").minimize().separator().close_window().build()?;
    let menu = MenuBuilder::new(app).items(&[&app_menu, &edit, &view, &window]).build()?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| match event.id().as_ref() {
        CLOSE_ALL => {
            for label in windows::DOCUMENT_WINDOWS {
                windows::hide(app, label);
            }
            hub::get(app).notice("Still running in the menu bar. Quit from the ◉ menu.".into());
        }
        QUIT => {
            hub::get(app).stop();
            app.exit(0);
        }
        id if id.starts_with("theme-") || id.starts_with("text-") => {
            let _ = app.emit("menu", id.to_string());
        }
        _ => {}
    });
    Ok(())
}
