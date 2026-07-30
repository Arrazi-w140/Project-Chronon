mod widget_window;   // 
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
        .manage(widget_window::WidgetState::default())   //
        .invoke_handler(tauri::generate_handler![
            greet,
            widget_window::push_widget,           // 
            widget_window::update_widget_config,   // 
            widget_window::get_widget_config,      //
            widget_window::delete_widget,          // 
            widget_window::is_widget_active,       // 
            widget_window::set_widget_position,    // 
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}