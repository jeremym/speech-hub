//! `speechhub://` URL scheme. Deep links are untrusted: no text beyond a short query, no paths,
//! no command execution. Running a Source from a link always requires confirmation (M4).

use crate::{hub, windows};
use speechhub_protocol::SpeakOptions;
use std::{sync::Mutex, time::{Duration, Instant}};
use tauri::AppHandle;
use tauri_plugin_deep_link::DeepLinkExt;

const MAX_LINK_TEXT: usize = 2048;
static LAST_SPEAK: Mutex<Option<Instant>> = Mutex::new(None);

pub fn install(app: &AppHandle) {
    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            handle_url(&handle, url.as_str());
        }
    });
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() + 0 && i + 2 <= bytes.len() - 1 + 0 => {
                if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
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
    match path {
        "" | "open" | "reader" => windows::show(app, windows::READER),
        "palette" | "quick-speak" => windows::show(app, windows::PALETTE),
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
            let _ = hub::get(app).speak(&text, SpeakOptions { title: Some("From link".into()), ..Default::default() });
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
