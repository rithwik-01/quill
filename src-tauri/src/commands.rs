//! commands.rs — Tauri command surface (PLAN.md §6, §11)

use tauri::{AppHandle, Emitter};

#[tauri::command]
#[specta::specta]
pub fn greet(name: String) -> String {
    format!("Hello, {name}! — from Rust via specta")
}

#[tauri::command]
#[specta::specta]
pub fn ping() -> String {
    "pong".to_string()
}

// ---------------------------------------------------------------------------
// Ollama liveness / model helpers
// ---------------------------------------------------------------------------

// Ollama liveness is "ensure" semantics, not a passive probe: end users
// shouldn't know what Ollama is, so a stopped server is started on demand.
#[tauri::command]
#[specta::specta]
pub async fn check_ollama() -> Result<crate::ollama::VersionResponse, String> {
    let client = crate::ollama::OllamaClient::default_local();
    client.ensure_running().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_models() -> Result<crate::ollama::TagsResponse, String> {
    let client = crate::ollama::OllamaClient::default_local();
    client.ensure_running().await.map_err(|e| e.to_string())?;
    client.tags().await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn check_model_installed(model: String) -> Result<bool, String> {
    let client = crate::ollama::OllamaClient::default_local();
    client.ensure_running().await.map_err(|e| e.to_string())?;
    let tags = client.tags().await.map_err(|e| e.to_string())?;
    Ok(tags.models.iter().any(|m| m.name == model))
}

// Pull emits events via channel; simple wrapper that pulls and returns.
#[tauri::command]
#[specta::specta]
pub async fn pull_model(app: AppHandle, model: String) -> Result<(), String> {
    let client = crate::ollama::OllamaClient::default_local();
    client.ensure_running().await.map_err(|e| e.to_string())?;
    client
        .pull(&model, |prog| {
            let _ = app.emit("quill://pull-progress", &prog);
        })
        .await
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Action runner (PLAN.md §9)
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn run_action(
    app: AppHandle,
    action: String,
    text: String,
    model: String,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("Select some text first.".into());
    }
    let parsed = parse_action(&action)?;
    let client = crate::ollama::OllamaClient::default_local();
    let chosen_model = if model.is_empty() {
        crate::hardware::recommended_model()
    } else {
        model
    };
    client
        .ensure_running()
        .await
        .map_err(|e| crate::shortcut::friendly_chat_error(&e))?;
    let result = client
        .chat(&chosen_model, parsed.prompt(), &text)
        .await
        .map_err(|e| match e {
            crate::ollama::OllamaError::NotRunning { message } => {
                format!("Couldn't start the local AI engine. {message}")
            }
            crate::ollama::OllamaError::ModelNotFound { model } => {
                format!("That model isn't downloaded yet: {model} — Download with progress")
            }
            crate::ollama::OllamaError::Timeout { message } => {
                format!("This is taking too long. {message} — Cancel")
            }
            crate::ollama::OllamaError::EmptyResponse { .. } => {
                "No result — try again.".into()
            }
            other => other.to_string(),
        })?;
    if result.trim().is_empty() {
        return Err("No result — try again.".into());
    }
    Ok(result)
}

fn parse_action(s: &str) -> Result<crate::actions::Action, String> {
    match s {
        "fix_grammar" | "FixGrammar" | "fixGrammar" => Ok(crate::actions::Action::FixGrammar),
        "improve" | "Improve" => Ok(crate::actions::Action::Improve),
        "shorten" | "Shorten" => Ok(crate::actions::Action::Shorten),
        "simplify" | "Simplify" => Ok(crate::actions::Action::Simplify),
        _ => Err(format!("unknown action: {s}")),
    }
}

// ---------------------------------------------------------------------------
// Selection & replace
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn get_selected_text(app: AppHandle) -> Result<String, String> {
    crate::selection::get_selected_text(&app)
}

#[tauri::command]
#[specta::specta]
pub fn replace_text(app: AppHandle, text: String) -> Result<(), String> {
    crate::replace::replace_selected_text(&app, &text)
}

#[tauri::command]
#[specta::specta]
pub fn capture_and_run(
    app: AppHandle,
    action: String,
    model: String,
) -> Result<String, String> {
    let selected = crate::selection::get_selected_text(&app)?;
    if selected.trim().is_empty() {
        return Err("Select some text first.".into());
    }
    // Synchronous wrapper for simple hotkey path; for async chat use run_action.
    // Here we block on the async chat via tauri::async_runtime::block_on.
    let parsed = parse_action(&action)?;
    let chosen_model = if model.is_empty() {
        crate::hardware::recommended_model()
    } else {
        model
    };
    let client = crate::ollama::OllamaClient::default_local();
    let result = tauri::async_runtime::block_on(client.chat(&chosen_model, parsed.prompt(), &selected))
        .map_err(|e| e.to_string())?;
    crate::replace::replace_selected_text(&app, &result)?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// Accessibility (macOS) — all logic lives in crate::accessibility, which
// documents why grants go stale (code-signing identity churn) and why that is
// fixed in the build config, not here.
#[tauri::command]
#[specta::specta]
pub fn is_accessibility_trusted() -> bool {
    crate::accessibility::is_trusted()
}

#[tauri::command]
#[specta::specta]
pub fn request_accessibility_permission() -> bool {
    // Shows the system prompt if this app identity has never been asked.
    // Returns the CURRENT trust only (Apple: the prompt is async) — the
    // frontend polls `is_accessibility_trusted` until it flips.
    crate::accessibility::request_with_prompt()
}

#[tauri::command]
#[specta::specta]
pub fn open_accessibility_settings() -> Result<(), String> {
    // Apple-recommended fallback deep link when the prompt was already
    // shown or dismissed.
    crate::accessibility::open_settings_pane()
}

/// Diagnostic snapshot: TCC flag, functional-probe result, and the exact
/// executable that needs the grant. The frontend shows this when the
/// permission looks stuck (stale-entry scenario).
#[tauri::command]
#[specta::specta]
pub fn get_accessibility_status(
    app: AppHandle,
) -> Result<crate::accessibility::AccessibilityStatus, String> {
    Ok(crate::accessibility::status(&app))
}

#[tauri::command]
#[specta::specta]
pub fn validate_hotkey(hotkey: String) -> Result<(), String> {
    let normalized = crate::shortcut::normalize_hotkey(&hotkey);
    let _: tauri_plugin_global_shortcut::Shortcut = normalized
        .as_str()
        .try_into()
        .map_err(|e| format!("invalid hotkey '{hotkey}' (normalized '{normalized}'): {e:?}"))?;
    Ok(())
}

/// Accept from the popup: hide → restore source-app focus → paste → save to
/// history. Order matters — the popup holds keyboard focus, so it must be
/// hidden and the source app re-activated BEFORE the synthetic Cmd/Ctrl+V,
/// otherwise the paste lands in Quill.
#[tauri::command]
#[specta::specta]
pub fn accept_result(
    app: AppHandle,
    history: tauri::State<'_, crate::history::HistoryManager>,
    text: String,
    original: String,
    action: String,
    model: String,
    refinements: Vec<String>,
) -> Result<(), String> {
    let _ = crate::popup::hide_popup(&app);
    if let Some(source) = crate::popup::take_source_app() {
        crate::focus::activate(source);
        // Let the activation settle before injecting the paste chord.
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
    let paste = crate::replace::replace_selected_text(&app, &text);
    // History of accepted changes (SQLite, Handy-style). The paste result is
    // undetectable (§11), so we record the accept either way — the result is
    // on the clipboard for manual recovery.
    if let Err(e) = history.save_entry(&action, &model, &original, &text, &refinements) {
        log::error!("failed to save history entry: {e}");
    }
    match paste {
        Ok(()) => {
            let _ = app.emit("quill://pasted", ());
            Ok(())
        }
        Err(e) => {
            let msg = format!("{e} — your rewritten text is on the clipboard, press Cmd+V to paste manually");
            let _ = app.emit("quill://paste-failed", msg.clone());
            Err(msg)
        }
    }
}

/// Popup refine chat: apply one instruction to the current version.
/// Stateless single round-trip (original + current + instruction).
#[tauri::command]
#[specta::specta]
pub async fn refine_result(
    original: String,
    current: String,
    instruction: String,
    model: String,
) -> Result<String, String> {
    if instruction.trim().is_empty() {
        return Err("Type an instruction first.".into());
    }
    let chosen_model = if model.is_empty() {
        crate::hardware::recommended_model()
    } else {
        model
    };
    let client = crate::ollama::OllamaClient::default_local();
    let user = format!(
        "Original text:\n{original}\n\nCurrent version:\n{current}\n\nInstruction: {instruction}"
    );
    let messages = vec![
        crate::ollama::ChatMessage {
            role: "system".into(),
            content: crate::actions::REFINE_PROMPT.into(),
        },
        crate::ollama::ChatMessage {
            role: "user".into(),
            content: user,
        },
    ];
    client
        .chat_messages(&chosen_model, messages)
        .await
        .map_err(|e| crate::shortcut::friendly_chat_error(&e))
}

#[tauri::command]
#[specta::specta]
pub fn close_popup(app: AppHandle) -> Result<(), String> {
    crate::popup::close_popup(&app)
}

#[tauri::command]
#[specta::specta]
pub fn get_popup_text(app: AppHandle) -> Result<String, String> {
    crate::selection::get_selected_text(&app)
}

#[tauri::command]
#[specta::specta]
pub fn open_url(app: AppHandle, url: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(url, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())?;
    } else {
        manager.disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}
