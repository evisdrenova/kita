// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod app_handler;
use app_handler::{get_all_apps, launch_or_switch_to_app};
use tauri::Manager;

#[tauri::command]
fn get_all_applications() -> Result<Vec<app_handler::AppMetadata>, String> {
    get_all_apps()
}

#[tauri::command]
async fn launch_or_switch_to_application(app: app_handler::AppMetadata) -> Result<(), String> {
    launch_or_switch_to_app(app).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default().setup(|app| {
        {
          let window = app.get_webview_window("main").unwrap();
          window.open_devtools();
          window.close_devtools();
        }
        Ok(())
      })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![    get_all_applications,
            launch_or_switch_to_application])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
