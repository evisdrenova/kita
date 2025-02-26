use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tokio::time::interval;
use sysinfo::{System, SystemExt, ProcessExt};
use std::time::Duration;
use std::thread::sleep;
use serde::{Serialize, Deserialize};


use crate::app_handler::{get_all_apps, get_running_apps, AppMetadata};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppResourceUsage {
    pub pid: u32,
    pub cpu_usage: f64,        // CPU percentage (0-100)
    pub memory_bytes: u64,     // Memory usage in bytes 
}

// resource monitoring state
#[derive(Default)]
pub struct ResourceMonitorState {
    monitored_pids: Arc<Mutex<Vec<u32>>>,
    is_monitoring: Arc<Mutex<bool>>,
}


// start monitoring specific PIDs
#[tauri::command]
pub async fn start_resource_monitoring(
    pids: Vec<u32>,
    app_handle: tauri::AppHandle,
    state: State<'_, ResourceMonitorState>,
) -> Result<(), String> {
    *state.is_monitoring.lock().unwrap() = true;
    
    // update the list of monitored PIDs
    {
        let mut monitored = state.monitored_pids.lock().unwrap();
        *monitored = pids;
    }
    
    // clone what we need for the background task
    let is_monitoring = state.is_monitoring.clone();
    let monitored_pids = state.monitored_pids.clone();
    
    // start the monitoring task
    tokio::spawn(async move {
        let mut system = System::new();
        let mut update_interval = interval(Duration::from_secs(1));
        
        // continue monitoring until flag is turned off
        while *is_monitoring.lock().unwrap() {
            // current list of PIDs to monitor
            let pids_to_monitor: Vec<u32> = {
                monitored_pids.lock().unwrap().clone()
            };
            
            if !pids_to_monitor.is_empty() {
                // refresh system processes for monitoring
                system.refresh_processes();
                
                // create payload for updated resources
                let mut updated_resources = HashMap::new();
                
                for pid in pids_to_monitor {
                    let sys_pid = sysinfo::Pid::from(pid as usize);
                    
                    if let Some(process) = system.process(sys_pid) {
                        let cpu_usage = process.cpu_usage();
                        let memory_bytes = process.memory();
          
                        
                        updated_resources.insert(pid, AppResourceUsage {
                            pid,
                            cpu_usage: cpu_usage.into(),
                            memory_bytes,
                        });
                    }
                }
                
                // emit the updated resources to the frontend
                if !updated_resources.is_empty() {
                    let _ = app_handle.emit("resource-usage-updated", updated_resources);
                }
            }
            
            // wait for the next update interval
            update_interval.tick().await;
        }
        
        println!("Resource monitoring stopped");
    });
    
    Ok(())
}


#[tauri::command]
pub fn stop_resource_monitoring(state: State<'_, ResourceMonitorState>) -> Result<(), String> {
    *state.is_monitoring.lock().unwrap() = false;
    Ok(())
}

#[tauri::command]
pub fn get_resource_data(pids: Vec<u32>) -> Result<HashMap<u32, AppResourceUsage>, String> {
    let mut system = System::new();
    system.refresh_processes();
    
    // small delay for CPU measurement
    sleep(Duration::from_millis(100));
    
    // refresh again to get the CPU usage delta
    system.refresh_processes();
    
    let mut result = HashMap::new();
    
    for pid in pids {
        let sys_pid = sysinfo::Pid::from(pid as usize);
        
        if let Some(process) = system.process(sys_pid) {
            let cpu_usage = process.cpu_usage();
            let memory_bytes = process.memory();
            
            result.insert(pid, AppResourceUsage {
                pid,
                cpu_usage: cpu_usage.into(),
                memory_bytes,
            });
        }
    }
    
    Ok(result)
}

#[tauri::command]
pub async fn get_apps_with_live_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
    // start a background task to get all apps and monitor their resources
    tokio::spawn(async move {
        let mut system = System::new();
        let mut update_interval = interval(Duration::from_secs(1));
        
        loop {
            match get_all_apps() {
                Ok(mut apps) => {
                    // Refresh system data
                    system.refresh_processes();
                    
                    // Add resource usage for running apps
                    for app in &mut apps {
                        if let Some(pid) = app.pid {
                            let sys_pid = sysinfo::Pid::from(pid as usize);
                            
                            if let Some(process) = system.process(sys_pid) {
                                let cpu_usage = process.cpu_usage();
                                let memory_bytes = process.memory();
                                
                                app.resource_usage = Some(AppResourceUsage {
                                    pid,
                                    cpu_usage: cpu_usage.into(),
                                    memory_bytes,
                                });
                            }
                        }
                    }
                    
                    // Emit event with updated apps
                    let _ = app_handle.emit("apps-with-resources-updated", apps);
                },
                Err(e) => {
                    println!("Error getting apps: {}", e);
                }
            }
            
            // Wait for next interval
            update_interval.tick().await;
        }
    });
    
    Ok(())
}

// Register all commands and state with Tauri
pub fn init<R: tauri::Runtime>(app: &mut tauri::App<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Register our resource monitoring state
    app.manage(ResourceMonitorState::default());
    
    println!("Resource monitoring system initialized");
    Ok(())
}

// Gets CPU and memory usage for a specific process
pub fn get_process_resource_usage(pid: u32) -> Result<AppResourceUsage, String> {
    let mut system = System::new();
    
    // We need to refresh twice for accurate CPU usage measurement
    system.refresh_processes();
    
    // Convert u32 to proper Pid type
    let sys_pid = sysinfo::Pid::from(pid as usize);
    
    // Check if process exists
    if !system.process(sys_pid).is_some() {
        return Err(format!("Process with PID {} not found", pid));
    }
    
    // Small delay for CPU measurement
    sleep(Duration::from_millis(100));
    
    // Refresh again to get CPU usage delta
    system.refresh_processes();
    
    if let Some(process) = system.process(sys_pid) {
        let cpu_usage = process.cpu_usage();
        let memory_bytes = process.memory();

        Ok(AppResourceUsage {
            pid,
            cpu_usage: cpu_usage.into(),
            memory_bytes,

        })
    } else {
        Err(format!("Process with PID {} disappeared during measurement", pid))
    }
}

// Updated function to get all apps with resource usage
pub fn get_all_apps_with_usage() -> Result<Vec<AppMetadata>, String> {
    let mut apps = get_all_apps()?;
    
    // Setup sysinfo once for all processes
    let mut system = System::new();
    system.refresh_processes();
    
    // Wait a moment for proper CPU usage calculation
    sleep(Duration::from_millis(100));
    
    // Refresh again to get the delta for CPU usage
    system.refresh_processes();
    
    // Add resource usage for running apps (those with PIDs)
    for app in &mut apps {
        if let Some(pid) = app.pid {
            let sys_pid = sysinfo::Pid::from(pid as usize);
            
            if let Some(process) = system.process(sys_pid) {
                let cpu_usage = process.cpu_usage();
                let memory_bytes = process.memory();
               
                
                app.resource_usage = Some(AppResourceUsage {
                    pid,
                    cpu_usage: cpu_usage.into(),
                    memory_bytes,
      
                });
            }
        }
    }
    
    Ok(apps)
}

// Function to continuously monitor the resources of multiple apps
#[tauri::command]
pub async fn monitor_app_resources(app_handle: tauri::AppHandle) -> Result<(), String> {
    // Spawn a background task
    tokio::spawn(async move {
        let mut system = System::new();
        let mut update_interval = tokio::time::interval(Duration::from_secs(1));
        
        loop {
            // Get the current running apps
            match get_running_apps() {
                Ok(apps) => {
                    // Filter to get only apps with PIDs
                    let running_pids: Vec<u32> = apps.iter()
                        .filter_map(|app| app.pid)
                        .collect();
                    
                    if !running_pids.is_empty() {
                        // Refresh sysinfo data
                        system.refresh_processes();
                        
                        // Create payload with resource data
                        let mut payload = std::collections::HashMap::new();
                        
                        for pid in running_pids {
                            let sys_pid = sysinfo::Pid::from(pid as usize);
                            
                            if let Some(process) = system.process(sys_pid) {
                                let cpu_usage = process.cpu_usage();
                                let memory_bytes = process.memory();
                        
                                
                                payload.insert(pid, AppResourceUsage {
                                    pid,
                                    cpu_usage: cpu_usage.into(),
                                    memory_bytes,
                      
                                });
                            }
                        }
                        
                        // Emit event with updated resource data
                        if !payload.is_empty() {
                            let _ = app_handle.emit("resource-usage-updated", payload);
                        }
                    }
                },
                Err(e) => {
                    println!("Error getting running apps: {}", e);
                }
            }
            
            // Wait for next update interval
            update_interval.tick().await;
        }
    });
    
    Ok(())
}