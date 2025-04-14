use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

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
    fn get_app_icon_swift(path: *const c_char) -> *mut c_char;
    fn switch_to_app_swift(pid: i32) -> bool;
    fn force_quit_app_swift(pid: i32) -> bool;
    fn restart_app_swift(path: *const c_char) -> bool;
    fn free_string_swift(pointer: *mut c_char);
}

#[derive(Deserialize)]
struct AppsResponse {
    running_apps: Vec<AppMetadata>,
    installed_apps: Vec<AppMetadata>,
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

#[tauri::command]
pub async fn launch_or_switch_to_app(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Try to switch if we have a PID
    if let Some(pid) = app.pid {
        let int3pid = pid as i32;
        let switched = unsafe { switch_to_app_swift(int3pid) };

        if switched {
            // Optionally, you can add resource usage update logic here
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

    Ok(())
}

#[tauri::command]
pub async fn force_quit_application(pid: u32) -> Result<(), String> {
    let result = unsafe { force_quit_app_swift(pid as i32) };

    if result {
        Ok(())
    } else {
        Err(format!("Failed to force quit application with PID {}", pid))
    }
}

#[tauri::command]
pub async fn restart_application(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // First, attempt to force quit if we have a PID
    if let Some(pid) = app.pid {
        let _ = force_quit_application(pid).await;
    }

    // Prepare path as C string
    let path_cstring =
        CString::new(app.path.clone()).map_err(|_| "Failed to create C string".to_string())?;

    // Restart the application
    let restarted = unsafe { restart_app_swift(path_cstring.as_ptr()) };

    if !restarted {
        return Err(format!("Failed to restart application: {}", app.path));
    }

    Ok(())
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
