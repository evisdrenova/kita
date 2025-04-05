mod app_handler;
mod chunker;
mod database_handler;
mod embedder;
mod file_processor;
mod model_registry;
mod resource_monitor;
mod server;
mod settings;
mod tokenizer;
mod utils;
mod vectordb_manager;
mod window;

use file_processor::FileProcessorState;
use tauri::Manager;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let db_path = database_handler::init_database(app.app_handle().clone())?;
            let db_path_str = db_path.to_string_lossy().to_string();

            // settings::init_settings(&db_path_str, app.app_handle().clone())?;
            // file_processor::init_file_processor(&db_path_str, 4, app.app_handle().clone())?;
            // vectordb_manager::init_vector_db(app)?;
            // server::init_server(app)?;
            // resource_monitor::init_resource_monitor(app)?;
            // server::register_llm_commands(app)?;

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
            file_processor::get_semantic_files_data,
            file_processor::open_file,
            model_registry::get_models,
            model_registry::get_downloaded_models,
            model_registry::start_model_download,
            model_registry::check_model_exists,
            server::ask_llm,
            settings::get_settings,
            settings::update_settings,
            window::show_main_window
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
