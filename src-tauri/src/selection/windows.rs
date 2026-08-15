// selection/windows.rs — Windows selection capture
//
// Vendored from yetone/get-selected-text/src/windows.rs (MIT/Apache-2.0).
//
// Original windows.rs was:
//
//   pub fn get_selected_text() -> Result<String, Box<dyn Error>> {
//       let mut enigo = Enigo::new(&Settings::default()).unwrap();
//       crate::utils::get_selected_text_by_clipboard(&mut enigo, false)
//   }
//
// `utils::get_selected_text_by_clipboard` saved clipboard via `arboard`,
// wrote a placeholder, sent Ctrl+C with enigo, slept 100 ms, read, then
// restored. This file preserves that algorithm but delegates to the shared
// `super::get_selected_text_via_clipboard` which adds the Drop guard and
// 300 ms poll required by PLAN §4.
//
// Windows has no Accessibility API equivalent for selected text, so the
// clipboard path is the only mechanism (PLAN §4: "Windows — clipboard
// path only").

#![cfg(target_os = "windows")]

use tauri::AppHandle;

/// Windows entry point — clipboard path only.
///
/// Always uses the guard-backed clipboard path in `super`. The guard
/// ensures the user's original clipboard (text or image) is restored
/// even if enigo fails or the function panics.
pub fn get_selected_text(app: &AppHandle) -> Result<String, String> {
    super::get_selected_text_via_clipboard(app)
}

// No additional Windows-specific helpers are needed. All clipboard and
// enigo logic lives in `selection/mod.rs` so the 300 ms poll, placeholder,
// and Drop guard are shared with the macOS fallback.
//
// If Windows-specific tuning is ever required (e.g. different poll
// intervals for Win32 clipboard daemon latency), add it here and call a
// Windows-specialised helper in `mod.rs`. For v1 the shared path is
// sufficient — Notepad, Word, Outlook, Chrome <textarea>, Slack, VS Code
// all respond to Ctrl+C within the 300 ms window per the manual smoke
// checklist (§12).
