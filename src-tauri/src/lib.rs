mod app_handler;
mod file_processor;
mod resource_monitor;
mod database_handler;

use file_processor::FileProcessorState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            window.close_devtools();
            match database_handler::initialize_database(app.app_handle().clone()) {
                Ok(()) => {
                    println!("Database successfully initialized.")
                }
                Err(e) => {
                    eprintln!("Failed to initialize database: {e}")
                }
            }
            resource_monitor::init(app)?;
            Ok(())
        })
        .manage(FileProcessorState::default())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            app_handler::get_apps_data,
            app_handler::force_quit_application,
            app_handler::restart_application,
            app_handler::launch_or_switch_to_app,
            resource_monitor::start_resource_monitoring,
            resource_monitor::stop_resource_monitoring,
            file_processor::init_file_processor,
            file_processor::process_paths_tauri,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
