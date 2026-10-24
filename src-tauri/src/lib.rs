//! Speech Hub desktop app. See PLAN.md for the architecture.

mod accessibility;
mod commands;
mod deeplink;
pub mod engines;
mod hotkeys;
mod hub;
mod ipc;
mod playback;
mod settings;
mod sources;
pub mod text;
mod tray;
mod windows;

use tauri::{Manager, WindowEvent};

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    tauri::Builder::default()
        // Must be first: a second launch (e.g. from the CLI in M3) forwards here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::show(app, windows::SETTINGS);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().with_handler(hotkeys::handler).build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_deep_link::init())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let settings = settings::SettingsStore::load(&data_dir);
            app.manage(settings.clone());

            let bundled = app.path().resource_dir().ok().map(|r| r.join("resources").join("models"));
            let roots = engines::ModelRoots::new(bundled, data_dir.join("models"));
            log::info!("model roots: {:?}", roots.roots);
            let hub = hub::Hub::new(app.handle().clone(), settings.clone(), roots)
                .map_err(|e| std::io::Error::other(e))?;
            app.manage(hub);

            tray::build(app.handle())?;
            hotkeys::apply(app.handle(), &settings.get());
            if let Err(e) = ipc::start(app.handle().clone(), &data_dir) {
                log::error!("ipc server failed to start: {e}");
            }
            deeplink::install(app.handle());

            if !settings.get().onboarding_complete {
                windows::show(app.handle(), windows::ONBOARDING);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing a window hides it; the app lives in the menu bar.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if windows::ALL.contains(&window.label()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::complete_onboarding,
            commands::hub_status,
            commands::hub_speak,
            commands::hub_open,
            commands::hub_stop,
            commands::hub_pause,
            commands::hub_resume,
            commands::hub_toggle_pause,
            commands::hub_speak_selection,
            commands::hub_voices,
            commands::hub_preview_voice,
            commands::hub_engines,
            commands::reader_take_pending,
            commands::ax_is_trusted,
            commands::ax_request,
            commands::open_system_settings,
            commands::hotkeys_apply,
            commands::open_window,
            commands::hide_window,
            commands::system_info,
            commands::cli_status,
            commands::cli_install,
            commands::cli_uninstall,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Speech Hub");
}
