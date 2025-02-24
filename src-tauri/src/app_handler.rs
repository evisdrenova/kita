use libproc::libproc::proc_pid;
use libproc::processes;
use serde::{Serialize, Deserialize};   
use std::path::Path;
use std::fs;
use std::path::PathBuf;
use std::env;
use std::process::{Command};

#[derive(Debug, Serialize, Deserialize)]
pub struct AppMetadata {
    name: String,
    path: String,
    is_running: bool, 
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
                            is_running: false,
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

    // gets all of the user-facing desktop applications on macOS that have a GUI, 
    // this filters out background processes
    // handle errors by using a mapp_err
    let pids: Vec<u32> = processes::pids_by_type(processes::ProcFilter::All).map_err(|e| format!("Failed to list PIDs: {}", e))?;

    let mut desktop_apps: Vec<AppMetadata> = Vec::new();

    for pid in pids {
        // pid 0 is kernal process, skip it
        if pid == 0 {continue;}

        // get the process path
        if let Ok(path) = proc_pid::pidpath(pid.try_into().unwrap()){
            if path.contains(".app") &&  (path.starts_with("/Applications") || path.starts_with("/System/Applications") || path.contains("/Users/") && path.contains("/Applications/")) {

                if let Some(app_name) = Path::new(&path).file_name().and_then(|n| n.to_str()).map(|s| s.replace(".app", "")){
                    if !app_name.contains("Helper") && 
                       !app_name.contains("Agent") && 
                       !app_name.ends_with("Assistant") && 
                       !app_name.starts_with("com.") && 
                       !app_name.starts_with("plugin_") {

                        desktop_apps.push(AppMetadata {
                            name: app_name,
                            path,
                            is_running: false,
                             pid: Some(pid)
                        });
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

    // let mut cmd = Command::new(app.path);

    // let child = cmd.spawn()
    // .map_err(|e| format!("Failed to start application: {}", e))?;

    // let pid = child.id();

    // println!("Started application with PID: {}", pid);

    
    let status = Command::new("open")
    .arg(&app.path)
    .spawn()
    .map_err(|e| format!("Failed to launch application: {}", e))?;

    println!("Launching application: {}", app.path);

    Ok(())


    // unsafe {
    //     if app.is_running {
    //         // Switch to running app using NSRunningApplication
    //         if let Some(pid) = app.pid {
    //             let cls = Class::get("NSRunningApplication")
    //                 .ok_or("Failed to get NSRunningApplication class")?;

    //             // Explicitly define the selector
    //             let sel_running_app = sel!(runningApplicationWithProcessIdentifier:);
    //             let ns_app: *mut Object = msg_send![cls, sel_running_app, pid as i32];
                
    //             if ns_app.is_null() {
    //                 return Err(format!("No application found with PID {}", pid));
    //             }

    //             // Explicitly define the activate selector
    //             let sel_activate = sel!(activateWithOptions:);
    //             let _: () = msg_send![ns_app, sel_activate, 3]; // NSApplicationActivateAllWindows | NSApplicationActivateIgnoringOtherApps
                
    //             Ok(())
    //         } else {
    //             Err("PID not provided for running app".to_string())
    //         }
    //     } else {
    //         // Launch new app using NSWorkspace
    //         let cls = Class::get("NSWorkspace")
    //             .ok_or("Failed to get NSWorkspace class")?;

    //         // Get sharedWorkspace instance
    //         let workspace: *mut Object = msg_send![cls, sharedWorkspace];
            
    //         if workspace.is_null() {
    //             return Err("Failed to get shared NSWorkspace".to_string());
    //         }

    //         // Convert app.path to NSString
    //         let path_nsstring: *mut Object = msg_send![class!(NSString), stringWithUTF8String: app.path.as_ptr() as *const i8];
            
    //         if path_nsstring.is_null() {
    //             return Err(format!("Failed to create NSString from path: {}", app.path));
    //         }

    //         // Launch the application
    //         let success: bool = msg_send![workspace, launchApplicationAtPath: path_nsstring
    //             options: 0  // Default launch options
    //             configuration: std::ptr::null_mut()  // No configuration
    //             error: std::ptr::null_mut()];  // Ignore error pointer for simplicity

    //         if success {
    //             Ok(())
    //         } else {
    //             Err(format!("Failed to launch app at path: {}", app.path))
    //         }
    //     }
    // }
}