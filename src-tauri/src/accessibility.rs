//! accessibility.rs — macOS Accessibility permission, exactly as Apple documents it.
//!
//! Quill needs this permission to read the selection (AX API, see
//! `selection/macos.rs`) and to paste the result (synthetic Cmd+V via enigo).
//!
//! # The only two APIs involved
//!
//! - `AXIsProcessTrusted()` — the check.
//! - `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` —
//!   check + native prompt. The prompt is shown at most ONCE per app identity;
//!   the return value is the CURRENT state, never the post-dialog one, so the
//!   caller must re-check afterwards. The frontend does that by polling
//!   (`useAccessibility.ts`).
//!
//! Everything else is a fallback for the user, not for the code:
//! `x-apple.systempreferences:...?Privacy_Accessibility` to open the pane, and
//! `tccutil reset Accessibility <bundle-id>` to recover from a stale entry.
//!
//! # Why grants go stale (and why this file is short now)
//!
//! macOS keys the grant to the app's CODE SIGNATURE. An unsigned or ad-hoc
//! build gets a new identity on every rebuild, so the grant stops matching
//! while System Settings still shows a toggle belonging to the dead identity.
//! That is a build-configuration problem, fixed by `bundle.macOS.signingIdentity`
//! in tauri.conf.json (and `scripts/sign-dev.sh` for the bare `tauri dev`
//! binary, which is a separate TCC entry from Quill.app). It is NOT something
//! to paper over with probes or heuristics in here.
//!
//! # Uninstalling does not clear the grant — that's by design
//!
//! macOS keeps TCC entries after an app is deleted; there is no uninstall hook
//! and no API for an app to revoke its own grant. `tccutil reset Accessibility
//! com.quill.app` is Apple's supported way to clear it. For a signed release
//! this is the desired behavior: the grant survives app updates because the
//! signing identity and bundle ID stay stable.

use serde::Serialize;
use tauri::AppHandle;

/// Diagnostic snapshot for the frontend, shown when the permission looks stuck
/// (almost always a stale TCC entry from an identity change).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
#[specta(rename_all = "camelCase")]
pub struct AccessibilityStatus {
    /// `AXIsProcessTrusted()`.
    pub trusted: bool,
    /// The exact executable that needs the grant — TCC keys on this identity.
    pub executable_path: String,
    /// `false` when running the bare dev binary rather than `Quill.app`. The
    /// two are distinct TCC entries; granting one does not grant the other.
    pub bundled: bool,
    /// For the `tccutil reset Accessibility <id>` hint.
    pub bundle_identifier: String,
}

/// Bundled apps run as `.../Quill.app/Contents/MacOS/quill`; `tauri dev` runs
/// the bare `target/debug/quill`.
fn is_bundled(executable_path: &str) -> bool {
    executable_path.contains(".app/Contents/MacOS/")
}

fn current_exe_path() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// macOS implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
    use core_foundation::string::{CFString, CFStringRef};

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
        static kAXTrustedCheckOptionPrompt: CFStringRef;
    }

    /// `AXIsProcessTrusted()` — the canonical check. Reflects changes live in
    /// the running process, so no restart is ever needed after granting.
    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// `AXIsProcessTrustedWithOptions` with `kAXTrustedCheckOptionPrompt`.
    /// Shows the native dialog if this app identity has never been asked.
    /// Returns the CURRENT trust — the dialog is asynchronous, so a `false`
    /// here just means "not granted yet"; keep polling `is_trusted()`.
    pub fn request_with_prompt() -> bool {
        unsafe {
            let key = CFString::wrap_under_get_rule(kAXTrustedCheckOptionPrompt);
            let options = CFDictionary::from_CFType_pairs(&[(key, CFBoolean::true_value())]);
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef())
        }
    }

    /// Fallback for when the prompt was already shown once and will not
    /// reappear: send the user straight to the pane.
    pub fn open_settings_pane() -> Result<(), String> {
        std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn status(app: &AppHandle) -> AccessibilityStatus {
        let executable_path = current_exe_path();
        AccessibilityStatus {
            trusted: is_trusted(),
            bundled: is_bundled(&executable_path),
            executable_path,
            bundle_identifier: app.config().identifier.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Non-macOS stubs (Windows has no equivalent permission)
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn is_trusted() -> bool {
        true
    }

    pub fn request_with_prompt() -> bool {
        true
    }

    pub fn open_settings_pane() -> Result<(), String> {
        Ok(())
    }

    pub fn status(app: &AppHandle) -> AccessibilityStatus {
        AccessibilityStatus {
            trusted: true,
            executable_path: current_exe_path(),
            bundled: true,
            bundle_identifier: app.config().identifier.clone(),
        }
    }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::is_bundled;

    #[test]
    fn distinguishes_app_bundle_from_dev_binary() {
        assert!(is_bundled("/Applications/Quill.app/Contents/MacOS/quill"));
        assert!(!is_bundled(
            "/Users/me/quill/src-tauri/target/debug/quill"
        ));
        // A `.app` somewhere in the path is not the same as being the bundle's
        // own executable — TCC only cares about the latter.
        assert!(!is_bundled("/Users/me/Some.app/Resources/tools/quill"));
    }
}
