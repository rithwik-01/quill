// selection/macos.rs — macOS selection capture (vendored, MIT/Apache-2.0)
#![cfg(target_os = "macos")]

use tauri::AppHandle;

pub fn get_selected_text(app: &AppHandle) -> Result<String, String> {
    match try_ax_selected_text() {
        Ok(text) if !text.is_empty() => Ok(text),
        Ok(_) => super::get_selected_text_via_clipboard(app),
        Err(_) => super::get_selected_text_via_clipboard(app),
    }
}

use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use std::ffi::c_void;

const K_AX_ERROR_SUCCESS: i32 = 0;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> *const c_void;
    fn AXUIElementCopyAttributeValue(
        element: *const c_void,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> i32;
}

fn try_ax_selected_text() -> Result<String, String> {
    unsafe {
        // No AXIsProcessTrusted() pre-check here — the hotkey handler already
        // gated on it. Many apps simply don't implement AXSelectedText, so any
        // failure falls through to the clipboard path regardless of trust.
        let system_wide = AXUIElementCreateSystemWide();
        if system_wide.is_null() {
            return Err("AXUIElementCreateSystemWide returned null".into());
        }
        let focused_attr = CFString::new("AXFocusedUIElement");
        let mut focused_value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(
            system_wide,
            focused_attr.as_concrete_TypeRef(),
            &mut focused_value,
        );
        CFRelease(system_wide as CFTypeRef);
        if err != K_AX_ERROR_SUCCESS {
            return Err(format!("AXFocusedUIElement failed: {err}"));
        }
        if focused_value.is_null() {
            return Err("AXFocusedUIElement returned null".into());
        }
        let focused_element = focused_value as *const c_void;
        let selected_attr = CFString::new("AXSelectedText");
        let mut text_value: CFTypeRef = std::ptr::null();
        let err = AXUIElementCopyAttributeValue(
            focused_element,
            selected_attr.as_concrete_TypeRef(),
            &mut text_value,
        );
        CFRelease(focused_value);
        if err != K_AX_ERROR_SUCCESS {
            return Err(format!("AXSelectedText failed: {err}"));
        }
        if text_value.is_null() {
            return Err("AXSelectedText returned null".into());
        }
        let cf_str = CFString::wrap_under_create_rule(text_value as CFStringRef);
        Ok(cf_str.to_string())
    }
}
