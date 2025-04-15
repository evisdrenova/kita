use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use tauri::Emitter;

use crate::resource_monitor::AppResourceUsage;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppMetadata {
    pub name: String,
    pub path: String,
    pub pid: Option<u32>,
    pub icon: Option<String>,
    pub resource_usage: Option<AppResourceUsage>,
}

extern "C" {
    fn get_combined_apps_swift() -> *mut c_char;
    fn get_running_apps_swift() -> *mut c_char;
    fn get_app_icon_swift(path: *const c_char) -> *mut c_char;
    fn switch_to_app_swift(pid: i32) -> bool;
    fn force_quit_app_swift(pid: i32) -> bool;
    fn restart_app_swift(path: *const c_char) -> bool;
    fn check_process_running_swift(pid: i32) -> bool;
    fn free_string_swift(pointer: *mut c_char);
}

#[derive(Deserialize)]
struct AppsResponse {
    running_apps: Vec<AppMetadata>,
    installed_apps: Vec<AppMetadata>,
}

pub fn get_running_apps() -> Result<Vec<AppMetadata>, String> {
    let apps_json_ptr = unsafe { get_running_apps_swift() };
    if apps_json_ptr.is_null() {
        return Err("Failed to get apps".to_string());
    }
    let apps_json = unsafe {
        let c_str = CStr::from_ptr(apps_json_ptr);
        let result = c_str
            .to_str()
            .map_err(|_| "Invalid UTF-8".to_string())?
            .to_owned();
        free_string_swift(apps_json_ptr);
        result
    };

    let apps_response = serde_json::from_str(&apps_json).map_err(|e| e.to_string())?;

    Ok(apps_response)
}

pub fn get_app_icon(app_path: &str) -> Result<Option<String>, String> {
    let path_cstring =
        CString::new(app_path).map_err(|_| "Failed to create C string".to_string())?;

    let icon_ptr = unsafe { get_app_icon_swift(path_cstring.as_ptr()) };

    if icon_ptr.is_null() {
        return Ok(None);
    }

    let icon = unsafe {
        let c_str = CStr::from_ptr(icon_ptr);
        let result = c_str
            .to_str()
            .map_err(|_| "Invalid UTF-8".to_string())?
            .to_owned();
        free_string_swift(icon_ptr);
        result
    };

    Ok(Some(icon))
}

fn filter_apps(app: Vec<AppMetadata>) -> Vec<AppMetadata> {
    let filtered_apps: Vec<AppMetadata> = app
        .into_iter()
        .filter(|app| {
            let name = &app.name;
            let path = &app.path;

            !(name.contains("Helper")
                || name.contains("Agent")
                || name.ends_with("Assistant")
                || name.starts_with("com.")
                || name.starts_with("plugin_")
                || name.starts_with(".")
                || path.contains(".framework")
                || path.contains("Contents/Frameworks/")
                || path.contains("Contents/XPCServices/")
                || path.contains("Contents/PlugIns/")
                || path.contains("Contents/Helpers/")
                || path.contains("/usr/libexec")
                || path.contains("System/Library/CoreServices/")
                || name.contains("Crash Reporter")
                || name.contains("Updater")
                || name.contains("Diagnostics"))
        })
        .collect();

    filtered_apps
}

#[tauri::command]
pub fn get_apps_data() -> Result<Vec<AppMetadata>, String> {
    let apps_json_ptr = unsafe { get_combined_apps_swift() };

    if apps_json_ptr.is_null() {
        return Err("Failed to get apps".to_string());
    }

    let apps_json = unsafe {
        let c_str = CStr::from_ptr(apps_json_ptr);
        let result = c_str
            .to_str()
            .map_err(|_| "Invalid UTF-8".to_string())?
            .to_owned();
        free_string_swift(apps_json_ptr);
        result
    };

    let apps_response: AppsResponse =
        serde_json::from_str(&apps_json).map_err(|e| e.to_string())?;

    let mut combined_apps = apps_response.running_apps;

    let unique_installed_apps: Vec<AppMetadata> = apps_response
        .installed_apps
        .into_iter()
        .filter(|installed| {
            !combined_apps
                .iter()
                .any(|running| running.name == installed.name)
        })
        .collect();

    combined_apps.extend(unique_installed_apps);

    combined_apps.par_iter_mut().for_each(|app| {
        if let Ok(icon) = get_app_icon(&app.path) {
            app.icon = icon;
        }
    });

    Ok(filter_apps(combined_apps))
}

#[tauri::command]
pub async fn launch_or_switch_to_app(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    if let Some(pid) = app.pid {
        let int3pid = pid as i32;
        let switched = unsafe { switch_to_app_swift(int3pid) };

        if switched {
            tokio::spawn(async move {
                // wait for app to be active
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
                    let mut updated_app = app.clone();
                    updated_app.resource_usage = Some(usage);
                    let _ = app_handle.emit("app-activated", updated_app);
                }
            });

            return Ok(());
        }
    }

    // If switching fails or no PID, launch the app
    let path_cstring =
        CString::new(app.path.clone()).map_err(|_| "Failed to create C string".to_string())?;

    let restarted = unsafe { restart_app_swift(path_cstring.as_ptr()) };

    if !restarted {
        return Err(format!("Failed to launch application: {}", app.path));
    }

    // For newly launched apps, monitor and update resource usage
    let app_path = app.path.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Try to find the newly launched app in running apps
        if let Ok(running_apps) = get_running_apps() {
            if let Some(running_app) = running_apps.iter().find(|a| a.path == app_path) {
                if let Some(pid) = running_app.pid {
                    if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
                        let mut updated_app = running_app.clone();
                        updated_app.resource_usage = Some(usage);
                        let _ = app_handle.emit("app-launched", updated_app);
                    }
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn force_quit_application(pid: u32) -> Result<(), String> {
    // Initial attempt to force quit
    let result = unsafe { force_quit_app_swift(pid as i32) };

    if !result {
        return Err(format!(
            "Failed to initiate termination for application with PID {}",
            pid
        ));
    }

    // Now poll to see if the application has actually terminated
    // Define a timeout (5 seconds)
    // the swift .terminate() method sends a kill signal to the app, but the app might be saving data and take time to close, so we poll it to see if the process is still running, since we can't call async functions from rust to swift
    let timeout = std::time::Duration::from_secs(5);
    let start_time = std::time::Instant::now();

    // Poll at regular intervals
    let poll_interval = std::time::Duration::from_millis(200);

    while start_time.elapsed() < timeout {
        // Check if the process is still running
        if !is_process_running(pid) {
            // Process has terminated successfully
            return Ok(());
        }

        // Wait before checking again
        tokio::time::sleep(poll_interval).await;
    }

    // If we get here, we timed out waiting for the process to terminate
    Err(format!(
        "Timed out waiting for application with PID {} to terminate",
        pid
    ))
}

fn is_process_running(pid: u32) -> bool {
    unsafe { check_process_running_swift(pid as i32) }
}

#[tauri::command]
pub async fn restart_application(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // First, attempt to force quit if we have a PID
    if let Some(pid) = app.pid {
        let _ = force_quit_application(pid).await;

        // Wait a moment for the app to fully quit
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Prepare path as C string
    let path_cstring =
        CString::new(app.path.clone()).map_err(|_| "Failed to create C string".to_string())?;

    // Restart the application
    let restarted = unsafe { restart_app_swift(path_cstring.as_ptr()) };

    if !restarted {
        return Err(format!("Failed to restart application: {}", app.path));
    }

    // Update the frontend with resource usage information after restart
    let app_path = app.path.clone();
    tokio::spawn(async move {
        // Wait a bit for the app to start
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Try to find the newly launched app
        if let Ok(apps) = crate::app_handler::get_running_apps() {
            if let Some(new_app) = apps.iter().find(|a| a.path == app_path) {
                if let Some(new_pid) = new_app.pid {
                    if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(new_pid)
                    {
                        // Create updated app with resource data
                        let mut updated_app = new_app.clone();
                        updated_app.resource_usage = Some(usage);

                        let _ = app_handle.emit("app-restarted", updated_app);
                    }
                }
            }
        }
    });

    Ok(())
}
