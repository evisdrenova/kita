// the objc crate has some cgs errors that it throws
// these shouldn't impact functionality so silencing for now until they fix them in a new version or something
#![allow(unexpected_cfgs)]

use base64::prelude::*;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSSize};
use lazy_static::lazy_static;
use libproc::libproc::proc_pid;
use libproc::processes;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
use objc2_app_kit::NSPNGFileType;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use tauri::Emitter;

use crate::resource_monitor::AppResourceUsage;

// used to cache the app icons so that we don't have to load them every time
lazy_static! {
    static ref ICON_CACHE: Mutex<HashMap<String, Option<String>>> = Mutex::new(HashMap::new());
}

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppMetadata {
    name: String,
    path: String,
    pub pid: Option<u32>,
    icon: Option<String>, // Base64 encoded icon data
    pub resource_usage: Option<AppResourceUsage>,
}

const APPLICATIONS_DIR: &str = "/Applications";
const SYSTEM_APPLICATIONS_DIR: &str = "/System/Applications";

// gets all of the apps installed  in the app directory
pub fn get_installed_apps() -> Result<Vec<AppMetadata>, String> {
    let mut app_directories = vec![
        PathBuf::from(APPLICATIONS_DIR),
        PathBuf::from(SYSTEM_APPLICATIONS_DIR),
    ];

    if let Ok(home) = env::var("HOME") {
        app_directories.push(PathBuf::from(home).join("Applications"))
    }

    let mut installed_apps = Vec::new();

    for dir in app_directories {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                //check if it's an.app directory
                if path.is_dir() && path.extension().and_then(|ext| ext.to_str()) == Some("app") {
                    if let Some(app_name) = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.replace(".app", ""))
                    {
                        // Skip helper apps
                        if !app_name.contains("Helper")
                            && !app_name.contains("Agent")
                            && !app_name.ends_with("Assistant")
                            && !app_name.starts_with("com.")
                            && !app_name.starts_with("plugin_")
                            && !app_name.starts_with(".")
                        {
                            installed_apps.push(AppMetadata {
                                name: app_name,
                                path: path.to_string_lossy().into_owned(),
                                pid: None,
                                icon: None,
                                resource_usage: None,
                            });
                        }
                    }
                }
            }
        }
    }

    installed_apps.sort_by(|a, b| a.name.cmp(&b.name));
    installed_apps.dedup_by(|a, b| a.name == b.name);

    Ok(installed_apps)
}

// gets list of running apps and returns a vector of RunningApp or an error string
pub fn get_running_apps() -> Result<Vec<AppMetadata>, String> {
    let pids: Vec<u32> = processes::pids_by_type(processes::ProcFilter::All)
        .map_err(|e| format!("Failed to list PIDs: {}", e))?;

    let mut desktop_apps: Vec<AppMetadata> = Vec::new();

    for pid in pids {
        if pid == 0 {
            continue;
        }

        if let Ok(path) = proc_pid::pidpath(pid.try_into().unwrap()) {
            if path.contains(".app") {
                // Extract the .app bundle path
                if let Some(app_bundle_path) = path.split(".app").next() {
                    let bundle_path = format!("{}.app", app_bundle_path);

                    if bundle_path.starts_with("/Applications")
                        || bundle_path.starts_with("/System/Applications")
                        || (bundle_path.contains("/Users/")
                            && bundle_path.contains("/Applications/"))
                    {
                        if let Some(app_name) = Path::new(&bundle_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.replace(".app", ""))
                        {
                            if !app_name.contains("Helper")
                                && !app_name.contains("Agent")
                                && !app_name.ends_with("Assistant")
                                && !app_name.starts_with("com.")
                                && !app_name.starts_with("plugin_")
                                && !app_name.starts_with(".")
                            {
                                desktop_apps.push(AppMetadata {
                                    name: app_name,
                                    path: bundle_path,
                                    pid: Some(pid),
                                    icon: None,
                                    resource_usage: None,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    desktop_apps.sort_by(|a, b| a.name.cmp(&b.name));
    desktop_apps.dedup_by(|a, b| a.name == b.name);

    Ok(desktop_apps)
}

// launches a selected app or switches to it if it's already running
#[tauri::command]
pub async fn launch_or_switch_to_app(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // try to switch if we have a PID
    // if we have a PID then we know the app is running
    if let Some(pid) = app.pid {
        match unsafe { try_switch_to_pid(pid) } {
            Ok(()) => {
                // Successfully switched, send an update with fresh resource data
                tokio::spawn(async move {
                    // Wait a moment for the app to be fully active
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                    if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
                        // Create updated app with fresh resource data
                        let mut updated_app = app.clone();
                        updated_app.resource_usage = Some(usage);

                        // Emit to frontend
                        let _ = app_handle.emit("app-activated", updated_app);
                    }
                });

                return Ok(());
            }
            Err(_) => {
                // PID is outdated, fall back to launching via path
                println!("PID {} is outdated, attempting to launch via path", pid);
            }
        }
    }

    // If we get here, either:
    // 1. App wasn't running (no PID)
    // 2. PID was outdated and switch failed
    // So try to launch it
    Command::new("open")
        .arg(&app.path)
        .status()
        .map_err(|e| format!("Failed to launch application: {}", e))?;

    // For newly launched apps, we'll need to wait a bit and then check for the new process
    tokio::spawn(async move {
        // Wait for the app to start
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Try to find the newly launched app in running apps
        if let Ok(running_apps) = crate::app_handler::get_running_apps() {
            if let Some(running_app) = running_apps.iter().find(|a| a.path == app.path) {
                if let Some(pid) = running_app.pid {
                    if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(pid) {
                        // Create updated app with fresh resource data
                        let mut updated_app = running_app.clone();
                        updated_app.resource_usage = Some(usage);

                        // Emit to frontend
                        let _ = app_handle.emit("app-launched", updated_app);
                    }
                }
            }
        }
    });

    Ok(())
}

// gets the app using the pid using objc bridging in rusts FFI (foreign function interface)
unsafe fn try_switch_to_pid(pid: u32) -> Result<(), String> {
    let cls: &objc::runtime::Class = objc::runtime::Class::get("NSRunningApplication")
        .ok_or("Failed to get NSRunningApplication class")?;

    let app_instance: *mut Object = msg_send![cls, 
        runningApplicationWithProcessIdentifier: pid as i32];

    if app_instance.is_null() {
        return Err(format!("No application found with PID {}", pid));
    }

    // NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps = 3
    let _: () = msg_send![app_instance, activateWithOptions: 3];

    Ok(())
}

// get the running and installed apps returned as Vec
pub fn get_combined_apps() -> Result<Vec<AppMetadata>, String> {
    let mut running_apps = get_running_apps()?;

    let mut installed_apps = get_installed_apps()?;

    // de-dupe
    installed_apps.retain(|installed| {
        !running_apps
            .iter()
            .any(|running| running.name == installed.name)
    });

    running_apps.extend(installed_apps);

    Ok(running_apps)
}

// returns the running apps and installed apps along with their app icons
#[tauri::command]
pub fn get_apps_data() -> Result<Vec<SearchSection>, String> {
    let mut sections = Vec::new();

    let mut comibined_apps: Vec<AppMetadata> = get_combined_apps()?;

    process_icons_in_parallel(&mut comibined_apps);

    comibined_apps.sort_by(|a, b| a.name.cmp(&b.name));

    let app_items: Vec<SearchItem> = comibined_apps
        .into_iter()
        .map(|app| SearchItem::App(app))
        .collect();

    sections.push(SearchSection {
        type_: SectionType::Apps,
        title: "Applications".to_string(),
        items: app_items,
    });

    Ok(sections)
}

// runs function to get app icons in parallel
pub fn process_icons_in_parallel(apps: &mut Vec<AppMetadata>) {
    // get paths for parallelization
    let paths_and_names: Vec<(String, String)> = apps
        .iter()
        .map(|app| (app.path.clone(), app.name.clone()))
        .collect();

    // Process icons in parallel and collect results
    let icons: Vec<_> = paths_and_names
        .par_iter()
        .map(|(path, name)| {
            let icon = get_app_icon(path, name);
            icon
        })
        .collect();

    // Assign the icons back to the apps
    for (i, icon) in icons.into_iter().enumerate() {
        if i < apps.len() {
            apps[i].icon = icon;
        }
    }
}

// gets the app icon
// TODO: spend more time optimizing this later, icon convresion still taking like 10ms
pub fn get_app_icon(app_path: &str, app_name: &str) -> Option<String> {
    // Check cache first
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached_icon) = cache.get(app_path) {
            return cached_icon.clone();
        }
    }

    // Handle known problematic apps with hardcoded paths and return svg
    match app_path {
        path if path.contains("/System/Applications/Calendar.app") => {
            return get_app_icon_fallback(app_path, app_name);
        }
        path if path.contains("/System/Applications/Photo Booth.app") => {
            return get_app_icon_fallback(app_path, app_name);
        }
        path if path.contains("/System/Applications/System Settings.app") => {
            return get_app_icon_fallback(app_path, app_name);
        }
        _ => {}
    }

    // common icon path names
    let potential_icon_paths = vec![
        format!("{}/Contents/Resources/{}.icns", app_path, app_name),
        format!("{}/Contents/Resources/AppIcon.icns", app_path),
        format!("{}/Contents/Resources/Icon.icns", app_path),
        format!("{}/Contents/Resources/electron.icns", app_path),
    ];

    let mut icon_path = None;
    for path in potential_icon_paths {
        if Path::new(&path).exists() {
            icon_path = Some(path);
            break;
        }
    }

    // If no icon found, try to read Info.plist to find the icon file name
    if icon_path.is_none() {
        let info_plist_path = format!("{}/Contents/Info.plist", app_path);
        if let Ok(info_plist_content) = fs::read_to_string(&info_plist_path) {
            // CFBundleIconName is the name of the icon
            if let Some(icon_file_start) = info_plist_content.find("<key>CFBundleIconName</key>") {
                if let Some(string_start) = info_plist_content[icon_file_start..].find("<string>") {
                    if let Some(string_end) =
                        info_plist_content[icon_file_start + string_start..].find("</string>")
                    {
                        let start_pos = icon_file_start + string_start + "<string>".len();
                        let end_pos = icon_file_start + string_start + string_end;
                        let icon_file = &info_plist_content[start_pos..end_pos];

                        // Add .icns extension if missing
                        let icon_file_name = if icon_file.ends_with(".icns") {
                            icon_file.to_string()
                        } else {
                            format!("{}.icns", icon_file)
                        };

                        icon_path = Some(format!(
                            "{}/Contents/Resources/{}",
                            app_path, icon_file_name
                        ));
                    }
                }
            }
        }
    }

    // Read and process the icon file if found
    if let Some(path) = icon_path {
        if let Ok(icon_data) = fs::read(&path) {
            unsafe {
                let pool = NSAutoreleasePool::new(nil);

                // Create an NSData object with the icon data
                let ns_data: id = msg_send![class!(NSData), dataWithBytes:icon_data.as_ptr() length:icon_data.len()];
                if ns_data.is_null() {
                    println!("Failed to create NSData from icon data");
                    return get_app_icon_fallback(app_path, app_name);
                }

                // Create an NSImage from the NSData
                let ns_image: id = msg_send![class!(NSImage), alloc];
                let ns_image: id = msg_send![ns_image, initWithData:ns_data];
                if ns_image.is_null() {
                    println!("Failed to create NSImage from NSData");
                    return get_app_icon_fallback(app_path, app_name);
                }

                // Resize the image to 32x32 for better performance
                let size = NSSize::new(32.0, 32.0);
                let _: () = msg_send![ns_image, setSize:size];

                // Convert to PNG representation - first get the TIFF representation
                let tiff_data: id = msg_send![ns_image, TIFFRepresentation];
                if tiff_data.is_null() {
                    println!("Failed to get TIFF representation");
                    return get_app_icon_fallback(app_path, app_name);
                }

                // Then create the bitmap representation from the TIFF data
                let bitmap_rep: id =
                    msg_send![class!(NSBitmapImageRep), imageRepWithData:tiff_data];

                if bitmap_rep.is_null() {
                    println!("Failed to create bitmap representation");
                    return get_app_icon_fallback(app_path, app_name);
                }

                let properties: id = msg_send![class!(NSDictionary), dictionary];
                let png_data: id = msg_send![bitmap_rep, representationUsingType:NSPNGFileType properties:properties];

                if png_data.is_null() {
                    println!("Failed to create PNG data");
                    return get_app_icon_fallback(app_path, app_name);
                }

                // Get raw bytes from NSData for base64 encoding
                let length: usize = msg_send![png_data, length];
                let bytes: *const u8 = msg_send![png_data, bytes];
                let data_slice = std::slice::from_raw_parts(bytes, length);

                // Base64 encode
                let base64_result = format!(
                    "data:image/png;base64,{}",
                    BASE64_STANDARD.encode(data_slice)
                );

                pool.drain();

                if let Ok(mut cache) = ICON_CACHE.lock() {
                    cache.insert(app_path.to_string(), Some(base64_result.clone()));
                }
                return Some(base64_result);
            }
        } else {
            println!("Failed to read icon file: {}", path);
        }
    }

    // Use our fast fallback if we couldn't find or process an icon
    println!("No .icns file found, using fast fallback for {}", app_path);
    get_app_icon_fallback(app_path, app_name)
}

// Improved fallback method that replaces the slow NSWorkspace approach
pub fn get_app_icon_fallback(app_path: &str, app_name: &str) -> Option<String> {
    // Extract the first letter of the app name for our letter-based icon
    let first_letter = app_name
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .next()
        .unwrap_or('A');

    // Generate a color based on the app name for visual differentiation
    let hash = app_name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
    let hue = hash % 360;

    // Create a simple colored SVG with the first letter
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
            <rect x="0" y="0" width="64" height="64" rx="12" fill="hsl({}, 70%, 60%)"/>
            <text x="32" y="42" font-family="Arial" font-size="32" font-weight="bold" 
                  text-anchor="middle" fill="white">{}</text>
        </svg>"#,
        hue, first_letter
    );

    let base64_svg = format!(
        "data:image/svg+xml;base64,{}",
        BASE64_STANDARD.encode(svg.as_bytes())
    );

    // Cache the result
    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.insert(app_path.to_string(), Some(base64_svg.clone()));
    }

    Some(base64_svg)
}

#[tauri::command]
pub async fn force_quit_application(pid: u32) -> Result<(), String> {
    // On macOS, you can use the "kill" command to force quit apps
    match std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status()
    {
        Ok(status) => {
            if status.success() {
                Ok(())
            } else {
                Err(format!(
                    "Failed to force quit application. Exit code: {:?}",
                    status.code()
                ))
            }
        }
        Err(e) => Err(format!("Failed to execute kill command: {}", e)),
    }
}

#[tauri::command]
pub async fn restart_application(
    app: AppMetadata,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    // Step 1: Force quit if it's running
    if let Some(pid) = app.pid {
        // Try to force quit
        let _ = force_quit_application(pid).await;

        // Wait a moment for the app to fully quit
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Step 2: Launch the app
    Command::new("open")
        .arg(&app.path)
        .status()
        .map_err(|e| format!("Failed to launch application: {}", e))?;

    // Step 3: Update the frontend after restarting
    tokio::spawn(async move {
        // Wait a bit for the app to start
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Try to find the newly launched app
        if let Ok(apps) = get_running_apps() {
            if let Some(new_app) = apps.iter().find(|a| a.path == app.path) {
                if let Some(new_pid) = new_app.pid {
                    if let Ok(usage) = crate::resource_monitor::get_process_resource_usage(new_pid)
                    {
                        // Create updated app with resource data
                        let mut updated_app = new_app.clone();
                        updated_app.resource_usage = Some(usage);

                        // Emit to frontend
                        let _ = app_handle.emit("app-restarted", updated_app);
                    }
                }
            }
        }
    });

    Ok(())
}
