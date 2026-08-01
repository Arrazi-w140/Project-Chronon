mod font_library;
mod widget_window;   // 
mod updater;   // GitHub-release auto-update (background check + Settings > Updates commands)
#[cfg(target_os = "windows")]
mod desktop_layer;   // WorkerW reparenting so the widget sits behind desktop icons (Windows only)

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(widget_window::WidgetState::default())   //
        .manage(updater::PendingUpdate::default())
        .manage(updater::AutoCheckEnabled::default())
        .setup(|app| {
            font_library::initialize(&app.handle())
                .map_err(std::io::Error::other)?;

            // Registered here rather than chained on the Builder above
            // because it's desktop-only — Tauri's mobile targets don't
            // support the updater plugin.
            #[cfg(desktop)]
            {
                app.handle().plugin(tauri_plugin_updater::Builder::new().build())?;
                updater::spawn_background_checks(app.handle().clone());
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            font_library::import_fonts,
            font_library::list_imported_fonts,
            font_library::delete_imported_font,
            widget_window::push_widget,           // 
            widget_window::update_widget_config,   // 
            widget_window::get_widget_config,      //
            widget_window::delete_widget,          // 
            widget_window::is_widget_active,       // 
            widget_window::set_widget_position,    // 
            widget_window::set_widget_size,
            updater::check_for_updates,
            updater::install_update_now,
            updater::get_app_version,
            updater::set_auto_check_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
