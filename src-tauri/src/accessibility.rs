//! macOS Accessibility: permission check and selected-text capture.
//!
//! Own minimal FFI instead of a binding crate: five functions, no version drift.
//! Never activates a window or touches the clipboard (PLAN.md §3, §9).

use core_foundation::{
    base::{CFGetTypeID, CFRelease, CFType, CFTypeRef, TCFType},
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFDictionaryRef},
    string::{CFString, CFStringRef},
};
use std::ffi::c_void;

type AXUIElementRef = *const c_void;
type AXError = i32;

const K_AX_ERROR_SUCCESS: AXError = 0;
const K_AX_ERROR_API_DISABLED: AXError = -25211;
const K_AX_ERROR_NO_VALUE: AXError = -25212;
const K_AX_ERROR_ATTRIBUTE_UNSUPPORTED: AXError = -25205;
const K_AX_ERROR_NOT_IMPLEMENTED: AXError = -25208;
const K_AX_ERROR_CANNOT_COMPLETE: AXError = -25204;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
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
    fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    static kAXTrustedCheckOptionPrompt: CFStringRef;
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AxError {
    #[error("Accessibility permission has not been granted to Speech Hub")]
    NotTrusted,
    #[error("No app or text field has keyboard focus")]
    NoFocusedElement,
    #[error("Nothing is selected, or this app doesn't expose its selection to Accessibility")]
    NoSelection,
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

    fn set_bool_attr(&self, name: &str, value: bool) -> bool {
        let attr = CFString::new(name);
        let v = if value { CFBoolean::true_value() } else { CFBoolean::false_value() };
        let err = unsafe { AXUIElementSetAttributeValue(self.0, attr.as_concrete_TypeRef(), v.as_CFTypeRef()) };
        err == K_AX_ERROR_SUCCESS
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

/// Read the current selection from whatever element has focus, without activating anything.
///
/// Browsers (Chromium, Gecko) and Electron apps build their accessibility tree lazily: until an
/// assistive tool announces itself, the web area reports no selection at all. Native text views
/// answer immediately, so we try once, and only then flip `AXEnhancedUserInterface` on the
/// frontmost app, wait for the tree, retry, and put the flag back the way we found it.
pub fn selected_text() -> Result<Selection, AxError> {
    if !is_trusted(false) {
        return Err(AxError::NotTrusted);
    }
    let system = Element::system_wide();
    let app = system.child("AXFocusedApplication").ok();
    let app_name = app.as_ref().and_then(|a| a.string_attr("AXTitle").ok());

    match capture_once(&system, app.as_ref()) {
        Ok(text) => {
            log::debug!("selection: direct");
            return Ok(Selection { text, app_name });
        }
        Err(AxError::NoFocusedElement) if app.is_none() => return Err(AxError::NoFocusedElement),
        Err(_) => {}
    }

    let Some(app) = app else { return Err(AxError::NoSelection) };
    let was_enhanced = app.bool_attr("AXEnhancedUserInterface").unwrap_or(false);
    let was_manual = app.bool_attr("AXManualAccessibility").unwrap_or(false);
    // Gecko honors AXEnhancedUserInterface; Chromium honors both (AXManualAccessibility is its own
    // switch that avoids the window-animation side effects of the enhanced flag).
    app.set_bool_attr("AXManualAccessibility", true);
    app.set_bool_attr("AXEnhancedUserInterface", true);
    let mut result = Err(AxError::NoSelection);
    for (i, delay_ms) in [40u64, 80, 150, 250].into_iter().enumerate() {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        match capture_once(&system, Some(&app)) {
            Ok(text) => {
                log::debug!("selection: after enabling browser accessibility (attempt {})", i + 1);
                result = Ok(Selection { text, app_name: app_name.clone() });
                break;
            }
            Err(e) => result = Err(e),
        }
    }
    if !was_enhanced {
        app.set_bool_attr("AXEnhancedUserInterface", false);
    }
    if !was_manual {
        app.set_bool_attr("AXManualAccessibility", false);
    }
    result
}

/// One capture attempt: focused element's selected text, then the range path, then the
/// focused window's web area (Gecko sometimes focuses a container rather than the document).
fn capture_once(system: &Element, app: Option<&Element>) -> Result<String, AxError> {
    let focused = match app {
        Some(a) => a.child("AXFocusedUIElement").or_else(|_| system.child("AXFocusedUIElement")),
        None => system.child("AXFocusedUIElement"),
    }
    .map_err(|e| match e {
        AxError::NoSelection => AxError::NoFocusedElement,
        other => other,
    })?;
    if let Some(t) = selection_of(&focused) {
        return Ok(t);
    }
    // Walk up a few levels: a focused link or button inside a selected paragraph won't carry the
    // selection, but the web area above it does.
    let mut el = focused;
    for _ in 0..6 {
        let Ok(parent) = el.child("AXParent") else { break };
        if let Some(t) = selection_of(&parent) {
            return Ok(t);
        }
        if parent.string_attr("AXRole").map(|r| r == "AXWindow").unwrap_or(false) {
            break;
        }
        el = parent;
    }
    Err(AxError::NoSelection)
}

fn selection_of(el: &Element) -> Option<String> {
    if let Ok(text) = el.string_attr("AXSelectedText") {
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    if let Ok(range) = el.attr("AXSelectedTextRange") {
        if let Ok(v) = el.param_attr("AXStringForRange", &range) {
            if let Some(text) = cf_to_string(&v) {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

/// Deep link into System Settings → Privacy & Security → Accessibility.
pub const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
