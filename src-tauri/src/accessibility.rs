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
pub fn selected_text() -> Result<Selection, AxError> {
    if !is_trusted(false) {
        return Err(AxError::NotTrusted);
    }
    let system = Element::system_wide();
    let app_name = system
        .child("AXFocusedApplication")
        .ok()
        .and_then(|app| app.string_attr("AXTitle").ok());
    let focused = system.child("AXFocusedUIElement").map_err(|e| match e {
        AxError::NoSelection => AxError::NoFocusedElement,
        other => other,
    })?;

    // 1. Direct selected text (works in most Cocoa text views, WebKit, Chromium, Gecko).
    if let Ok(text) = focused.string_attr("AXSelectedText") {
        if !text.trim().is_empty() {
            return Ok(Selection { text, app_name });
        }
    }
    // 2. Range + parameterized string (some editors only implement this path).
    if let Ok(range) = focused.attr("AXSelectedTextRange") {
        if let Ok(v) = focused.param_attr("AXStringForRange", &range) {
            if let Some(text) = cf_to_string(&v) {
                if !text.trim().is_empty() {
                    return Ok(Selection { text, app_name });
                }
            }
        }
    }
    Err(AxError::NoSelection)
}

/// Deep link into System Settings → Privacy & Security → Accessibility.
pub const SETTINGS_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
