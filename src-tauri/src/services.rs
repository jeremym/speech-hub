//! macOS Services menu: "Speak with Speech Hub" and "Open in Speech Hub Reader".
//!
//! The service entries live in `src-tauri/Info.plist` (`NSServices`). macOS hands us the selected
//! text on a dedicated pasteboard, which is how the Services menu works for every app; this is not
//! the general clipboard and nothing is written back. Useful in apps whose Accessibility selection
//! can't be read.

use crate::hub;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::NSObject};
use objc2_app_kit::{NSApplication, NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::{NSObjectProtocol, NSString};
use speechhub_protocol::SpeakOptions;
use tauri::AppHandle;

pub struct Ivars {
    app: AppHandle,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; ServicesProvider has no Drop impl.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SpeechHubServicesProvider"]
    #[ivars = Ivars]
    pub struct ServicesProvider;

    impl ServicesProvider {
        #[unsafe(method(speakText:userData:error:))]
        fn speak_text(&self, pboard: &NSPasteboard, _user_data: Option<&NSString>, _error: *mut *mut NSString) {
            if let Some(text) = read_text(pboard) {
                let options = SpeakOptions { title: Some("Services".to_string()), ..Default::default() };
                if let Err(e) = hub::get(&self.ivars().app).speak(&text, options) {
                    log::warn!("services speak: {e}");
                }
            }
        }

        #[unsafe(method(openInReader:userData:error:))]
        fn open_in_reader(&self, pboard: &NSPasteboard, _user_data: Option<&NSString>, _error: *mut *mut NSString) {
            if let Some(text) = read_text(pboard) {
                hub::get(&self.ivars().app).open(&text, Some("Services".to_string()));
            }
        }
    }

    unsafe impl NSObjectProtocol for ServicesProvider {}
);

impl ServicesProvider {
    fn new(mtm: MainThreadMarker, app: AppHandle) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { app });
        unsafe { msg_send![super(this), init] }
    }
}

fn read_text(pboard: &NSPasteboard) -> Option<String> {
    let s = unsafe { pboard.stringForType(NSPasteboardTypeString) }?;
    let s = s.to_string();
    if s.trim().is_empty() { None } else { Some(s) }
}

/// Register the provider. Main thread only; call once from setup.
pub fn install(app: &AppHandle) {
    let Some(mtm) = MainThreadMarker::new() else {
        log::warn!("services: not on the main thread");
        return;
    };
    let provider = ServicesProvider::new(mtm, app.clone());
    let ns_app = NSApplication::sharedApplication(mtm);
    unsafe { ns_app.setServicesProvider(Some(&provider)) };
    // The provider must outlive every service call; the app owns it for its whole life.
    std::mem::forget(provider);
}
