//! popup.rs — popup window lifecycle (PLAN.md §10 Phase 3)
//!
//! v1.1: the hotkey opens this popup instead of pasting headless. The popup
//! is focusable (refine chat needs keyboard input), so showing it steals
//! focus from the source app — we capture the frontmost app first and
//! restore it on Accept (see focus.rs).

use tauri::{AppHandle, Manager, WebviewWindowBuilder};

const POPUP_LABEL: &str = "popup";
const POPUP_URL: &str = "src/popup/index.html";
const POPUP_TITLE: &str = "Quill";
/// Logical size — keep in sync with tauri.conf.json popup window.
const POPUP_W: f64 = 400.0;
const POPUP_H: f64 = 480.0;
/// Offset from the mouse cursor, and margin kept inside the monitor.
const CURSOR_OFFSET: f64 = 14.0;

/// The app that owned the selection before the popup stole focus.
/// Captured in `show_popup`, consumed by the Accept flow.
static SOURCE_APP: std::sync::Mutex<Option<crate::focus::SourceApp>> =
    std::sync::Mutex::new(None);

/// Take the captured source app (None if capture failed or was already taken).
pub fn take_source_app() -> Option<crate::focus::SourceApp> {
    let mut guard = SOURCE_APP.lock().ok()?;
    guard.take()
}

/// Show the popup near the cursor and focus it. The window is declared
/// (visible:false) in tauri.conf.json; this creates it only as a fallback.
pub fn show_popup(app: &AppHandle) -> Result<(), String> {
    // BEFORE the popup takes focus, remember who had it.
    let source = crate::focus::capture_frontmost();
    if source.is_none() {
        log::warn!("could not capture frontmost app — Accept may need manual re-focus");
    }
    if let Ok(mut guard) = SOURCE_APP.lock() {
        *guard = source;
    }

    let win = match app.get_webview_window(POPUP_LABEL) {
        Some(win) => win,
        None => {
            #[allow(unused_mut)]
            let mut builder =
                WebviewWindowBuilder::new(app, POPUP_LABEL, tauri::WebviewUrl::App(POPUP_URL.into()))
                    .title(POPUP_TITLE)
                    .inner_size(POPUP_W, POPUP_H)
                    .decorations(false)
                    .resizable(false)
                    .visible(false)
                    .skip_taskbar(true)
                    .always_on_top(true);
            #[cfg(not(target_os = "macos"))]
            {
                builder = builder.transparent(true);
            }
            builder.build().map_err(|e| e.to_string())?
        }
    };

    position_near_cursor(app, &win);
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

/// Place the popup at cursor + offset, clamped into the monitor under the
/// cursor (falling back to the primary/current monitor). All coordinates are
/// physical pixels — cursor_position and Monitor bounds share that space.
fn position_near_cursor(app: &AppHandle, win: &tauri::WebviewWindow) {
    let Ok(cursor) = app.cursor_position() else {
        let _ = win.center();
        return;
    };
    let Ok(monitors) = win.available_monitors() else {
        let _ = win.center();
        return;
    };
    if monitors.is_empty() {
        let _ = win.center();
        return;
    }
    let mon = monitors
        .iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            cursor.x >= pos.x as f64
                && cursor.y >= pos.y as f64
                && cursor.x < (pos.x as f64 + size.width as f64)
                && cursor.y < (pos.y as f64 + size.height as f64)
        })
        .unwrap_or(&monitors[0]);

    let scale = mon.scale_factor().max(1.0);
    let mon_pos = mon.position();
    let mon_size = mon.size();
    let (pw, ph) = (POPUP_W * scale, POPUP_H * scale);

    let mut x = cursor.x + CURSOR_OFFSET * scale;
    let mut y = cursor.y + CURSOR_OFFSET * scale;
    // Clamp fully inside the monitor.
    x = x.clamp(mon_pos.x as f64, (mon_pos.x as f64 + mon_size.width as f64 - pw).max(mon_pos.x as f64));
    y = y.clamp(mon_pos.y as f64, (mon_pos.y as f64 + mon_size.height as f64 - ph).max(mon_pos.y as f64));

    if let Err(e) = win.set_position(tauri::PhysicalPosition::new(x, y)) {
        log::warn!("failed to position popup near cursor: {e}");
        let _ = win.center();
    }
}

pub fn hide_popup(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(POPUP_LABEL) {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn close_popup(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window(POPUP_LABEL) {
        win.close().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub fn show_popup_command(app: AppHandle) -> Result<(), String> {
    show_popup(&app)
}

#[tauri::command]
#[specta::specta]
pub fn hide_popup_command(app: AppHandle) -> Result<(), String> {
    hide_popup(&app)
}

#[tauri::command]
#[specta::specta]
pub fn close_popup_command(app: AppHandle) -> Result<(), String> {
    close_popup(&app)
}
