// selection/mod.rs — vendored selection capture for Quill v1
//
// PLAN §4: vendor, do not depend on `get-selected-text` (yetone, v0.1.6,
// 2024-05-29, MIT/Apache-2.0). The crate is ~200 lines, returns bare `()`
// errors, and must integrate clipboard save/restore anyway. Copy it here
// and own it.
//
// License of the original: MIT OR Apache-2.0 (yetone/get-selected-text).
// This vendored file is derived from that implementation; the original
// license applies to the borrowed algorithm.
//
// Algorithm (PLAN §4):
//   macOS  — try Accessibility API first (AXUIElementCopyAttributeValue for
//            kAXSelectedTextAttribute on focused element). Many apps don't
//            implement it; on any failure fall back to clipboard path.
//   Windows — clipboard path only.
//   Clipboard path — save current clipboard → send Cmd+C/Ctrl+C via enigo
//            → poll clipboard up to 300 ms for change → read → restore
//            saved contents. Restore MUST run on every exit path (including
//            errors/panics) via a Drop guard, not a trailing statement.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

// ---------------------------------------------------------------------------
// Public entry point — called from a Tauri command
// ---------------------------------------------------------------------------

/// Capture the user's current selection, regardless of which app is focused.
///
/// Returns `Ok("")` when nothing is selected (caller should toast
/// "Select some text first." per failure matrix §11). Returns `Err(msg)`
/// only for unrecoverable errors (e.g. clipboard unavailable, enigo init
/// failed). On macOS an Accessibility permission denial is NOT an error
/// here — we silently fall back to the clipboard path; the onboarding
/// check uses `AXIsProcessTrusted()` directly to prompt the user.
pub fn get_selected_text(app: &AppHandle) -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        return macos::get_selected_text(app);
    }
    #[cfg(target_os = "windows")]
    {
        return windows::get_selected_text(app);
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err("selection capture not supported on this platform".into())
    }
}

// ---------------------------------------------------------------------------
// Drop guard — guarantees clipboard restoration
// ---------------------------------------------------------------------------

/// Saves the current clipboard and restores it on drop.
///
/// Must be held on the stack for the entire clipboard-path operation.
/// Because `Drop` runs on every exit path — early return, `?`, panic
/// unwind — this satisfies PLAN §4 "must run on every exit path
/// including errors and panics. Implement it as a guard struct with
/// a Drop impl, not a trailing statement."
struct ClipboardGuard {
    app: AppHandle,
    saved_text: Option<String>,
    // Only populated when there was no text (avoids decoding bitmap on
    // the common text path, matching Handy clipboard.rs optimisation).
    saved_image: Option<tauri::image::Image<'static>>,
    // Set to true if we intentionally want to keep the new clipboard
    // (used only by replace.rs, never by capture — capture always restores).
    defused: bool,
}

impl ClipboardGuard {
    fn new(app: &AppHandle) -> Self {
        let clipboard = app.clipboard();
        let saved_text = clipboard.read_text().ok().filter(|t| !t.is_empty());
        let saved_image = if saved_text.is_none() {
            clipboard.read_image().ok().map(|img| img.to_owned())
        } else {
            None
        };
        Self {
            app: app.clone(),
            saved_text,
            saved_image,
            defused: false,
        }
    }

    /// Prevent restoration (used if caller decides to keep new contents).
    #[allow(dead_code)]
    fn defuse(mut self) {
        self.defused = true;
        std::mem::forget(self);
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        if self.defused {
            return;
        }
        let clipboard = self.app.clipboard();
        if let Some(text) = &self.saved_text {
            let _ = clipboard.write_text(text);
        } else if let Some(image) = &self.saved_image {
            let _ = clipboard.write_image(image);
        } else {
            // Clipboard was empty before we hijacked it — don't leave the
            // selection behind.
            let _ = clipboard.clear();
        }
    }
}

// ---------------------------------------------------------------------------
// Clipboard path — shared by macOS fallback and Windows
// ---------------------------------------------------------------------------

/// Poll interval while waiting for the target app to update the clipboard
/// after we send Cmd+C / Ctrl+C.
const POLL_INTERVAL: Duration = Duration::from_millis(15);
/// Maximum time to wait for the clipboard to change.
const POLL_TIMEOUT: Duration = Duration::from_millis(300);

/// Vendored clipboard path. Saves clipboard (via `ClipboardGuard`), sends
/// Cmd+C / Ctrl+C through `enigo`, then polls the clipboard for up to
/// 300 ms for a change.
///
/// A temporary empty placeholder is written before sending the copy so that
/// the case "selected text == previous clipboard contents" is not
/// mis-detected as "no selection". The guard still restores the TRUE
/// original contents, so the placeholder never leaks.
pub(crate) fn get_selected_text_via_clipboard(app: &AppHandle) -> Result<String, String> {
    // Guard is created first so its Drop restores the TRUE original even
    // if any step below returns Err or panics.
    let _guard = ClipboardGuard::new(app);

    let clipboard = app.clipboard();

    // Snapshot the pre-copy text for change detection. We already saved
    // the original in the guard; now we write a placeholder so an
    // identical-text selection is still detected as a change.
    // If writing the placeholder fails we still try the copy — the
    // clipboard may still change.
    let _ = clipboard.write_text("");

    // Brief pause to let the placeholder settle (avoids racing the
    // clipboard daemon on macOS).
    std::thread::sleep(Duration::from_millis(20));

    send_copy_keystroke()?;

    // Poll for a change from the placeholder.
    let start = Instant::now();
    loop {
        std::thread::sleep(POLL_INTERVAL);

        match clipboard.read_text() {
            Ok(current) if !current.is_empty() => {
                // Any non-empty string different from the placeholder means
                // the app responded to Cmd+C.
                return Ok(current);
            }
            Ok(_) => {} // still empty — no selection or app hasn't responded yet
            Err(_) => {} // transient read error — keep polling
        }

        if start.elapsed() >= POLL_TIMEOUT {
            break;
        }
    }

    // Final read after timeout — if still empty, treat as "no selection"
    // (failure matrix: toast "Select some text first.", not an error).
    match clipboard.read_text() {
        Ok(text) if !text.is_empty() => Ok(text),
        _ => Ok(String::new()),
    }
    // `_guard` drops here and restores the original clipboard contents.
}

// ---------------------------------------------------------------------------
// enigo keystroke — virtual key codes copied from Handy input.rs:35-60
// ---------------------------------------------------------------------------

/// Send Cmd+C (macOS) or Ctrl+C (Windows) via enigo 0.6.1.
///
/// Virtual key codes are taken from Handy `src-tauri/src/input.rs`:
///   macOS:   Key::Meta + Key::Other(8)   — 8 = kVK_ANSI_C
///   Windows: Key::Control + Key::Other(0x43) — 0x43 = VK_C
///
/// Uses `Key::Other` with raw VK codes on purpose: it bypasses layout
/// translation (Russian, AZERTY, Dvorak) and matches Handy's paste
/// path which uses Key::Other(9) for V (kVK_ANSI_V) and 0x56 for VK_V.
/// `Key::Unicode('c')` would be layout-dependent and is not used.
fn send_copy_keystroke() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("failed to init enigo: {e}"))?;

    // Defensive: release stray modifiers that could corrupt the chord
    // (mirrors get-selected-text utils::up_control_keys).
    let _ = enigo.key(Key::Control, Direction::Release);
    let _ = enigo.key(Key::Alt, Direction::Release);
    let _ = enigo.key(Key::Shift, Direction::Release);
    let _ = enigo.key(Key::Meta, Direction::Release);

    #[cfg(target_os = "macos")]
    {
        enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| format!("failed to press Cmd: {e}"))?;
        enigo
            .key(Key::Other(8), Direction::Click)
            .map_err(|e| format!("failed to click C: {e}"))?;
        // Chord hold — 10 ms is enough for the target to see the flags;
        // Handy uses 100 ms for paste because some apps poll global state.
        // Copy has no such requirement.
        std::thread::sleep(Duration::from_millis(10));
        enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| format!("failed to release Cmd: {e}"))?;
    }

    #[cfg(target_os = "windows")]
    {
        enigo
            .key(Key::Control, Direction::Press)
            .map_err(|e| format!("failed to press Ctrl: {e}"))?;
        enigo
            .key(Key::Other(0x43), Direction::Click)
            .map_err(|e| format!("failed to click C: {e}"))?;
        std::thread::sleep(Duration::from_millis(10));
        enigo
            .key(Key::Control, Direction::Release)
            .map_err(|e| format!("failed to release Ctrl: {e}"))?;
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = enigo;
        return Err("copy keystroke not supported on this platform".into());
    }

    Ok(())
}
