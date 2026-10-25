//! `speechhub://` URL scheme. Deep links are untrusted: no text beyond a short query, no paths,
//! no command execution. Running a Source from a link always requires confirmation (M4).

use crate::{hub, windows};
use speechhub_protocol::SpeakOptions;
use std::{sync::Mutex, time::{Duration, Instant}};
use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;

const MAX_LINK_TEXT: usize = 2048;
static LAST_SPEAK: Mutex<Option<Instant>> = Mutex::new(None);
/// Window-showing links are limited too: once a browser has "always allow" for the scheme, a
/// page looping `location = "speechhub://palette"` must not be able to steal focus continuously.
static LAST_WINDOW: Mutex<Option<Instant>> = Mutex::new(None);

fn window_link_allowed() -> bool {
    let mut last = LAST_WINDOW.lock().unwrap();
    if last.map(|t| t.elapsed() < Duration::from_secs(2)).unwrap_or(false) {
        return false;
    }
    *last = Some(Instant::now());
    true
}

pub fn install(app: &AppHandle) {
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            handle_url(&handle, url.as_str());
        }
    });
}

pub(crate) fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                // Slice bytes, not the str: `%` followed by a multibyte char must not panic.
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                if let Some(v) = hex {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(b'%');
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(url: &str, key: &str) -> Option<String> {
    let q = url.split_once('?')?.1;
    q.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

pub fn handle_url(app: &AppHandle, url: &str) {
    let Some(rest) = url.strip_prefix("speechhub://") else { return };
    let path = rest.split('?').next().unwrap_or("").trim_matches('/');
    log::info!("deep link: {path}");
    let shows_window = matches!(path, "" | "open" | "reader" | "palette" | "quick-speak" | "preferences" | "settings");
    if shows_window && !window_link_allowed() {
        log::info!("deep link ignored (rate limit): {path}");
        return;
    }
    match path {
        "" | "open" | "reader" => windows::show(app, windows::READER),
        "palette" | "quick-speak" => hub::get(app).open_palette(),
        "preferences" | "settings" => windows::show(app, windows::SETTINGS),
        "stop" => hub::get(app).stop(),
        "speak" => {
            let Some(text) = query_param(url, "text") else { return };
            if text.len() > MAX_LINK_TEXT {
                hub::get(app).notice("Link text too long (2 KB max). Use the speak command instead.".into());
                return;
            }
            // One speak link per second: a page can't machine-gun the speaker.
            let mut last = LAST_SPEAK.lock().unwrap();
            if last.map(|t| t.elapsed() < Duration::from_secs(1)).unwrap_or(false) {
                return;
            }
            *last = Some(Instant::now());
            // Off the main thread: `speak` may wait for the engine to load.
            let hub = hub::get(app).clone();
            std::thread::spawn(move || {
                let _ = hub.speak(&text, SpeakOptions { title: Some("From link".into()), ..Default::default() });
            });
        }
        p if p.starts_with("source/") => {
            let id = p.trim_start_matches("source/").to_string();
            crate::sources::confirm_and_run_from_link(app, &id);
        }
        other => log::warn!("unknown deep link path: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_query() {
        assert_eq!(query_param("speechhub://speak?text=Hello%20world%21&x=1", "text").as_deref(), Some("Hello world!"));
        assert_eq!(query_param("speechhub://speak?text=a+b", "text").as_deref(), Some("a b"));
        assert!(query_param("speechhub://speak", "text").is_none());
    }
}
