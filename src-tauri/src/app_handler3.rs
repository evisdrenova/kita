// // launches a selected app or switches to it if it's already running
// #[tauri::command]
// pub async fn launch_or_switch_to_app(
//     app: AppMetadata,
//     app_handle: tauri::AppHandle,
// ) -> Result<(), String> {
//     // try to switch if we have a PID
//     // if we have a PID then we know the app is running
//     if let Some(pid) = app.pid {
//         match unsafe { try_switch_to_pid(pid) } {
//             Ok(()) => {
//                 // Successfully switched, send an update with fresh resource data
//                 tokio::spawn(async move {
//                     // Wait a moment for the app to be fully active
//                     tokio::time::sleep(std::time::Duration::from_millis(200)).await;

//                     if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
//                         // Create updated app with fresh resource data
//                         let mut updated_app = app.clone();
//                         updated_app.resource_usage = Some(usage);

//                         // Emit to frontend
//                         let _ = app_handle.emit("app-activated", updated_app);
//                     }
//                 });

//                 return Ok(());
//             }
//             Err(_) => {
//                 // PID is outdated, fall back to launching via path
//                 println!("PID {} is outdated, attempting to launch via path", pid);
//             }
//         }
//     }

//     // If we get here, either:
//     // 1. App wasn't running (no PID)
//     // 2. PID was outdated and switch failed
//     // So try to launch it
//     Command::new("open")
//         .arg(&app.path)
//         .status()
//         .map_err(|e| format!("Failed to launch application: {}", e))?;

//     // For newly launched apps, we'll need to wait a bit and then check for the new process
//     tokio::spawn(async move {
//         // Wait for the app to start
//         tokio::time::sleep(std::time::Duration::from_secs(1)).await;

//         // Try to find the newly launched app in running apps
//         if let Ok(running_apps) = crate::app_handler::get_running_apps() {
//             if let Some(running_app) = running_apps.iter().find(|a| a.path == app.path) {
//                 if let Some(pid) = running_app.pid {
//                     if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
//                         // Create updated app with fresh resource data
//                         let mut updated_app = running_app.clone();
//                         updated_app.resource_usage = Some(usage);

//                         // Emit to frontend
//                         let _ = app_handle.emit("app-launched", updated_app);
//                     }
//                 }
//             }
//         }
//     });

//     Ok(())
// }

// #[tauri::command]
// pub async fn restart_application(
//     app: AppMetadata,
//     app_handle: tauri::AppHandle,
// ) -> Result<(), String> {
//     // try to force quit
//     if let Some(pid) = app.pid {
//         let _ = force_quit_application(pid).await;

//         // Wait a moment for the app to fully quit
//         tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
//     }

//     // launch the app
//     Command::new("open")
//         .arg(&app.path)
//         .status()
//         .map_err(|e| format!("Failed to launch application: {}", e))?;

//     // update the frontend after restarting
//     tokio::spawn(async move {
//         // Wait a bit for the app to start
//         tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

//         // Try to find the newly launched app
//         if let Ok(apps) = get_running_apps() {
//             if let Some(new_app) = apps.iter().find(|a| a.path == app.path) {
//                 if let Some(new_pid) = new_app.pid {
//                     if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(new_pid)
//                     {
//                         // Create updated app with resource data
//                         let mut updated_app = new_app.clone();
//                         updated_app.resource_usage = Some(usage);

//                         let _ = app_handle.emit("app-restarted", updated_app);
//                     }
//                 }
//             }
//         }
//     });

//     Ok(())
// }
