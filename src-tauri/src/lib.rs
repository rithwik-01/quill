mod accessibility;
mod actions;
mod commands;
mod focus;
mod hardware;
mod history;
mod ollama;
mod popup;
mod replace;
mod selection;
mod settings;
mod shortcut;

use specta_typescript::{BigIntExportBehavior, Typescript};
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;
use tauri_specta::{collect_commands, Builder};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let specta_builder = Builder::<tauri::Wry>::new().commands(collect_commands![
        commands::greet,
        commands::ping,
        commands::check_ollama,
        commands::list_models,
        commands::check_model_installed,
        commands::pull_model,
        commands::run_action,
        commands::get_selected_text,
        commands::replace_text,
        commands::capture_and_run,
        commands::is_accessibility_trusted,
        commands::request_accessibility_permission,
        commands::open_accessibility_settings,
        commands::get_accessibility_status,
        commands::validate_hotkey,
        commands::accept_result,
        commands::refine_result,
        commands::close_popup,
        commands::get_popup_text,
        commands::open_url,
        commands::set_autostart,
        settings::get_settings,
        settings::save_settings,
        hardware::get_recommended_model,
        hardware::get_system_ram_gb,
        popup::show_popup_command,
        popup::hide_popup_command,
        popup::close_popup_command,
        shortcut::register_hotkey_command,
        shortcut::get_hotkey,
        shortcut::is_hotkey_registered,
        history::get_history_entries,
        history::delete_history_entry,
        history::clear_history,
    ]);

    #[cfg(debug_assertions)]
    {
        let bindings_path = "../src/bindings.ts";
        // BigIntExportBehavior::Number — history ids/timestamps are i64 but well
        // within Number range; matches Handy's lib.rs:728 configuration.
        let ts_config = Typescript::default().bigint(BigIntExportBehavior::Number);
        if let Err(e) = specta_builder.export(ts_config, bindings_path) {
            eprintln!("warning: failed to export specta bindings: {e}");
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .clear_targets()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Webview,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: Some("quill".into()) },
                ))
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(specta_builder.invoke_handler())
        .setup(move |app| {
            specta_builder.mount_events(app);
            // SQLite history (Handy-style manager, managed state for commands)
            let history_manager = crate::history::HistoryManager::new(app.handle())
                .map_err(|e| format!("failed to init history: {e}"))?;
            app.manage(history_manager);
            let store_handle = app.handle().clone();
            // Ensure settings.json exists with defaults (so first launch hydrates correctly)
            {
                let settings = crate::settings::load_settings_snapshot(&store_handle);
                // Persist defaults once; also ensures hotkey is stored canonically
                let _ = crate::settings::save_settings_snapshot(&store_handle, &settings);
            }
            // Register hotkey from persisted settings (falls back to default).
            // Apple docs: global shortcut registration can fail if another app holds the chord;
            // we surface it via log + emit for the frontend toast.
            let handle = app.handle().clone();
            let stored_hotkey = crate::settings::load_settings_snapshot(&handle).hotkey;
            let hotkey_to_register = if stored_hotkey.trim().is_empty() {
                shortcut::DEFAULT_HOTKEY.to_string()
            } else {
                stored_hotkey
            };
            if let Err(e) = shortcut::register_hotkey(&handle, &hotkey_to_register) {
                log::warn!("failed to register hotkey '{}': {e}", hotkey_to_register);
                // Also emit so the Settings UI can show a conflict warning on next hydrate
                let _ = handle.emit("quill://hotkey-error", e.to_string());
            } else {
                log::info!("registered hotkey '{}'", hotkey_to_register);
            }
            // Setup tray icon (Phase 4) — simple default that shows the main window on click
            #[cfg(desktop)]
            {
                use tauri::menu::{Menu, MenuItem};
                use tauri::tray::TrayIconBuilder;
                let quit = MenuItem::with_id(app, "quit", "Quit Quill", true, None::<&str>)
                    .unwrap();
                let show = MenuItem::with_id(app, "show", "Show Quill", true, None::<&str>)
                    .unwrap();
                let menu = Menu::with_items(app, &[&show, &quit]).unwrap();
                let _ = TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "show" => {
                            if let Some(win) = app.get_webview_window("main") {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                        _ => {}
                    })
                    .build(app);
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
