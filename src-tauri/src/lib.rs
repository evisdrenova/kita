mod app_handler;
mod resource_monitor;

use app_handler::{get_all_apps, launch_or_switch_to_app, AppMetadata};
use resource_monitor::get_all_apps_with_usage;
use serde::{Serialize, Deserialize};
use tauri::Manager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SectionType {
    Apps,
    Files,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SearchItem {
    App(AppMetadata),
    // File(FileMetadata),
    // Semantic(SemanticResult), 
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchSection {
    pub type_: SectionType,
    pub title: String,
    pub items: Vec<SearchItem>,
} 

#[tauri::command]
async fn launch_or_switch_to_application(app: AppMetadata, app_handle: tauri::AppHandle) -> Result<(), String> {
    // Pass the app_handle to allow for resource updates after launch
    launch_or_switch_to_app(app, app_handle).await
}

#[tauri::command]
fn get_apps_data() -> Result<Vec<SearchSection>, String> {
    let mut sections = Vec::new();

    // Try to get apps with resource usage first
    let apps = match get_all_apps_with_usage() {
        Ok(apps) => apps,
        Err(_) => get_all_apps()?, // Fall back to regular function if resource monitoring fails
    };

    if !apps.is_empty() {
        let app_items: Vec<SearchItem> = apps.into_iter().map(|app| SearchItem::App(app)).collect();

        sections.push(SearchSection {
            type_: SectionType::Apps,
            title: "Applications".to_string(),
            items: app_items,
        })
    }

    Ok(sections)
}

#[tauri::command]
fn get_apps_with_resources() -> Result<Vec<AppMetadata>, String> {
    resource_monitor::get_all_apps_with_usage()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
            window.close_devtools();
            resource_monitor::init(app)?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            get_apps_data,
            launch_or_switch_to_application,
            get_apps_with_resources,
            resource_monitor::start_resource_monitoring,
            resource_monitor::stop_resource_monitoring,
            resource_monitor::get_resource_data,
            resource_monitor::monitor_app_resources,
            resource_monitor::get_apps_with_live_resources
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}