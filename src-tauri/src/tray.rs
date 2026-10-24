//! Menu-bar icon and menu.

use crate::{hub, windows};
use tauri::{
    AppHandle, Manager,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

const ICON: &[u8] = include_bytes!("../icons/tray-template@2x.png");

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let item = |id: &str, label: &str, accel: Option<&str>| MenuItem::with_id(app, id, label, true, accel);
    let menu = Menu::with_items(
        app,
        &[
            &item("quick_speak", "Quick Speak…", None)?,
            &item("reader", "Open Reader", None)?,
            &item("speak_selection", "Speak Selected Text", None)?,
            &item("stop", "Stop Speaking", None)?,
            &PredefinedMenuItem::separator(app)?,
            &item("sources", "Sources…", None)?,
            &item("voices", "Voices…", None)?,
            &item("pronunciation", "Pronunciation Studio…", None)?,
            &PredefinedMenuItem::separator(app)?,
            &item("settings", "Preferences…", Some("Cmd+,"))?,
            &PredefinedMenuItem::separator(app)?,
            &item("quit", "Quit Speech Hub", Some("Cmd+Q"))?,
        ],
    )?;
    let icon = Image::from_bytes(ICON)?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Speech Hub")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quick_speak" => windows::show(app, windows::PALETTE),
            "reader" => windows::show(app, windows::READER),
            "speak_selection" => {
                let _ = hub::get(app).speak_selection();
            }
            "stop" => hub::get(app).stop(),
            "sources" => windows::navigate(app, windows::SETTINGS, "sources"),
            "voices" => windows::navigate(app, windows::SETTINGS, "voices"),
            "pronunciation" => windows::navigate(app, windows::SETTINGS, "pronunciation"),
            "settings" => windows::navigate(app, windows::SETTINGS, "general"),
            "quit" => {
                hub::get(app).stop();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;
    let _ = app.get_webview_window("none");
    Ok(())
}
