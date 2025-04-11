use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::sleep,
    time::Duration,
};
use sysinfo::{ProcessExt, System, SystemExt};
use tauri::{Emitter, Manager, State};
use tokio::time::interval;

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
pub fn init_resource_monitor<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(ResourceMonitorState::default());
    println!("Resource monitoring initialized");
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
        let mut tick_interval = interval(Duration::from_secs(60));

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
                    usage_map.insert(
                        *pid,
                        AppResourceUsage {
                            pid: *pid,
                            cpu_usage: process.cpu_usage() as f64,
                            memory_bytes: process.memory(),
                        },
                    );
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
