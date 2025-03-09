// the objc crate has some cgs errors that it throws
// these shouldn't impact functionality so silencing for now until they fix them in a new version or something
#![allow(unexpected_cfgs)]

use base64::prelude::*;
use cocoa::base::{id, nil};
use cocoa::foundation::{NSAutoreleasePool, NSSize, NSUInteger};
use lazy_static::lazy_static;
use libproc::libproc::proc_pid;
use libproc::processes;
use objc::runtime::Object;
use objc::{class, msg_send, sel, sel_impl};
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
use objc2_app_kit::NSBitmapImageFileType;


use crate::resource_monitor::AppResourceUsage;

// used to cache the app icons so that we don't have to load them every time
lazy_static! {
    static ref ICON_CACHE: Mutex<HashMap<String, Option<String>>> = Mutex::new(HashMap::new());
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
pub fn get_apps_data() -> Result<Vec<AppMetadata>, String> {
    let mut combined_apps: Vec<AppMetadata> = get_combined_apps()?;

    process_icons_in_parallel(&mut combined_apps);

    combined_apps.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(combined_apps)
}

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

pub fn get_app_icon(app_path: &str, app_name: &str) -> Option<String> {

    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached_icon) = cache.get(app_path) {
            return cached_icon.clone();
        }
    }

    unsafe {
        let pool = NSAutoreleasePool::new(nil);

        // Convert app_path to NSString
        let cstr = std::ffi::CString::new(app_path).unwrap_or_default();
        let ns_string: id = msg_send![class!(NSString), alloc];
        let ns_app_path: id = msg_send![ns_string, initWithUTF8String:cstr.as_ptr()];

        if ns_app_path.is_null() {
            println!("Failed to create NSString from path: {}", app_path);
            pool.drain();
            return get_app_icon_fallback(app_path, app_name);
        }

        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        let ns_image: id = msg_send![workspace, iconForFile: ns_app_path];

        if ns_image.is_null() {
            println!("iconForFile returned null for: {}", app_path);
            pool.drain();
            return get_app_icon_fallback(app_path, app_name);
        }

        let size = NSSize::new(32.0, 32.0);
        let _: () = msg_send![ns_image, setSize:size];

        let png_data: Vec<u8> = nsimage_to_png_data(ns_image)?;

        let base64_result = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(&png_data)
        );
        
        pool.drain();

        if let Ok(mut cache) = ICON_CACHE.lock() {
            cache.insert(app_path.to_string(), Some(base64_result.clone()));
        }

        Some(base64_result)
    }
}

fn nsimage_to_png_data(ns_image: id) -> Option<Vec<u8>> {
    unsafe {
        // Set up an autorelease pool to catch any Objective-C exceptions
        let pool = NSAutoreleasePool::new(nil);
        
        // Wrap everything in a closure to ensure cleanup happens
        let result = (|| {
            // Get CGImage from NSImage
            let cg_image: id = msg_send![ns_image, CGImageForProposedRect:nil context:nil hints:nil];
            if cg_image.is_null() {
                return None;
            }
            
            // Create NSBitmapImageRep from CGImage
            let bitmap_rep: id = msg_send![class!(NSBitmapImageRep), alloc];
            let bitmap_rep: id = msg_send![bitmap_rep, initWithCGImage:cg_image];
            if bitmap_rep.is_null() {
                return None;
            }
            
            // Set the size of the bitmap representation to match the NSImage size
            let size: NSSize = msg_send![ns_image, size];
            let _: () = msg_send![bitmap_rep, setSize:size];
            
            // Get PNG representation
            let png_type = NSBitmapImageFileType::PNG;
            let empty_properties: id = msg_send![class!(NSDictionary), dictionary];
            let png_data: id = msg_send![bitmap_rep, representationUsingType:png_type.0 properties:empty_properties];
        
            if png_data.is_null() {
                let _: () = msg_send![bitmap_rep, release];
                return None;
            }
            
            // Get data length and bytes
            let length = ns_data_length(png_data);
            let bytes = ns_data_bytes(png_data);
            
            if bytes.is_null() || length == 0 {
                let _: () = msg_send![bitmap_rep, release];
                return None;
            }
            
            let mut data = Vec::with_capacity(length);
            std::ptr::copy_nonoverlapping(bytes, data.as_mut_ptr(), length);
            data.set_len(length);
            
            // Clean up the bitmap rep, but not the png_data yet as we're using its bytes
            let _: () = msg_send![bitmap_rep, release];
            
            Some(data)
        })();
        
        // Drain the autorelease pool to clean up any Objective-C objects
        pool.drain();
        
        result
    }
}

fn ns_data_length(data: id) -> usize {
    unsafe {
        if data.is_null() {
            return 0;
        }
        
        let result: NSUInteger = msg_send![data, length];
        result as usize
    }
}

fn ns_data_bytes(data: id) -> *const u8 {
    unsafe {
        if data.is_null() {
            return std::ptr::null();
        }
        
        let result: *const std::ffi::c_void = msg_send![data, bytes];
        result as *const u8
    }
}

// create custom svg image if we can't get the app icon in a reasonable amount of time
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
    // On macOS, we can use the "kill" command to force quit apps
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
    // try to force quit
    if let Some(pid) = app.pid {
        let _ = force_quit_application(pid).await;

        // Wait a moment for the app to fully quit
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // launch the app
    Command::new("open")
        .arg(&app.path)
        .status()
        .map_err(|e| format!("Failed to launch application: {}", e))?;

    // update the frontend after restarting
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

                        let _ = app_handle.emit("app-restarted", updated_app);
                    }
                }
            }
        }
    });

    Ok(())
}