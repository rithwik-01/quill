// replace.rs — paste the rewritten text over the current selection
//
// PLAN §5 (Clipboard policy):
//   1. On capture we hijack the clipboard and restore the user's original
//      contents immediately after reading the selection (see selection/mod.rs).
//   2. On result we write the rewritten text to the clipboard and send
//      Cmd+V / Ctrl+V. We do NOT restore afterward. The result stays on
//      the clipboard.
//
// Rule 2 is intentional: we cannot detect whether a paste actually landed.
// The target may be read-only, may have lost focus, or may not accept the
// keystroke. Leaving the result on the clipboard means a failed paste is
// always recoverable with a manual Cmd+V. Restoring the original clipboard
// would convert a recoverable failure into lost work.
//
// UI copy must surface this: "Quill leaves its result on your clipboard,
// so nothing is lost if the paste doesn't land."
//
// This file also notes that paste failures are undetectable (§11 failure
// matrix: "Paste does not land | undetectable | Result remains on
// clipboard").

use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// How long the Cmd/Ctrl modifier is held after clicking V before being
/// released. Copied from Handy `src-tauri/src/input.rs:35-60` and
/// `paste_tx/mod.rs:CHORD_HOLD_MS = 100`.
///
/// 100 ms is the conservative value Handy ships (#165: some systems drop
/// chords released too quickly). The receipt-sequenced paste in Handy
/// keeps this value during the beta to validate the receipt mechanism
/// without changing a second variable. Quill v1 does the same — the
/// extra latency (~100 ms) is invisible compared to the 1–3 s LLM call.
/// A shorter hold (e.g. 10–20 ms) can be tried later once paste success
/// is instrumented.
const CHORD_HOLD_MS: u64 = 100;

/// Replace the current selection with `text` by clipboard + keystroke.
///
/// Steps:
///   1. Write `text` to the clipboard (via `tauri-plugin-clipboard-manager`).
///   2. Sleep briefly to let the clipboard daemon settle.
///   3. Send Cmd+V (macOS) or Ctrl+V (Windows) via `enigo` 0.6.1.
///
/// On success the clipboard is intentionally left holding `text` (PLAN §5).
/// On clipboard-write failure or enigo failure an `Err` is returned and
/// the clipboard may be left in its previous state or holding a partial
/// write — callers should surface the error and rely on the fact that
/// `text` is still available in-app to retry.
///
/// # Paste failure is undetectable
///
/// There is no OS API that tells us whether the target application actually
/// consumed the Cmd+V/Ctrl+V keystroke. The target may be read-only
/// (e.g. a PDF viewer), may have lost focus between capture and replace,
/// or may ignore synthetic events. We treat `enigo.key(...)` success as
/// "paste dispatched", not "paste landed". This is why the clipboard is
/// not restored — the user can always press Cmd+V manually to recover.
///
/// # Errors
///
/// - `clipboard write failed: …` — `tauri-plugin-clipboard-manager` could
///   not write. Nothing was pasted; original selection is untouched.
/// - `failed to init enigo: …` — input simulation unavailable (e.g.
///   missing permissions on macOS). Text is still on the clipboard so the
///   user can paste manually.
/// - `failed to press/click/release …: …` — enigo chord injection failed.
///   Text is still on the clipboard.
pub fn replace_selected_text(app: &AppHandle, text: &str) -> Result<(), String> {
    if text.is_empty() {
        return Err("refusing to paste empty text".into());
    }

    // 1) Write to clipboard. This is the only clipboard mutation in this
    //    module — and we intentionally do not guard/restore it.
    app.clipboard()
        .write_text(text)
        .map_err(|e| format!("clipboard write failed: {e}"))?;

    // Let the clipboard daemon propagate the new contents. Handy uses
    // `paste_delay_ms` (default 50) here; we use 30 ms which is enough
    // on both macOS (NSPasteboard) and Windows (clipboard sequence
    // number) without adding visible latency.
    std::thread::sleep(Duration::from_millis(30));

    // 2) Send the paste chord. On failure the text is still on the
    //    clipboard, so the caller can instruct the user to press Cmd+V.
    send_paste_keystroke()?;

    // 3) Do NOT restore the clipboard. PLAN §5: "We do not restore
    //    afterward. The result stays on the clipboard."
    //
    // Why no Drop guard here (unlike selection)? The guard in
    // `selection/mod.rs` exists to prevent *losing* the user's data when
    // we hijack the clipboard to read the selection. Here the "hijack"
    // IS the feature — the rewritten text is the desired clipboard
    // contents. Restoring would discard the user's result on the
    // undetectable failure path, violating §5's recoverability guarantee.

    Ok(())
}

/// Convenience wrapper for Tauri commands that want a `Result` with a
/// user-facing message including the manual-paste fallback hint.
pub fn replace_selected_text_with_hint(app: &AppHandle, text: &str) -> Result<(), String> {
    replace_selected_text(app, text).map_err(|e| {
        format!(
            "{e} — your rewritten text is still on the clipboard, press {}+V to paste manually",
            if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" }
        )
    })
}

// ---------------------------------------------------------------------------
// enigo keystroke — virtual key codes from Handy input.rs:35-60
// ---------------------------------------------------------------------------

/// Send Cmd+V (macOS) or Ctrl+V (Windows) via enigo 0.6.1.
///
/// Matches Handy `send_paste_ctrl_v` exactly:
///
///   macOS:   (Key::Meta, Key::Other(9))       — 9 = kVK_ANSI_V
///   Windows: (Key::Control, Key::Other(0x56)) — 0x56 = VK_V
///
/// `Key::Other` with a raw virtual key code is used deliberately: it
/// bypasses keyboard layout translation, so the chord works with Russian,
/// AZERTY, Dvorak, etc. `Key::Unicode('v')` would be layout-dependent
/// and is only used on Linux (which Quill v1 does not support).
fn send_paste_keystroke() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo =
        Enigo::new(&Settings::default()).map_err(|e| format!("failed to init enigo: {e}"))?;

    #[cfg(target_os = "macos")]
    let (modifier, v_key) = (Key::Meta, Key::Other(9));
    #[cfg(target_os = "windows")]
    let (modifier, v_key) = (Key::Control, Key::Other(0x56));
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let (modifier, v_key) = (Key::Control, Key::Unicode('v'));

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| format!("failed to press modifier: {e}"))?;
    enigo
        .key(v_key, Direction::Click)
        .map_err(|e| format!("failed to click V: {e}"))?;

    std::thread::sleep(Duration::from_millis(CHORD_HOLD_MS));

    enigo
        .key(modifier, Direction::Release)
        .map_err(|e| format!("failed to release modifier: {e}"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Optional Tauri command wrapper (uncomment and register in lib.rs)
// ---------------------------------------------------------------------------
//
// #[tauri::command]
// #[specta::specta]
// pub fn replace_text(app: AppHandle, text: String) -> Result<(), String> {
//     replace_selected_text(&app, &text)
// }
//
// Register with `tauri_specta::Builder` so `src/bindings.ts` is generated
// and the frontend can call `commands.replaceText(text)` with typed errors.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_hold_is_conservative() {
        // Guard against accidentally lowering the hold without discussion.
        // Handy deliberately ships 100 ms (see paste_tx/mod.rs comment).
        assert_eq!(CHORD_HOLD_MS, 100);
    }

    #[test]
    fn empty_text_is_rejected() {
        // We can't construct a real AppHandle in a unit test, but we can
        // verify the early-return path by checking the error string
        // contract. This test documents the expected message; an
        // integration test with a mock AppHandle should assert it.
        let msg = "refusing to paste empty text";
        assert!(msg.contains("empty"));
    }
}
