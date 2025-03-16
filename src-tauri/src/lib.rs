mod app_handler;
mod chunker;
mod database_handler;
mod embedder;
mod file_processor;
mod qdrant_manager;
mod resource_monitor;
mod tokenizer;
mod utils;

use file_processor::FileProcessorState;
use std::io::{Error, ErrorKind};
use tauri::{AppHandle, Manager};
use tauri_plugin_shell::ShellExt;

type AppResult<T> = Result<T, Box<dyn std::error::Error>>;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            window.close_devtools();

            let db_path = init_database(app.app_handle().clone())?;
            let db_path_str = db_path.to_string_lossy().to_string();

            init_file_processor(db_path_str, 4, app.app_handle().clone())?;

            init_vector_db(app)?;

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn init_database(app_handle: AppHandle) -> AppResult<std::path::PathBuf> {
    match database_handler::initialize_database(app_handle) {
        Ok(path) => {
            println!("Database successfully initialized at {:?}", path);
            Ok(path)
        }
        Err(e) => {
            eprintln!("Failed to initialize database: {e}");
            Err(Box::new(Error::new(
                ErrorKind::Other,
                format!("Failed to initialize database: {}", e),
            )))
        }
    }
}

fn init_file_processor(
    db_path: String,
    concurrency: usize,
    app_handle: AppHandle,
) -> AppResult<()> {
    match file_processor::initialize_file_processor(db_path, concurrency, app_handle) {
        Ok(()) => {
            println!("File processor successfully initialized.");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to initialize file processor: {e}");
            Err(Box::new(Error::new(
                ErrorKind::Other,
                format!("Failed to initialize file processor: {}", e),
            )))
        }
    }
}

fn init_vector_db(app: &tauri::App) -> AppResult<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create runtime for qdrant");

    let app_handle = app.app_handle().clone();
    match runtime.block_on(qdrant_manager::initialize_qdrant(app_handle)) {
        Ok(manager) => {
            app.manage(qdrant_manager::QdrantState { manager });
            println!("Vector database successfully initialized");
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to initialize vector database: {e}");
            Err(Box::new(Error::new(
                ErrorKind::Other,
                format!("Failed to initialize vector database: {}", e),
            )))
        }
    }
}
