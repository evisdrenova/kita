mod app_handler;
mod database_handler;
mod file_processor;
mod resource_monitor;
mod tokenizer;
mod utils;

use file_processor::FileProcessorState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();

            window.open_devtools();
            window.close_devtools();

            let db_path = match database_handler::initialize_database(app.app_handle().clone()) {
                Ok(path) => {
                    println!("Database successfully initialized at {:?}", path);
                    path
                }
                Err(e) => {
                    eprintln!("Failed to initialize database: {e}");
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to initialize database: {}", e),
                    )));
                }
            };

            let db_path_str = db_path.to_string_lossy().to_string();

            match file_processor::initialize_file_processor(
                db_path_str,
                4,
                app.app_handle().clone(),
            ) {
                Ok(()) => {
                    println!("File processor successfully initialized.")
                }
                Err(e) => {
                    eprintln!("Failed to initialize file processor: {e}");
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to initialize file processor: {}", e),
                    )));
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
            file_processor::process_paths_command,
            file_processor::get_files_data,
            file_processor::open_file,
            file_processor::check_fts_table
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
