// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod app_handler;
use app_handler::get_all_apps;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust and Typescript and React!", name)
}

#[tauri::command]
fn get_all_applications() -> Result<Vec<app_handler::AppMetadata>, String> {
    get_all_apps()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .invoke_handler(tauri::generate_handler![get_all_applications])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
