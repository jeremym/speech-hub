//! Pre-warm lazy accessibility trees.
//!
//! Chromium builds its web-content accessibility tree only after an assistive tool sets
//! `AXEnhancedUserInterface` on the app, and then needs ~3 s (measured, Brave 155). Watching which
//! app is frontmost and setting the flag the moment a known browser comes forward means the first
//! Option+Escape in that browser is instant instead of a multi-second "waking up" wait.
//!
//! The list of apps is a seed of Chromium/Gecko browsers plus whatever apps needed the slow path
//! during this session (`accessibility::remember_lazy_app`). Nothing is read from those apps here;
//! we only set the flag.

use crate::{accessibility, settings::SettingsStore};
use std::time::Duration;

const POLL: Duration = Duration::from_millis(1000);

pub const KNOWN_LAZY_APPS: &[&str] = &[
    "com.brave.Browser",
    "com.brave.Browser.beta",
    "com.brave.Browser.nightly",
    "com.google.Chrome",
    "com.google.Chrome.beta",
    "com.google.Chrome.canary",
    "org.chromium.Chromium",
    "com.microsoft.edgemac",
    "com.microsoft.edgemac.Beta",
    "company.thebrowser.Browser",
    "com.vivaldi.Vivaldi",
    "com.operasoftware.Opera",
    "com.operasoftware.OperaGX",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "app.zen-browser.zen",
];

pub fn start(settings: SettingsStore) {
    let spawned = std::thread::Builder::new().name("ax-prewarm".into()).spawn(move || {
        let mut last_pid: libc::pid_t = 0;
        loop {
            std::thread::sleep(POLL);
            if !settings.get().prewarm_browsers || !accessibility::is_trusted(false) {
                // Nothing to do until the user turns it on or grants Accessibility; poll gently.
                std::thread::sleep(Duration::from_secs(5));
                continue;
            }
            let Some(pid) = accessibility::frontmost_pid() else { continue };
            if pid == last_pid {
                continue;
            }
            last_pid = pid;
            let Some(bundle_id) = accessibility::bundle_id(pid) else { continue };
            if KNOWN_LAZY_APPS.contains(&bundle_id.as_str()) || accessibility::is_lazy_app(&bundle_id) {
                if accessibility::enable_tree(pid) {
                    log::debug!("prewarm: asked {bundle_id} to build its accessibility tree");
                }
            }
        }
    });
    if let Err(e) = spawned {
        log::warn!("prewarm thread: {e}");
    }
}
