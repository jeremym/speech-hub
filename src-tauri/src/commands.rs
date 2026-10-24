//! Tauri commands: the frontend's only way to talk to the hub. Thin wrappers, no logic.

use crate::{
    accessibility, engines, hotkeys::{self, HotkeyStatus}, hub::{self, ReaderText}, settings::{Settings, SettingsStore}, windows,
};
use serde::Serialize;
use speechhub_protocol::{SpeakOptions, Status, VoiceInfo};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    code: String,
    message: String,
}

impl From<hub::HubError> for CommandError {
    fn from(e: hub::HubError) -> Self {
        CommandError { code: serde_json::to_value(e.code()).unwrap().as_str().unwrap_or("internal").into(), message: e.to_string() }
    }
}

impl From<String> for CommandError {
    fn from(message: String) -> Self {
        CommandError { code: "internal".into(), message }
    }
}

type R<T> = Result<T, CommandError>;

// ---------- settings ----------

#[tauri::command]
pub fn get_settings(store: State<SettingsStore>) -> Settings {
    store.get()
}

#[tauri::command]
pub fn update_settings(app: AppHandle, store: State<SettingsStore>, settings: Settings) -> R<Settings> {
    let before = store.get();
    let after = store.replace(settings)?;
    let hub = hub::get(&app);
    if before.hotkeys != after.hotkeys {
        hotkeys::apply(&app, &after);
    }
    if before.engine_profile != after.engine_profile {
        hub.load_engine(&after.engine_profile);
    }
    if before.voice_id != after.voice_id && !after.voice_id.is_empty() {
        hub.set_voice(&after.voice_id);
    }
    if before.speed != after.speed {
        hub.set_speed(after.speed);
    }
    if before.launch_at_login != after.launch_at_login {
        let al = app.autolaunch();
        let res = if after.launch_at_login { al.enable() } else { al.disable() };
        if let Err(e) = res {
            log::warn!("launch at login: {e}");
        }
    }
    Ok(after)
}

#[tauri::command]
pub fn complete_onboarding(app: AppHandle, store: State<SettingsStore>) -> R<Settings> {
    let s = store.update(|s| s.onboarding_complete = true)?;
    windows::hide(&app, windows::ONBOARDING);
    Ok(s)
}

// ---------- hub ----------

#[tauri::command]
pub fn hub_status(app: AppHandle) -> Status {
    hub::get(&app).status()
}

#[tauri::command]
pub fn hub_speak(app: AppHandle, text: String, options: Option<SpeakOptions>) -> R<String> {
    Ok(hub::get(&app).speak(&text, options.unwrap_or_default())?)
}

#[tauri::command]
pub fn hub_open(app: AppHandle, text: String, title: Option<String>) {
    hub::get(&app).open(&text, title);
}

#[tauri::command]
pub fn hub_stop(app: AppHandle) {
    hub::get(&app).stop();
}

#[tauri::command]
pub fn hub_pause(app: AppHandle) {
    hub::get(&app).pause();
}

#[tauri::command]
pub fn hub_resume(app: AppHandle) {
    hub::get(&app).resume();
}

#[tauri::command]
pub fn hub_toggle_pause(app: AppHandle) {
    hub::get(&app).toggle_pause();
}

#[tauri::command]
pub fn hub_speak_selection(app: AppHandle) -> R<String> {
    Ok(hub::get(&app).speak_selection()?)
}

#[tauri::command]
pub fn hub_voices(app: AppHandle) -> Vec<VoiceInfo> {
    hub::get(&app).voices()
}

#[tauri::command]
pub fn hub_preview_voice(app: AppHandle, voice_id: String) -> R<String> {
    Ok(hub::get(&app).preview_voice(&voice_id)?)
}

#[tauri::command]
pub fn hub_engines(app: AppHandle) -> Vec<engines::ProfileCard> {
    let hub = hub::get(&app);
    engines::catalog(hub.roots(), &recommended_profile())
}

#[tauri::command]
pub fn reader_take_pending(app: AppHandle) -> Option<ReaderText> {
    hub::get(&app).take_pending_reader()
}

// ---------- accessibility ----------

#[tauri::command]
pub fn ax_is_trusted() -> bool {
    accessibility::is_trusted(false)
}

#[tauri::command]
pub fn ax_request() -> bool {
    accessibility::is_trusted(true)
}

/// Open a System Settings pane. Allowlisted: no arbitrary URLs leave the app.
#[tauri::command]
pub fn open_system_settings(pane: String) -> R<()> {
    let url = match pane.as_str() {
        "accessibility" => accessibility::SETTINGS_URL,
        "spoken_content" => "x-apple.systempreferences:com.apple.preference.universalaccess?SpokenContent",
        "login_items" => "x-apple.systempreferences:com.apple.LoginItems-Settings.extension",
        _ => return Err(format!("unknown settings pane '{pane}'").into()),
    };
    std::process::Command::new("/usr/bin/open").arg(url).spawn().map(|_| ()).map_err(|e| e.to_string().into())
}

// ---------- hotkeys ----------

#[tauri::command]
pub fn hotkeys_apply(app: AppHandle, store: State<SettingsStore>) -> Vec<HotkeyStatus> {
    hotkeys::apply(&app, &store.get())
}

// ---------- windows ----------

#[tauri::command]
pub fn open_window(app: AppHandle, label: String, pane: Option<String>) {
    match pane {
        Some(p) => windows::navigate(&app, &label, &p),
        None => windows::show(&app, &label),
    }
}

#[tauri::command]
pub fn hide_window(app: AppHandle, label: String) {
    windows::hide(&app, &label);
}

// ---------- system ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub arch: String,
    pub memory_gb: f32,
    pub macos: String,
    pub recommended_profile: String,
    pub app_version: String,
    pub data_dir: String,
}

fn memory_gb() -> f32 {
    std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "hw.memsize"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<f64>().ok())
        .map(|b| (b / 1024.0 / 1024.0 / 1024.0) as f32)
        .unwrap_or(0.0)
}

pub fn recommended_profile() -> String {
    if std::env::consts::ARCH != "aarch64" || memory_gb() < 8.0 { "lite".into() } else { "standard".into() }
}

#[tauri::command]
pub fn system_info(app: AppHandle) -> SystemInfo {
    let macos = std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    SystemInfo {
        arch: std::env::consts::ARCH.into(),
        memory_gb: memory_gb(),
        macos,
        recommended_profile: recommended_profile(),
        app_version: app.package_info().version.to_string(),
        data_dir: app.path().app_data_dir().map(|p| p.display().to_string()).unwrap_or_default(),
    }
}

// ---------- command line tool ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    /// Path of the `speak` binary shipped inside the app.
    pub bundled: Option<String>,
    /// Where the symlink goes.
    pub link: String,
    pub installed: bool,
    /// Does the login shell's PATH include the link directory?
    pub in_path: bool,
    pub socket: String,
}

fn bundled_cli() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let direct = dir.join("speak");
    if direct.exists() {
        return Some(direct);
    }
    // `tauri dev` runs the unbundled binary; the sidecar sits next to the sources.
    let triple = format!("speak-{}-apple-darwin", std::env::consts::ARCH);
    let dev = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries").join(triple);
    dev.exists().then_some(dev)
}

fn cli_link_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(|| "/tmp".into());
    home.join(".local").join("bin").join("speak")
}

fn login_shell_path_contains(dir: &std::path::Path) -> bool {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
    let out = std::process::Command::new(shell).args(["-lc", "printf %s \"$PATH\""]).output();
    let path = out.ok().and_then(|o| String::from_utf8(o.stdout).ok()).unwrap_or_default();
    let want = dir.display().to_string();
    let home = std::env::var("HOME").unwrap_or_default();
    path.split(':').any(|p| p == want || p.replacen('~', &home, 1) == want)
}

#[tauri::command]
pub fn cli_status(app: AppHandle) -> CliStatus {
    let link = cli_link_path();
    let installed = std::fs::read_link(&link).map(|t| t.exists()).unwrap_or(false);
    let socket = app.path().app_data_dir().map(|d| d.join("run/hub.sock").display().to_string()).unwrap_or_default();
    CliStatus {
        bundled: bundled_cli().map(|p| p.display().to_string()),
        in_path: login_shell_path_contains(link.parent().unwrap()),
        link: link.display().to_string(),
        installed,
        socket,
    }
}

/// Symlink ~/.local/bin/speak → the bundled binary. No sudo, no copying (updates stay in sync).
#[tauri::command]
pub fn cli_install(app: AppHandle) -> R<CliStatus> {
    let target = bundled_cli().ok_or_else(|| "the speak binary is not bundled with this build".to_string())?;
    let link = cli_link_path();
    if let Some(dir) = link.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    match std::fs::symlink_metadata(&link) {
        Ok(m) if m.file_type().is_symlink() => std::fs::remove_file(&link).map_err(|e| e.to_string())?,
        Ok(_) => return Err(format!("{} exists and is not a symlink; remove it first", link.display()).into()),
        Err(_) => {}
    }
    std::os::unix::fs::symlink(&target, &link).map_err(|e| e.to_string())?;
    Ok(cli_status(app))
}

#[tauri::command]
pub fn cli_uninstall(app: AppHandle) -> R<CliStatus> {
    let link = cli_link_path();
    if let Ok(m) = std::fs::symlink_metadata(&link) {
        if m.file_type().is_symlink() {
            std::fs::remove_file(&link).map_err(|e| e.to_string())?;
        }
    }
    Ok(cli_status(app))
}
