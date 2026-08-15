use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_plugin_store::StoreExt;

// ---------------------------------------------------------------------------
// Settings struct — persisted via tauri-plugin-store (JSON file)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl Default for Theme {
    fn default() -> Self {
        Self::System
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub model: String,
    pub hotkey: String,
    pub launch_at_login: bool,
    pub theme: Theme,
    #[serde(default)]
    pub onboarding_complete: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            model: "qwen3.5:4b".to_string(),
            hotkey: "CommandOrControl+Shift+Period".to_string(),
            launch_at_login: false,
            theme: Theme::System,
            onboarding_complete: false,
        }
    }
}

const STORE_PATH: &str = "settings.json";
const STORE_KEY: &str = "settings";

// ---------------------------------------------------------------------------
// Store IO helpers
// ---------------------------------------------------------------------------

fn load_from_store(app: &tauri::AppHandle) -> Settings {
    let Ok(store) = app.store(STORE_PATH) else {
        return Settings::default();
    };
    let Some(val) = store.get(STORE_KEY) else {
        return Settings::default();
    };
    let mut s: Settings = serde_json::from_value(val.clone()).unwrap_or_default();
    // Migration: canonicalize legacy hotkey spellings (CmdOrCtrl+Shift+. etc.) to Period
    s.hotkey = crate::shortcut::normalize_hotkey(&s.hotkey);
    s
}

fn save_to_store(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    let mut normalized = settings.clone();
    normalized.hotkey = crate::shortcut::normalize_hotkey(&normalized.hotkey);
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    let val = serde_json::to_value(&normalized).map_err(|e| e.to_string())?;
    store.set(STORE_KEY, val);
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Commands — exported via tauri-specta
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub fn get_settings(app: tauri::AppHandle) -> Settings {
    load_from_store(&app)
}

#[tauri::command]
#[specta::specta]
pub fn save_settings(app: tauri::AppHandle, settings: Settings) -> Result<(), String> {
    save_to_store(&app, &settings)?;
    Ok(())
}

// Snapshot helpers for non-command usage (shortcut.rs etc.)
pub fn load_settings_snapshot(app: &tauri::AppHandle) -> Settings {
    load_from_store(app)
}

pub fn save_settings_snapshot(app: &tauri::AppHandle, settings: &Settings) -> Result<(), String> {
    save_to_store(app, settings)
}
