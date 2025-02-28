// use std::collections::HashMap;
// use std::sync::{Arc, Mutex};
// use tauri::{Emitter, Manager, State};
// use tokio::time::interval;
// use sysinfo::{System, SystemExt, ProcessExt};
// use std::time::Duration;
// use std::thread::sleep;
// use serde::{Serialize, Deserialize};
// use std::panic::{self, AssertUnwindSafe};

// use crate::app_handler::{get_all_apps, get_running_apps, AppMetadata};


// #[derive(Debug, Serialize, Deserialize, Clone)]
// pub struct AppResourceUsage {
//     pub pid: u32,
//     pub cpu_usage: f64,        // CPU percentage (0-100)
//     pub memory_bytes: u64,     // Memory usage in bytes 
// }

// // resource monitoring state
// #[derive(Default)]
// pub struct ResourceMonitorState {
//     monitored_pids: Arc<Mutex<Vec<u32>>>,
//     is_monitoring: Arc<Mutex<bool>>,
// }


// // start monitoring specific PIDs
// #[tauri::command]
// pub async fn start_resource_monitoring(
//     pids: Vec<u32>,
//     app_handle: tauri::AppHandle,
//     state: State<'_, ResourceMonitorState>,
// ) -> Result<(), String> {
//     // Set monitoring flag to true
//     *state.is_monitoring.lock().unwrap() = true;
    
//     // Update the list of monitored PIDs - safely
//     {
//         let mut monitored = state.monitored_pids.lock().unwrap();
//         let mut valid_pids = Vec::new();
        
//         // Create system once outside loop
//         let mut system = System::new();
//         system.refresh_processes();
        
//         // Filter out any invalid PIDs
//         for pid in pids {
//             let sys_pid = sysinfo::Pid::from(pid as usize);
//             if system.process(sys_pid).is_some() {
//                 valid_pids.push(pid);
//             }
//         }
        
//         *monitored = valid_pids;
//     }
    
//     // Clone what we need for the background task
//     let is_monitoring = state.is_monitoring.clone();
//     let monitored_pids = state.monitored_pids.clone();
    
//     // Spawn the monitoring task
//     tokio::spawn(async move {
//         let mut system = System::new();
//         let mut update_interval = tokio::time::interval(Duration::from_secs(1));
        
//         // Continue monitoring until flag is turned off
//         while *is_monitoring.lock().unwrap() {
//             // Get the current list of PIDs to monitor
//             let pids_to_monitor: Vec<u32> = {
//                 monitored_pids.lock().unwrap().clone()
//             };
            
//             if !pids_to_monitor.is_empty() {
//                 // Wrap in catch_unwind to prevent panics
//         let refresh_result = panic::catch_unwind(AssertUnwindSafe(|| {
//                     system.refresh_processes();
//                 }));
//                 if refresh_result.is_ok() {
//                     // Create payload for updated resources
//                     let mut updated_resources = std::collections::HashMap::new();
                    
//                     // Process each PID
//                     for pid in &pids_to_monitor {
//                         let sys_pid = sysinfo::Pid::from(*pid as usize);
                        
//                         if let Some(process) = system.process(sys_pid) {
//                             let cpu_usage = process.cpu_usage();
//                             let memory_bytes = process.memory();
                            
                            
//                             updated_resources.insert(*pid, AppResourceUsage {
//                                 pid: *pid,
//                                 cpu_usage: cpu_usage.into(),
//                                 memory_bytes,
                   
//                             });
//                         }
//                     }
                    
//                     // Emit the updated resources to the frontend
//                     if !updated_resources.is_empty() {
//                         let _ = app_handle.emit("resource-usage-updated", updated_resources);
//                     }
//                 } else {
//                     println!("Error refreshing system processes");
//                 }
//             }
            
//             // Simply await the tick - if it panics, the task will end but won't crash the process
//             update_interval.tick().await;
//         }
        
//         println!("Resource monitoring stopped");
//     });
    
//     Ok(())
// }

// #[tauri::command]
// pub fn stop_resource_monitoring(state: State<'_, ResourceMonitorState>) -> Result<(), String> {
//     *state.is_monitoring.lock().unwrap() = false;
//     Ok(())
// }

// #[tauri::command]
// pub fn get_resource_data(pids: Vec<u32>) -> Result<HashMap<u32, AppResourceUsage>, String> {
//     let mut system = System::new();
//     system.refresh_processes();
    
//     // small delay for CPU measurement
//     sleep(Duration::from_millis(100));
    
//     // refresh again to get the CPU usage delta
//     system.refresh_processes();
    
//     let mut result = HashMap::new();
    
//     for pid in pids {
//         let sys_pid = sysinfo::Pid::from(pid as usize);
        
//         if let Some(process) = system.process(sys_pid) {
//             let cpu_usage = process.cpu_usage();
//             let memory_bytes = process.memory();
            
//             result.insert(pid, AppResourceUsage {
//                 pid,
//                 cpu_usage: cpu_usage.into(),
//                 memory_bytes,
//             });
//         }
//     }
    
//     Ok(result)
// }

// #[tauri::command]
// pub async fn get_apps_with_live_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
//     // start a background task to get all apps and monitor their resources
//     tokio::spawn(async move {
//         let mut system = System::new();
//         let mut update_interval = interval(Duration::from_secs(1));
        
//         loop {
//             match get_all_apps() {
//                 Ok(mut apps) => {
//                     // Refresh system data
//                     system.refresh_processes();
                    
//                     // Add resource usage for running apps
//                     for app in &mut apps {
//                         if let Some(pid) = app.pid {
//                             let sys_pid = sysinfo::Pid::from(pid as usize);
                            
//                             if let Some(process) = system.process(sys_pid) {
//                                 let cpu_usage = process.cpu_usage();
//                                 let memory_bytes = process.memory();
                                
//                                 app.resource_usage = Some(AppResourceUsage {
//                                     pid,
//                                     cpu_usage: cpu_usage.into(),
//                                     memory_bytes,
//                                 });
//                             }
//                         }
//                     }
                    
//                     // Emit event with updated apps
//                     let _ = app_handle.emit("apps-with-resources-updated", apps);
//                 },
//                 Err(e) => {
//                     println!("Error getting apps: {}", e);
//                 }
//             }
            
//             // Wait for next interval
//             update_interval.tick().await;
//         }
//     });
    
//     Ok(())
// }

// // Register all commands and state with Tauri
// pub fn init<R: tauri::Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
//     // Register our resource monitoring state
//     app.manage(ResourceMonitorState::default());
    
//     println!("Resource monitoring system initialized");
//     Ok(())
// }

// // Gets CPU and memory usage for a specific process
// pub fn get_process_resource_usage(pid: u32) -> Result<AppResourceUsage, String> {
//     let mut system = System::new();
    
//     // We need to refresh twice for accurate CPU usage measurement
//     system.refresh_processes();
    
//     // Convert u32 to proper Pid type
//     let sys_pid = sysinfo::Pid::from(pid as usize);
    
//     // Check if process exists
//     if !system.process(sys_pid).is_some() {
//         return Err(format!("Process with PID {} not found", pid));
//     }
    
//     // Small delay for CPU measurement
//     sleep(Duration::from_millis(100));
    
//     // Refresh again to get CPU usage delta
//     system.refresh_processes();
    
//     if let Some(process) = system.process(sys_pid) {
//         let cpu_usage = process.cpu_usage();
//         let memory_bytes = process.memory();

//         Ok(AppResourceUsage {
//             pid,
//             cpu_usage: cpu_usage.into(),
//             memory_bytes,

//         })
//     } else {
//         Err(format!("Process with PID {} disappeared during measurement", pid))
//     }
// }

// // Updated function to get all apps with resource usage
// pub fn get_all_apps_with_usage() -> Result<Vec<AppMetadata>, String> {
//     let mut apps = get_all_apps()?;
    
//     // Setup sysinfo once for all processes
//     let mut system = System::new();
//     system.refresh_processes();
    
//     // Wait a moment for proper CPU usage calculation
//     sleep(Duration::from_millis(100));
    
//     // Refresh again to get the delta for CPU usage
//     system.refresh_processes();
    
//     // Add resource usage for running apps (those with PIDs)
//     for app in &mut apps {
//         if let Some(pid) = app.pid {
//             let sys_pid = sysinfo::Pid::from(pid as usize);
            
//             if let Some(process) = system.process(sys_pid) {
//                 let cpu_usage = process.cpu_usage();
//                 let memory_bytes = process.memory();
               
                
//                 app.resource_usage = Some(AppResourceUsage {
//                     pid,
//                     cpu_usage: cpu_usage.into(),
//                     memory_bytes,
      
//                 });
//             }
//         }
//     }
    
//     Ok(apps)
// }

// // Function to continuously monitor the resources of multiple apps
// #[tauri::command]
// pub async fn monitor_app_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
//     // Spawn a background task
//     tokio::spawn(async move {
//         let mut system = System::new();
//         let mut update_interval = tokio::time::interval(Duration::from_secs(1));
        
//         loop {
//             // Use a try-catch block to prevent panics from propagating
//             match get_running_apps() {
//                 Ok(apps) => {
//                     // Filter to get only apps with PIDs
//                     let running_pids: Vec<u32> = apps.iter()
//                         .filter_map(|app| app.pid)
//                         .collect();
                    
//                     if !running_pids.is_empty() {
//                         // Refresh sysinfo data - wrap in a catch to prevent panics
//                         let refresh_result = panic::catch_unwind(AssertUnwindSafe(|| {
//                             system.refresh_processes();
//                         }));
                        
//                         if refresh_result.is_ok() {
//                             // Create payload with resource data
//                             let mut payload = std::collections::HashMap::new();
                            
//                             for pid in running_pids {
//                                 let sys_pid = sysinfo::Pid::from(pid as usize);
                                
//                                 // Check if process still exists
//                                 if let Some(process) = system.process(sys_pid) {
//                                     // Safely get values with default fallbacks
//                                     let cpu_usage = process.cpu_usage();
//                                     let memory_bytes = process.memory();
                             
                                    
//                                     payload.insert(pid, AppResourceUsage {
//                                         pid,
//                                         cpu_usage: cpu_usage.into(),
//                                         memory_bytes,
                      
//                                     });
//                                 }
//                             }
                            
//                             // Only emit if we have data and it's safe to do so
//                             if !payload.is_empty() {
//                                 // Use a try-catch for the emit as well
//                                 if let Err(e) = app_handle.emit("resource-usage-updated", payload) {
//                                     println!("Failed to emit resource updates: {:?}", e);
//                                 }
//                             }
//                         } else {
//                             println!("Error refreshing system processes");
//                         }
//                     }
//                 },
//                 Err(e) => {
//                     println!("Error getting running apps: {}", e);
//                     // Short sleep to prevent rapid error loops
//                     tokio::time::sleep(Duration::from_millis(500)).await;
//                 }
//             }
            
//             // For async operations in tokio, we can't use catch_unwind directly
//             // Instead, we use a simple try/catch with sleep as a fallback
//             let tick_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
//                 // Just check if the interval is valid, don't await here
//                 &update_interval
//             }));
            
//             if tick_result.is_ok() {
//                 // Safely await the tick
//                 update_interval.tick().await;
//             } else {
//                 println!("Error with interval, recreating it");
//                 // Re-create interval if it panicked
//                 update_interval = tokio::time::interval(Duration::from_secs(1));
//                 tokio::time::sleep(Duration::from_secs(1)).await;
//             }
//         }
//     });
    
//     Ok(())
// }


use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use serde::{Deserialize, Serialize};
use sysinfo::{ProcessExt, System, SystemExt};
use tauri::{Manager, State, AppHandle, Emitter};
use tokio::time::interval;



use crate::app_handler::{get_all_apps, get_combined_apps, get_running_apps, AppMetadata};



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppResourceUsage {
    pub pid: u32,
    pub cpu_usage: f64,    // CPU percentage (0-100)
    pub memory_bytes: u64, // Memory usage in bytes
}

/// Holds the shared state for resource monitoring.
#[derive(Default)]
pub struct ResourceMonitorState {
    /// List of user-requested PIDs to monitor. 
    monitored_pids: Arc<Mutex<Vec<u32>>>,

    /// Single boolean flag indicating if monitoring is active.
    is_monitoring: Arc<Mutex<bool>>,
}

/// Initialize and register the ResourceMonitorState with your Tauri app.
pub fn init<R: tauri::Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(ResourceMonitorState::default());
    println!("Resource monitoring system initialized");
    Ok(())
}

/// Starts resource monitoring for a given set of PIDs. Spawns a single background
/// task if not already active, and emits updates via "resource-usage-updated".
#[tauri::command]
pub async fn start_resource_monitoring(
    pids: Vec<u32>,
    app_handle: tauri::AppHandle,
    state: State<'_, ResourceMonitorState>,
) -> Result<(), String> {
    // Mark that we should be monitoring
    {
        let mut flag = state.is_monitoring.lock().unwrap();
        *flag = true;
    }

    // Validate the PIDs once to ensure they exist before monitoring
    {
        let mut system = System::new();
        system.refresh_processes();

        let mut valid_pids = Vec::new();
        for pid in pids {
            let sys_pid = sysinfo::Pid::from(pid as usize);
            if system.process(sys_pid).is_some() {
                valid_pids.push(pid);
            }
        }

        // Store the valid PIDs
        let mut monitored = state.monitored_pids.lock().unwrap();
        *monitored = valid_pids;
    }

    // If a background task is already running, do nothing else here.
    // We only spawn once, and let that task continuously monitor.
    // TODO: handle re-spawning the task after it stops. The task should only run while the window is in focus
    // otherwise, we don't need to update the resources if the user isn't looking at the app
    let is_monitoring_now = state.is_monitoring.clone();
    let monitored_pids_clone = state.monitored_pids.clone();

    // Spawn a background monitoring task **only** if we aren’t already running it.
    // TODO: implement check for existing task here
    tokio::spawn(async move {
        let mut system = System::new();
        let mut tick_interval = interval(Duration::from_secs(1));

        // The main loop
        loop {
            // Check if we are still supposed to monitor
            if !*is_monitoring_now.lock().unwrap() {
                println!("Resource monitoring loop exiting...");
                break;
            }

            // Refresh all processes once per tick (sysinfo uses a delta to compute CPU)
            system.refresh_processes();

            // Collect usage for the monitored PIDs
            let pids_to_monitor = { monitored_pids_clone.lock().unwrap().clone() };
            let mut usage_map = HashMap::new();

            for pid in &pids_to_monitor {
                let sys_pid = sysinfo::Pid::from(*pid as usize);
                if let Some(process) = system.process(sys_pid) {
                    usage_map.insert(*pid, AppResourceUsage {
                        pid: *pid,
                        cpu_usage: process.cpu_usage() as f64,
                        memory_bytes: process.memory(),
                    });
                }
            }

            if !usage_map.is_empty() {
                let _ = app_handle.emit("resource-usage-updated", usage_map);
            }

            tick_interval.tick().await;
        }
    });

    Ok(())
}

/// Stops the background resource monitoring loop.
#[tauri::command]
pub fn stop_resource_monitoring(state: State<'_, ResourceMonitorState>) -> Result<(), String> {
    {
        let mut flag = state.is_monitoring.lock().unwrap();
        *flag = false;
    }
    Ok(())
}


// ====================================
// On-demand retrieval of resource data
// ====================================

/// Fetch resource usage for the given list of PIDs on-demand (blocking call).
/// This uses the “two refresh” trick for a point-in-time CPU measurement if needed.
/// However, if you rely on frequent calls to refresh_processes(), sysinfo’s delta
/// logic should handle CPU usage. Adjust if you want simpler or more accurate measures.
/// this is a one time operation to get the resources once
#[tauri::command]
pub fn get_resource_data(pids: Vec<u32>) -> Result<HashMap<u32, AppResourceUsage>, String> {
    let mut system = System::new();
    // First refresh
    system.refresh_processes();
    sleep(Duration::from_millis(100));
    // Second refresh to compute CPU deltas
    system.refresh_processes();

    let mut result = HashMap::new();

    for pid in pids {
        let sys_pid = sysinfo::Pid::from(pid as usize);
        if let Some(proc_) = system.process(sys_pid) {
            result.insert(pid, AppResourceUsage {
                pid,
                cpu_usage: proc_.cpu_usage() as f64,
                memory_bytes: proc_.memory(),
            });
        }
    }

    Ok(result)
}

// ====================================
// Example: Get all apps with live resources
// ====================================

/// Example continuous app monitor that retrieves all apps and adds resource usage,
/// then emits “apps-with-resources-updated”. If you want to unify it with the 
/// existing “start_resource_monitoring,” you can. Currently, it spawns a separate loop.
#[tauri::command]
pub async fn get_apps_with_live_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
    tokio::spawn(async move {
        let mut system = System::new();
        let mut update_interval = interval(Duration::from_secs(1));

        loop {
            // Get all apps from your custom function
            match get_all_apps() {
                Ok(mut apps) => {
                    system.refresh_processes();

                    for app in &mut apps {
                        if let Some(pid) = app.pid {
                            let sys_pid = sysinfo::Pid::from(pid as usize);
                            if let Some(process) = system.process(sys_pid) {
                                app.resource_usage = Some(AppResourceUsage {
                                    pid,
                                    cpu_usage: process.cpu_usage() as f64,
                                    memory_bytes: process.memory(),
                                });
                            }
                        }
                    }

                    let _ = app_handle.emit("apps-with-resources-updated", apps);
                }
                Err(e) => {
                    println!("Error getting apps: {}", e);
                }
            }

            update_interval.tick().await;
        }
    });

    Ok(())
}

// ====================================
// Example: Monitor resource usage of running apps
// ====================================

/// Similar to “start_resource_monitoring,” but fetches “running apps” from a custom
/// function, then emits usage data. If you only need to monitor known PIDs,
/// consider removing or unifying with the prior function.
#[tauri::command]
pub async fn monitor_app_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
    tokio::spawn(async move {
        let mut system = System::new();
        let mut update_interval = interval(Duration::from_secs(1));

        loop {
            // Retrieve the running apps from your custom logic
            match get_running_apps() {
                Ok(apps) => {
                    let running_pids: Vec<u32> = apps.iter()
                        .filter_map(|app| app.pid)
                        .collect();

                    if !running_pids.is_empty() {
                        system.refresh_processes();

                        let mut payload = HashMap::new();
                        for pid in running_pids {
                            let sys_pid = sysinfo::Pid::from(pid as usize);
                            if let Some(proc_) = system.process(sys_pid) {
                                payload.insert(pid, AppResourceUsage {
                                    pid,
                                    cpu_usage: proc_.cpu_usage() as f64,
                                    memory_bytes: proc_.memory(),
                                });
                            }
                        }

                        if !payload.is_empty() {
                            if let Err(e) = app_handle.emit("resource-usage-updated", payload) {
                                println!("Failed to emit resource updates: {:?}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("Error getting running apps: {}", e);
                    // small wait to avoid busy-loop on error
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }

            update_interval.tick().await;
        }
    });

    Ok(())
}

// ====================================
// Utilities (On-demand usage for a single process)
// ====================================

/// Fetch CPU and memory usage for a single process on-demand (blocking).
pub fn get_process_resource_usage(pid: u32) -> Result<AppResourceUsage, String> {
    let mut system = System::new();
    system.refresh_processes();
    sleep(Duration::from_millis(100));
    system.refresh_processes();

    let sys_pid = sysinfo::Pid::from(pid as usize);
    if let Some(proc_) = system.process(sys_pid) {
        Ok(AppResourceUsage {
            pid,
            cpu_usage: proc_.cpu_usage() as f64,
            memory_bytes: proc_.memory(),
        })
    } else {
        Err(format!("Process with PID {} not found", pid))
    }
}

/// Fetch all apps with resource usage in one shot (blocking).
pub fn get_all_apps_with_usage() -> Result<Vec<AppMetadata>, String> {
    let mut apps = get_all_apps()?;

    let mut system = System::new();
    system.refresh_processes();
    sleep(Duration::from_millis(100));
    system.refresh_processes();

    for app in &mut apps {
        if let Some(pid) = app.pid {
            let sys_pid = sysinfo::Pid::from(pid as usize);
            if let Some(proc_) = system.process(sys_pid) {
                app.resource_usage = Some(AppResourceUsage {
                    pid,
                    cpu_usage: proc_.cpu_usage() as f64,
                    memory_bytes: proc_.memory(),
                });
            }
        }
    }
    Ok(apps)
}