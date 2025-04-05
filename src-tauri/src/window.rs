use tauri::Manager;

#[tauri::command]
pub async fn show_main_window(window: tauri::Window) {
    let window = window.get_webview_window("main").unwrap();

    window.show().unwrap();
    window.open_devtools();
    window.close_devtools();
}
