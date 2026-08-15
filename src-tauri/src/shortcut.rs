//! shortcut.rs — global hotkey registration (PLAN.md §10 Phase 1)
//! Default: Cmd+Shift+. / Ctrl+Shift+.  Handled via tauri-plugin-global-shortcut.
//! Apple docs: global shortcuts do not need Accessibility, but selection capture
//! and paste do — the handler now surfaces those failures via `quill://*` events
//! so the UI can toast instead of silently doing nothing (the reported "nothing happens").

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const DEFAULT_HOTKEY: &str = "CommandOrControl+Shift+Period";

/// Canonicalize all hotkey spellings the JS UI might produce.
/// Handles: Cmd/Ctrl/CmdOrCtrl/CommandOrControl, "." ↔ Period, spacing, case.
/// The stored value is always `CommandOrControl+Shift+Period` so parsing is stable.
pub fn normalize_hotkey(s: &str) -> String {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return DEFAULT_HOTKEY.to_string();
    }
    // Protect the canonical token so we don't mangle it
    const PH: &str = "__CMDORCTRL__";
    let mut h = trimmed.replace("CommandOrControl", PH);
    // Also handle the JS default `CmdOrCtrl`
    h = h.replace("CmdOrCtrl", PH);
    // Standalone Cmd / Ctrl aliases
    h = h.replace("Cmd", "Command");
    // Handle raw Ctrl that remains
    h = h.replace("Ctrl", "Control");
    h = h.replace(PH, "CommandOrControl");

    // Normalize separators: allow both `+` and spacing; split, trim, remap key names
    // Split on '+' but keep robustness for "CommandOrControl + Shift + ."
    let parts: Vec<String> = h
        .split('+')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .map(|p| {
            // Key alias: "." -> Period, "period" -> Period, "," -> Comma etc.
            // For Quill v1 we only need "." -> Period, but handle common aliases.
            match p.as_str() {
                "." => "Period".to_string(),
                "," => "Comma".to_string(),
                "/" => "Slash".to_string(),
                ";" => "Semicolon".to_string(),
                "'" => "Quote".to_string(),
                "[" => "BracketLeft".to_string(),
                "]" => "BracketRight".to_string(),
                // case-insensitive aliases
                other => {
                    let lower = other.to_ascii_lowercase();
                    match lower.as_str() {
                        "period" | "." => "Period".to_string(),
                        "comma" => "Comma".to_string(),
                        "slash" => "Slash".to_string(),
                        "space" => "Space".to_string(),
                        "enter" | "return" => "Enter".to_string(),
                        "ctrl" | "control" => "Control".to_string(),
                        "cmd" | "command" => "Command".to_string(),
                        "cmdorctrl" | "commandorcontrol" => "CommandOrControl".to_string(),
                        "alt" | "option" => "Alt".to_string(),
                        "shift" => "Shift".to_string(),
                        "super" | "meta" => "Super".to_string(),
                        _ => {
                            // Normalize modifiers capitalization: command -> Command, shift -> Shift
                            // Keep code keys as-is but capitalize first letter
                            let cap = {
                                let mut c = other.chars();
                                match c.next() {
                                    None => String::new(),
                                    Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                                }
                            };
                            // Fixup known modifiers
                            match cap.as_str() {
                                "Command" => "Command".to_string(),
                                "Control" => "Control".to_string(),
                                "Alt" => "Alt".to_string(),
                                "Shift" => "Shift".to_string(),
                                "Super" => "Super".to_string(),
                                _ => cap,
                            }
                        }
                    }
                }
            }
        })
        .collect();

    // Also handle the case where "." was typed as a standalone key with leading dot char
    // e.g. "CommandOrControl+Shift+." already split into ["CommandOrControl","Shift","."] -> mapped above.

    let joined = parts.join("+");
    // Keep canonical but also normalize inner spelling: "Command+Control" shouldn't happen
    // Ensure we didn't produce empty
    if joined.is_empty() {
        DEFAULT_HOTKEY.to_string()
    } else {
        joined
    }
}

fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    let normalized = normalize_hotkey(s);
    normalized
        .as_str()
        .try_into()
        .map_err(|e| format!("invalid hotkey '{s}' (normalized '{normalized}'): {e:?}"))
}

pub fn register_default(app: &AppHandle) -> Result<(), String> {
    register_hotkey(app, DEFAULT_HOTKEY)
}

pub fn register_hotkey(app: &AppHandle, hotkey: &str) -> Result<(), String> {
    let normalized = normalize_hotkey(hotkey);
    let manager = app.global_shortcut();
    // Unregister existing to allow rebind — ignore errors if none registered
    let _ = manager.unregister_all();
    let shortcut = parse_shortcut(&normalized)?;
    manager
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                log::info!("hotkey pressed");
                handle_hotkey_pressed(app);
            }
        })
        .map_err(|e| format!("failed to register hotkey '{normalized}' (from '{hotkey}'): {e}. Try another chord like CommandOrControl+Shift+K"))?;
    log::info!("registered hotkey '{normalized}' (from '{hotkey}')");
    Ok(())
}

pub fn unregister_all(app: &AppHandle) {
    let _ = app.global_shortcut().unregister_all();
}

/// v1.1 flow: hotkey → capture → popup (Fix Grammar runs, result shown).
/// Nothing is pasted until the user Accepts inside the popup.
/// Errors are emitted to the popup (`quill://popup-error`) and, where the
/// popup may not be up yet, also to the main window (`quill://error`).
fn handle_hotkey_pressed(app: &AppHandle) {
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // 0. Accessibility gate — selection + paste both need it on macOS.
        // `AXIsProcessTrusted()` is Apple's answer and reflects changes live in
        // the running process, so no restart is needed after granting.
        #[cfg(target_os = "macos")]
        if !crate::accessibility::is_trusted() {
            log::warn!("hotkey: accessibility not trusted");
            let _ = app_handle.emit("quill://needs-permission", "Accessibility not granted — open System Settings");
            let _ = app_handle.emit(
                "quill://error",
                "Accessibility permission needed — open Settings → Privacy & Security → Accessibility, toggle Quill on, then press the hotkey again (no restart needed).",
            );
            return;
        }

        // 1. Capture selection
        let text = match crate::selection::get_selected_text(&app_handle) {
            Ok(t) => t,
            Err(e) => {
                log::error!("selection capture failed: {e}");
                let _ = app_handle.emit("quill://error", format!("Selection failed: {e}"));
                return;
            }
        };
        if text.trim().is_empty() {
            log::info!("no text selected");
            let _ = app_handle.emit("quill://no-selection", ());
            let _ = app_handle.emit("quill://error", "Select some text first, then press the hotkey.");
            return;
        }
        if text.len() > 6000 {
            log::warn!("selection is very long ({} chars) — proceeding anyway", text.len());
        }

        // 2. Open the popup near the cursor in its working state.
        // show_popup captures the frontmost app before stealing focus.
        if let Err(e) = crate::popup::show_popup(&app_handle) {
            log::error!("failed to show popup: {e}");
            let _ = app_handle.emit("quill://error", format!("Could not open popup: {e}"));
            return;
        }
        let _ = app_handle.emit("quill://popup-text", &text);

        // 3. Check Ollama liveness for a fast, friendly error
        let settings = crate::settings::load_settings_snapshot(&app_handle);
        let client = crate::ollama::OllamaClient::default_local();
        if let Err(e) = client.version().await {
            log::error!("ollama not running: {e}");
            let msg = format!("Ollama isn't running — start Ollama and try again. ({e})");
            let _ = app_handle.emit("quill://popup-error", &msg);
            let _ = app_handle.emit("quill://error", &msg);
            return;
        }

        // 4. Run the default action; the popup renders the result.
        let action = crate::actions::Action::FixGrammar;
        match client.chat(&settings.model, action.prompt(), &text).await {
            Ok(result) => {
                log::info!("correction ready ({} chars)", result.len());
                let _ = app_handle.emit("quill://popup-result", &result);
            }
            Err(e) => {
                log::error!("ollama chat failed: {e}");
                let msg = friendly_chat_error(&e);
                let _ = app_handle.emit("quill://popup-error", &msg);
                let _ = app_handle.emit("quill://error", &msg);
            }
        }
    });
}

/// Map typed Ollama errors to the §11 failure-matrix copy (shared with
/// commands::run_action).
pub fn friendly_chat_error(e: &crate::ollama::OllamaError) -> String {
    match e {
        crate::ollama::OllamaError::NotRunning { message } => {
            format!("Ollama isn't running. {message}")
        }
        crate::ollama::OllamaError::ModelNotFound { model } => {
            format!("That model isn't downloaded yet: {model}")
        }
        crate::ollama::OllamaError::Timeout { .. } => {
            "This is taking too long. Try again or pick a smaller model.".into()
        }
        crate::ollama::OllamaError::EmptyResponse { .. } => "No result — try again.".into(),
        other => other.to_string(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn register_hotkey_command(app: AppHandle, hotkey: String) -> Result<(), String> {
    let normalized = normalize_hotkey(&hotkey);
    // Persist canonical form
    let mut s = crate::settings::load_settings_snapshot(&app);
    s.hotkey = normalized.clone();
    crate::settings::save_settings_snapshot(&app, &s)?;
    register_hotkey(&app, &normalized)
}

#[tauri::command]
#[specta::specta]
pub fn get_hotkey(app: AppHandle) -> String {
    crate::settings::load_settings_snapshot(&app).hotkey
}

#[tauri::command]
#[specta::specta]
pub fn is_hotkey_registered(app: AppHandle, hotkey: String) -> bool {
    let normalized = normalize_hotkey(&hotkey);
    let target: Result<Shortcut, _> = normalized.as_str().try_into();
    match target {
        Ok(sc) => app.global_shortcut().is_registered(sc),
        Err(_) => false,
    }
}

// Keep unused import to avoid warnings about Code/Modifiers
#[allow(dead_code)]
fn _ensure_types() {
    let _ = Code::KeyA;
    let _ = Modifiers::CONTROL;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_dot_to_period() {
        assert_eq!(normalize_hotkey("CommandOrControl+Shift+."), "CommandOrControl+Shift+Period");
        assert_eq!(normalize_hotkey("CmdOrCtrl+Shift+."), "CommandOrControl+Shift+Period");
        assert_eq!(normalize_hotkey("CommandOrControl+Shift+Period"), "CommandOrControl+Shift+Period");
        assert_eq!(normalize_hotkey("Cmd+Shift+."), "Command+Shift+Period");
    }
    #[test]
    fn normalize_modifiers() {
        assert_eq!(normalize_hotkey("ctrl+shift+a"), "Control+Shift+A");
    }
}
