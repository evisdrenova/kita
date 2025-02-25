// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

mod app_handler;
use app_handler::{get_all_apps, launch_or_switch_to_app, AppMetadata};
use serde::{ Serialize, Deserialize};
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
async fn launch_or_switch_to_application(app: app_handler::AppMetadata) -> Result<(), String> {
    launch_or_switch_to_app(app).await
}



#[tauri::command]
fn get_search_data() -> Result<Vec<SearchSection>, String> {
    let mut sections = Vec::new();

    let apps = get_all_apps()?;


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
        .invoke_handler(tauri::generate_handler![get_search_data,
            launch_or_switch_to_application])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
