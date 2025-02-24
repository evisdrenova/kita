// the objc crate has some cgs errors that it throws
// these shouldn't impact functionality so silencing for now until they fix them in a new version or something
#![allow(unexpected_cfgs)]


use libproc::libproc::proc_pid;
use libproc::processes;
use serde::{Serialize, Deserialize};   
use std::path::Path;
use std::fs;
use std::path::PathBuf;
use std::env;
use std::process::Command;
use objc::{msg_send, sel, sel_impl, class};
use objc::runtime::{Object, Class};
use core_graphics::geometry::CGSize;
// use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::sync::Mutex;
use rayon::prelude::*;
use std::time::Instant;
use base64::prelude::*;
use cocoa::base::{id, nil, NO, YES};
use cocoa::foundation::{NSAutoreleasePool,NSData, NSSize, NSString, NSRect};
use objc2_app_kit::NSBitmapImageFileType;
use objc2_app_kit::NSPNGFileType;
// use cocoa::appkit::{NSWorkspace, NSImage, NSBitmapImageFileType, NSBitmapImageRep};
// use once_cell::sync::Lazy;

// used to cache the app icons so that we don't have to load them every time
lazy_static! {
    static ref ICON_CACHE: Mutex<HashMap<String, Option<String>>> = Mutex::new(HashMap::new());
}


#[derive(Debug, Serialize, Deserialize)]
pub struct AppMetadata {
    name: String,
    path: String,
    pid: Option<u32> ,
    icon: Option<String>  // Base64 encoded icon data 
}

const APPLICATIONS_DIR: &str = "/Applications";
const SYSTEM_APPLICATIONS_DIR: &str = "/System/Applications";


// gets all of the apps installed  in the app directory
pub fn get_installed_apps() ->  Result<Vec<AppMetadata>, String> {

    let mut app_directories = vec![
        PathBuf::from(APPLICATIONS_DIR),
        PathBuf::from(SYSTEM_APPLICATIONS_DIR),
    ];

    if let Ok(home) = env::var("HOME") {
        app_directories.push(PathBuf::from(home).join("Applications"))
    }

    let mut installed_apps = Vec::new();

    for dir in app_directories {
        if let Ok(entries) = fs::read_dir(&dir){
            for entry in entries.flatten() {
                let path = entry.path();

                //check if it's an.app directory
                if path.is_dir() &&
                path.extension().and_then(|ext| ext.to_str()) == Some("app") {
                    if let Some(app_name) = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|s| s.replace(".app", "")) 
                {
                    // Skip helper apps
                    if !app_name.contains("Helper") && 
                       !app_name.contains("Agent") && 
                       !app_name.ends_with("Assistant") 
                    {
                        installed_apps.push(AppMetadata {
                            name: app_name,
                            path: path.to_string_lossy().into_owned(),
                            pid: None,
                            icon: None, 
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
        if pid == 0 { continue; }

        if let Ok(path) = proc_pid::pidpath(pid.try_into().unwrap()) {
            if path.contains(".app") {
                // Extract the .app bundle path
                if let Some(app_bundle_path) = path.split(".app").next() {
                    let bundle_path = format!("{}.app", app_bundle_path);
                    
                    if bundle_path.starts_with("/Applications") || 
                       bundle_path.starts_with("/System/Applications") || 
                       (bundle_path.contains("/Users/") && bundle_path.contains("/Applications/")) {

                        if let Some(app_name) = Path::new(&bundle_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.replace(".app", ""))
                        {
                            if !app_name.contains("Helper") && 
                               !app_name.contains("Agent") && 
                               !app_name.ends_with("Assistant") && 
                               !app_name.starts_with("com.") && 
                               !app_name.starts_with("plugin_") {

                                desktop_apps.push(AppMetadata {
                                    name: app_name,
                                    path: bundle_path,
                                    pid: Some(pid),
                                    icon: None,
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
pub async fn launch_or_switch_to_app(app: AppMetadata) -> Result<(), String> {
    // try to switch if we have a PID
    // if we have a PID then we know the app is running
    if let Some(pid) = app.pid {
        match unsafe {
            try_switch_to_pid(pid)
        } {
            Ok(()) => return Ok(()), // Successfully switched
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


// ITS DFINITELY THE APP ICON THING THAT iS SLOWING THINGS DOWN!!!!!

pub fn get_app_icon(app_path: &str, app_name: &str) -> Option<String> {
    let total_start = Instant::now();
    
    // Check cache first
    let cache_start = Instant::now();
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached_icon) = cache.get(app_path) {
            println!("Cache hit for {}: {:?}", app_path, cache_start.elapsed());
            return cached_icon.clone();
        }
    }
    println!("Cache check took: {:?}", cache_start.elapsed());

    // Look for icon at standard locations
    let icon_search = Instant::now();
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
            // Very simple approach to extract CFBundleIconFile
            if let Some(icon_file_start) = info_plist_content.find("<key>CFBundleIconFile</key>") {
                if let Some(string_start) = info_plist_content[icon_file_start..].find("<string>") {
                    if let Some(string_end) = info_plist_content[icon_file_start + string_start..].find("</string>") {
                        let start_pos = icon_file_start + string_start + "<string>".len();
                        let end_pos = icon_file_start + string_start + string_end;
                        let icon_file = &info_plist_content[start_pos..end_pos];
                        
                        // Add .icns extension if missing
                        let icon_file_name = if icon_file.ends_with(".icns") {
                            icon_file.to_string()
                        } else {
                            format!("{}.icns", icon_file)
                        };
                        
                        icon_path = Some(format!("{}/Contents/Resources/{}", app_path, icon_file_name));
                    }
                }
            }
        }
    }
    println!("Icon search took: {:?}", icon_search.elapsed());

    // Read and process the icon file if found
    if let Some(path) = icon_path {
        let read_start = Instant::now();
        if let Ok(icon_data) = fs::read(&path) {
            println!("Icon file read took: {:?}", read_start.elapsed());
            
            let conversion_start = Instant::now();
            
            // We need to extract a PNG from the ICNS
            // For simplicity, we'll use a Tauri-compatible approach for NSImage
            unsafe {
                let pool = NSAutoreleasePool::new(nil);
                
                // Create an NSData object with the icon data
                let ns_data: id = msg_send![class!(NSData), dataWithBytes:icon_data.as_ptr() length:icon_data.len()];
                if ns_data.is_null() {
                    println!("Failed to create NSData from icon data");
                    return None;
                }
                
                // Create an NSImage from the NSData
                let ns_image: id = msg_send![class!(NSImage), alloc];
                let ns_image: id = msg_send![ns_image, initWithData:ns_data];
                if ns_image.is_null() {
                    println!("Failed to create NSImage from NSData");
                    return None;
                }

                // Resize the image to 64x64
                let size = NSSize::new(64.0, 64.0);
                let _: () = msg_send![ns_image, setSize:size];
                
                // Convert to PNG representation - first get the TIFF representation
                let tiff_data: id = msg_send![ns_image, TIFFRepresentation];
                if tiff_data.is_null() {
                    println!("Failed to get TIFF representation");
                    return None;
                }
                
                // Then create the bitmap representation from the TIFF data
                let bitmap_rep: id = msg_send![class!(NSBitmapImageRep), imageRepWithData:tiff_data];
                
                if bitmap_rep.is_null() {
                    println!("Failed to create bitmap representation");
                    return None;
                }
                
                let properties: id = msg_send![class!(NSDictionary), dictionary];
                let png_data: id = msg_send![bitmap_rep, representationUsingType:NSPNGFileType properties:properties];
                
                if png_data.is_null() {
                    println!("Failed to create PNG data");
                    return None;
                }
                
                // Get raw bytes from NSData for base64 encoding
                let length: usize = msg_send![png_data, length];
                let bytes: *const u8 = msg_send![png_data, bytes];
                let data_slice = std::slice::from_raw_parts(bytes, length);
                
                // Base64 encode
                let base64_result = format!("data:image/png;base64,{}", BASE64_STANDARD.encode(data_slice));
                
                pool.drain();
                
                println!("Icon conversion took: {:?}", conversion_start.elapsed());
                
                // Cache the result
                let cache_write_start = Instant::now();
                if let Ok(mut cache) = ICON_CACHE.lock() {
                    cache.insert(app_path.to_string(), Some(base64_result.clone()));
                }
                println!("Cache write took: {:?}", cache_write_start.elapsed());
                
                println!("Total icon processing took: {:?}", total_start.elapsed());
                return Some(base64_result);
            }
        } else {
            println!("Failed to read icon file: {}", path);
        }
    }

    // Fallback to legacy method if no icon found
    println!("No .icns file found, falling back to legacy method for {}", app_path);
    let fallback_start = Instant::now();
    let result = get_app_icon_legacy(app_path);
    println!("Fallback method took: {:?}", fallback_start.elapsed());
    
    // Cache the result
    if let Some(ref icon_data) = result {
        if let Ok(mut cache) = ICON_CACHE.lock() {
            cache.insert(app_path.to_string(), Some(icon_data.clone()));
        }
    }
    
    println!("Total icon processing took: {:?}", total_start.elapsed());
    result
}


// Legacy fallback method (using the previous QuickLook implementation)
pub fn get_app_icon_legacy(app_path: &str) -> Option<String> {
    unsafe {
        let pool = NSAutoreleasePool::new(nil);
        
        let workspace: id = msg_send![class!(NSWorkspace), sharedWorkspace];
        
        let path_str: id = msg_send![class!(NSString), 
            stringWithUTF8String: app_path.as_ptr()];
        
        let icon: id = msg_send![workspace, iconForFile: path_str];
        
        if icon.is_null() {
            pool.drain();
            return None;
        }

        let size = NSSize::new(64.0, 64.0);
        let _: () = msg_send![icon, setSize: size];
        
        let tiff_data: id = msg_send![icon, TIFFRepresentation];
        
        if tiff_data.is_null() {
            pool.drain();
            return None;
        }

        let length: usize = msg_send![tiff_data, length];
        let bytes: *const u8 = msg_send![tiff_data, bytes];
        let icon_data = std::slice::from_raw_parts(bytes, length);

        let base64_result = format!("data:image/png;base64,{}", 
        BASE64_STANDARD.encode(icon_data));

        pool.drain();
        
        Some(base64_result)
    }
}

// Update process_icons_in_parallel to use the new method
pub fn process_icons_in_parallel(apps: &mut Vec<AppMetadata>) {
    let icons_start = Instant::now();
    
    // Extract paths for parallel processing
    let paths_and_names: Vec<(String, String)> = apps
        .iter()
        .map(|app| (app.path.clone(), app.name.clone()))
        .collect();
    
    // Process icons in parallel and collect results
    let icons: Vec<_> = paths_and_names
        .par_iter()
        .map(|(path, name)| {
            let single_icon_start = Instant::now();
            let icon = get_app_icon(path, name); // Use the new ICNS-based method
            println!("Icon for {} took: {:?}", name, single_icon_start.elapsed());
            icon
        })
        .collect();
    
    // Assign the icons back to the apps
    for (i, icon) in icons.into_iter().enumerate() {
        if i < apps.len() {
            apps[i].icon = icon;
        }
    }
    
    println!("Total icon processing took: {:?}", icons_start.elapsed());
}

// Keep the get_all_apps function as is since it already uses process_icons_in_parallel
pub fn get_all_apps() -> Result<Vec<AppMetadata>, String> {
    let total_start = Instant::now();

    let apps_start = Instant::now();
    let running_apps = get_running_apps()?;
    println!("Getting running apps took: {:?}", apps_start.elapsed());

    let installed_start = Instant::now();
    let mut installed_apps = get_installed_apps()?;
    println!("Getting installed apps took: {:?}", installed_start.elapsed());

    let dedup_start = Instant::now();
    // Deduplicate before processing icons to reduce workload
    installed_apps.retain(|installed| {
        !running_apps.iter().any(|running| running.name == installed.name)
    });
    println!("Deduplication took: {:?}", dedup_start.elapsed());
    
    // Combine apps before processing icons
    let mut all_apps = running_apps;
    all_apps.extend(installed_apps);
    
    // Process all icons in parallel using the new ICNS-based method
    process_icons_in_parallel(&mut all_apps);
    
    let sort_start = Instant::now();
    all_apps.sort_by(|a, b| a.name.cmp(&b.name));
    println!("Sorting took: {:?}", sort_start.elapsed());

    println!("Total get_all_apps process took: {:?}", total_start.elapsed());
    Ok(all_apps)
}