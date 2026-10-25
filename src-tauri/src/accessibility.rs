//! macOS Accessibility: permission check and selected-text capture.
//!
//! Own minimal FFI instead of a binding crate: a handful of functions, no version drift.
//! Never activates a window or touches the clipboard (PLAN.md §3, §9).
//!
//! What real apps expose (measured 2026-10-25 with a probe, see docs/architecture.md):
//! - AppKit text views, Ghostty, browser URL bars: `AXSelectedText` / `AXSelectedTextRange`.
//! - Firefox: the focused `AXWebArea` answers only `AXSelectedTextMarkerRange` +
//!   `AXStringForTextMarkerRange` (the WebKit-style marker API).
//! - Chromium (Brave 155): no web content in the tree at all until an assistive tool sets
//!   `AXEnhancedUserInterface` on the app; the tree then takes ~3 s to appear. `AXManualAccessibility`
//!   is ignored. Once enabled, both `AXSelectedText` and the marker API work.

use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use std::collections::HashSet;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

type AXUIElementRef = *const c_void;
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_ERROR_API_DISABLED: AXError = -25211;
const K_AX_ERROR_NO_VALUE: AXError = -25212;
const K_AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25205;
const K_AX_ERROR_NOT_IMPLEMENTED: AXError = -25208;
const K_AX_ERROR_CANNOT_COMPLETE: AXError = -25204;

/// How long to wait for a browser to build its accessibility tree after we ask for it.
/// Brave 155 measured 3.3 s on a trivial page; heavy pages take longer.
pub const WARMUP_BUDGET: Duration = Duration::from_millis(6000);
const WARMUP_POLL: Duration = Duration::from_millis(100);

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> AXUIElementRef;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut libc::pid_t) -> AXError;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(element: AXUIElementRef, attribute: CFStringRef, value: CFTypeRef) -> AXError;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout_seconds: f32) -> AXError;
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AxError {
    #[error("Accessibility permission has not been granted to Speech Hub")]
    NotTrusted,
    #[error("No app or text field has keyboard focus")]
    NoFocusedElement,
    #[error("Nothing is selected")]
    NoSelection,
    #[error("This app doesn't expose its selected text to Accessibility")]
    Unavailable,
    #[error("Cancelled")]
    Cancelled,
    #[error("Accessibility error {0}")]
    Other(i32),
}

/// Is this process allowed to use Accessibility? `prompt` shows Apple's system dialog if not.
pub fn is_trusted(prompt: bool) -> bool {
    unsafe {
        let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
        let value = if prompt { CFBoolean::true_value() } else { CFBoolean::false_value() };
        let dict = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        AXIsProcessTrustedWithOptions(dict.as_concrete_TypeRef())
    }
}

/// Owned wrapper around an AXUIElementRef that releases on drop.
struct Element(AXUIElementRef);

// AXUIElementRef is an immutable CF token (pid + remote element id); the AX calls are
// synchronous Mach messages that are safe from any thread.
unsafe impl Send for Element {}

impl Drop for Element {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

impl Element {
    fn system_wide() -> Self {
        Element(unsafe { AXUIElementCreateSystemWide() })
    }

    fn application(pid: libc::pid_t) -> Self {
        Element(unsafe { AXUIElementCreateApplication(pid) })
    }

    fn pid(&self) -> Option<libc::pid_t> {
        let mut pid: libc::pid_t = 0;
        let err = unsafe { AXUIElementGetPid(self.0, &mut pid) };
        (err == K_AX_ERROR_SUCCESS && pid > 0).then_some(pid)
    }

    fn attr(&self, name: &str) -> Result<CFType, AxError> {
        let attr = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe { AXUIElementCopyAttributeValue(self.0, attr.as_concrete_TypeRef(), &mut out) };
        map_err(err)?;
        if out.is_null() {
            return Err(AxError::NoSelection);
        }
        Ok(unsafe { CFType::wrap_under_create_rule(out) })
    }

    fn param_attr(&self, name: &str, param: &CFType) -> Result<CFType, AxError> {
        let attr = CFString::new(name);
        let mut out: CFTypeRef = std::ptr::null();
        let err = unsafe {
            AXUIElementCopyParameterizedAttributeValue(
                self.0,
                attr.as_concrete_TypeRef(),
                param.as_CFTypeRef(),
                &mut out,
            )
        };
        map_err(err)?;
        if out.is_null() {
            return Err(AxError::NoSelection);
        }
        Ok(unsafe { CFType::wrap_under_create_rule(out) })
    }

    fn child(&self, name: &str) -> Result<Element, AxError> {
        let v = self.attr(name)?;
        // The CFType owns one retain; take it over as an Element.
        let ptr = v.as_CFTypeRef();
        std::mem::forget(v);
        Ok(Element(ptr))
    }

    fn string_attr(&self, name: &str) -> Result<String, AxError> {
        let v = self.attr(name)?;
        cf_to_string(&v).ok_or(AxError::NoSelection)
    }

    fn bool_attr(&self, name: &str) -> Option<bool> {
        let v = self.attr(name).ok()?;
        unsafe {
            if CFGetTypeID(v.as_CFTypeRef()) == CFBoolean::type_id() {
                let b = CFBoolean::wrap_under_get_rule(v.as_CFTypeRef() as _);
                Some(b.into())
            } else {
                None
            }
        }
    }

    /// Returns the raw AXError. Chromium answers `kAXErrorNotImplemented` for
    /// `AXEnhancedUserInterface` and still honors it, so callers must not trust the code.
    fn set_bool_attr(&self, name: &str, value: bool) -> AXError {
        let attr = CFString::new(name);
        let v = if value { CFBoolean::true_value() } else { CFBoolean::false_value() };
        unsafe { AXUIElementSetAttributeValue(self.0, attr.as_concrete_TypeRef(), v.as_CFTypeRef()) }
    }
}

fn cf_to_string(v: &CFType) -> Option<String> {
    unsafe {
        if CFGetTypeID(v.as_CFTypeRef()) == CFString::type_id() {
            let s = CFString::wrap_under_get_rule(v.as_CFTypeRef() as CFStringRef);
            Some(s.to_string())
        } else {
            None
        }
    }
}

fn map_err(err: AXError) -> Result<(), AxError> {
    match err {
        K_AX_ERROR_SUCCESS => Ok(()),
        K_AX_ERROR_API_DISABLED => Err(AxError::NotTrusted),
        K_AX_ERROR_NO_VALUE | K_AX_ERROR_ATTRIBUTE_UNSUPPORTED | K_AX_ERROR_NOT_IMPLEMENTED => {
            Err(AxError::NoSelection)
        }
        K_AX_ERROR_CANNOT_COMPLETE => Err(AxError::NoFocusedElement),
        other => Err(AxError::Other(other)),
    }
}

pub struct Selection {
    pub text: String,
    /// Name of the frontmost app, for the mini player title.
    pub app_name: Option<String>,
}

/// The app whose selection we want. Resolve it the instant the shortcut fires, before any of
/// our own windows become key; the capture itself can then run later, on any thread, because
/// an app's `AXFocusedUIElement` is per-app and doesn't care who is frontmost.
pub struct Target {
    app: Element,
    pub app_name: Option<String>,
    pub bundle_id: Option<String>,
}

/// Apps that needed the slow path this session; prewarm.rs warms them up when they come forward.
fn lazy_apps() -> &'static Mutex<HashSet<String>> {
    static L: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    L.get_or_init(|| Mutex::new(HashSet::new()))
}

pub fn is_lazy_app(bundle_id: &str) -> bool {
    lazy_apps().lock().map(|s| s.contains(bundle_id)).unwrap_or(false)
}

fn remember_lazy_app(bundle_id: &str) {
    if let Ok(mut s) = lazy_apps().lock() {
        s.insert(bundle_id.to_string());
    }
}

/// Pid of the app the AX server considers focused (falls back to AppKit's frontmost app).
pub fn frontmost_pid() -> Option<libc::pid_t> {
    Element::system_wide().child("AXFocusedApplication").ok().and_then(|a| a.pid()).or_else(frontmost_pid_appkit)
}

pub fn app_name(pid: libc::pid_t) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    Some(app.localizedName()?.to_string())
}

pub fn bundle_id(pid: libc::pid_t) -> Option<String> {
    use objc2_app_kit::NSRunningApplication;
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)?;
    let id = app.bundleIdentifier()?;
    Some(id.to_string())
}

/// Ask an app to build its accessibility tree now. Returns true if the flag was newly set.
pub fn enable_tree(pid: libc::pid_t) -> bool {
    let app = Element::application(pid);
    if app.bool_attr("AXEnhancedUserInterface") == Some(true) {
        return false;
    }
    app.set_bool_attr("AXEnhancedUserInterface", true);
    app.set_bool_attr("AXManualAccessibility", true);
    true
}

impl Target {
    pub fn frontmost() -> Result<Target, AxError> {
        if !is_trusted(false) {
            return Err(AxError::NotTrusted);
        }
        // Prefer the AX server's notion of focus; fall back to AppKit's frontmost app (the AX
        // query needs a window-server connection, which a bare CLI process lacks).
        let pid = Element::system_wide()
            .child("AXFocusedApplication")
            .ok()
            .and_then(|a| a.pid())
            .or_else(frontmost_pid_appkit)
            .ok_or(AxError::NoFocusedElement)?;
        Ok(Self::for_pid(pid))
    }

    /// Target a specific process (tests, or a caller that already knows the app).
    pub fn for_pid(pid: libc::pid_t) -> Target {
        let app = Element::application(pid);
        // The hotkey handler runs on the main thread: a hung target must cost at most a second,
        // not the AX default of six.
        unsafe { AXUIElementSetMessagingTimeout(app.0, 1.0) };
        // The name comes from AppKit (no IPC with the target), not from AXTitle.
        Target { app, app_name: app_name(pid), bundle_id: bundle_id(pid) }
    }

    /// Read the selection. Fast path first; if the app shows nothing, ask it to build its
    /// accessibility tree (browsers and Electron apps do this lazily) and poll until `budget`
    /// runs out. `on_slow` fires once when the slow path starts, so the UI can say why it's waiting.
    ///
    /// If the slow path produced the selection, the flag stays on: that app demonstrably needs it,
    /// and turning it back off would make every later capture pay the multi-second warm-up again.
    /// If it produced nothing, the flag is restored so ordinary apps are left exactly as found.
    ///
    /// Fails fast when the app answers "nothing selected" outright (a text view with an empty
    /// selection range); only an app that exposes no text at all is worth waking up.
    /// `cancel` aborts the wait (the mini player's stop button).
    pub fn capture(
        &self,
        budget: Duration,
        cancel: &AtomicBool,
        on_slow: impl FnOnce(Option<&str>),
    ) -> Result<Selection, AxError> {
        match capture_once(&self.app) {
            Ok(text) => {
                log::debug!("selection: direct");
                return Ok(self.selection(text));
            }
            Err(e @ (AxError::NotTrusted | AxError::NoSelection)) => return Err(e),
            Err(_) => {}
        }

        // `None` means the app would not say (Chromium answers NotImplemented). Only a flag we
        // read as `false` is restored to `false`: writing `false` blindly could switch off a flag
        // VoiceOver or Voice Control set, and they would not know.
        let was_enhanced = self.app.bool_attr("AXEnhancedUserInterface");
        let was_manual = self.app.bool_attr("AXManualAccessibility");
        // Chromium and Gecko honor AXEnhancedUserInterface; Electron honors AXManualAccessibility.
        let e1 = self.app.set_bool_attr("AXEnhancedUserInterface", true);
        let e2 = self.app.set_bool_attr("AXManualAccessibility", true);
        log::debug!("selection: enabling accessibility tree (enhanced={e1}, manual={e2}), waiting up to {budget:?}");
        on_slow(self.app_name.as_deref());

        let started = Instant::now();
        let mut result = Err(AxError::NoSelection);
        while started.elapsed() < budget {
            std::thread::sleep(WARMUP_POLL);
            if cancel.load(Ordering::SeqCst) {
                result = Err(AxError::Cancelled);
                break;
            }
            match capture_once(&self.app) {
                Ok(text) => {
                    log::debug!("selection: after warm-up ({} ms)", started.elapsed().as_millis());
                    if let Some(id) = &self.bundle_id {
                        remember_lazy_app(id);
                    }
                    result = Ok(self.selection(text));
                    break;
                }
                // The tree arrived and says nothing is selected: stop waiting.
                Err(AxError::NoSelection) => {
                    result = Err(AxError::NoSelection);
                    break;
                }
                Err(e) => result = Err(e),
            }
        }
        if result.is_err() {
            if was_enhanced == Some(false) {
                self.app.set_bool_attr("AXEnhancedUserInterface", false);
            }
            if was_manual == Some(false) {
                self.app.set_bool_attr("AXManualAccessibility", false);
            }
        }
        result
    }

    fn selection(&self, text: String) -> Selection {
        Selection { text, app_name: self.app_name.clone() }
    }
}

fn frontmost_pid_appkit() -> Option<libc::pid_t> {
    use objc2_app_kit::NSWorkspace;
    let ws = NSWorkspace::sharedWorkspace();
    let app = ws.frontmostApplication()?;
    Some(app.processIdentifier())
}

/// What one element says about the selection.
enum Probe {
    Found(String),
    /// The element speaks the text protocol and reports an empty selection: a real answer.
    Empty,
    /// The element has no text protocol at all.
    Unknown,
}

/// One capture attempt: the focused element's selection, then its ancestors up to the window.
/// A focused link or button inside a selected paragraph won't carry the selection, but the web
/// area above it does; Gecko sometimes focuses a container rather than the document.
///
/// `NoSelection` means some element in the chain answered "empty"; `Unavailable` or
/// `NoFocusedElement` means nobody exposed text, which is what a cold browser looks like.
fn capture_once(app: &Element) -> Result<String, AxError> {
    let focused = app.child("AXFocusedUIElement").map_err(|e| match e {
        AxError::NoSelection => AxError::NoFocusedElement,
        other => other,
    })?;
    let mut saw_empty = false;
    let mut el = focused;
    for depth in 0..7 {
        match selection_of(&el) {
            Probe::Found(t) => return Ok(t),
            Probe::Empty => saw_empty = true,
            Probe::Unknown => {}
        }
        if depth == 6 || el.string_attr("AXRole").map(|r| r == "AXWindow").unwrap_or(false) {
            break;
        }
        let Ok(parent) = el.child("AXParent") else { break };
        el = parent;
    }
    Err(if saw_empty { AxError::NoSelection } else { AxError::Unavailable })
}

/// Three dialects: plain `AXSelectedText` (AppKit, Chromium once awake), range + string
/// (AppKit, Ghostty), and text markers (Firefox, Safari/WebKit).
fn selection_of(el: &Element) -> Probe {
    let mut answered = false;
    if let Ok(text) = el.string_attr("AXSelectedText") {
        answered = true;
        if !text.trim().is_empty() {
            return Probe::Found(text);
        }
    }
    if let Ok(range) = el.attr("AXSelectedTextRange") {
        answered = true;
        if let Ok(v) = el.param_attr("AXStringForRange", &range) {
            if let Some(text) = cf_to_string(&v) {
                if !text.trim().is_empty() {
                    return Probe::Found(text);
                }
            }
        }
    }
    if let Ok(range) = el.attr("AXSelectedTextMarkerRange") {
        if let Ok(v) = el.param_attr("AXStringForTextMarkerRange", &range) {
            answered = true;
            if let Some(text) = cf_to_string(&v) {
                if !text.trim().is_empty() {
                    return Probe::Found(text);
                }
            }
        }
    }
    if answered { Probe::Empty } else { Probe::Unknown }
}

/// Deep link into System Settings → Privacy & Security → Accessibility.
pub const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
