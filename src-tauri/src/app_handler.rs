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
use objc::{msg_send, sel, sel_impl};
use objc::runtime::Object;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppMetadata {
    name: String,
    path: String,
    pid: Option<u32>  
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
                            pid: None 
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


// combines the running and installed apps into one vector
pub fn get_all_apps() -> Result<Vec<AppMetadata>, String> {

    let running_apps = get_running_apps()?;
    let mut installed_apps = get_installed_apps()?;

    // de-dupe running apps and installed apps
    installed_apps.retain(|installed| {
        !running_apps.iter().any(|running | running.name == installed.name)
    });

    let mut all_apps = running_apps;
    all_apps.extend(installed_apps);

    // sort them alphabetically
    all_apps.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(all_apps)
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