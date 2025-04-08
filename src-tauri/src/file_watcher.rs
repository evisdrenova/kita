use crate::AppResult;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::sleep;
use rusqlite::Connection;
use tracing::{debug, error, info};
use tokio::task;

use crate::file_processor::{is_valid_file_extension, FileMetadata, FileProcessorError,
    FileProcessorState, ProcessingStatus,
};
use crate::vectordb_manager::VectorDbManager;
const DEBOUNCE_TIMEOUT_MS: u64 = 1000;

pub struct FileWatcher {
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    app_handle: AppHandle,
    event_tx: Option<Sender<Event>>,
}

impl FileWatcher {
    pub fn new(app_handle: AppHandle) -> Self {
        FileWatcher {
            watched_paths: Arc::new(Mutex::new(HashSet::new())),
            app_handle,
            event_tx: None,
        }
    }

    pub async fn start_watching(
        &mut self,
        paths: Vec<FileMetadata>,
        app_handle: AppHandle,
    ) -> AppResult<()> {
        let (tx, rx) = channel(100);
        self.event_tx = Some(tx.clone());

        // Add paths to watched set
        {
            let mut watched_paths = self.watched_paths.lock().unwrap();
            for path in &paths {
                let pathbuf = PathBuf::from(&path.base.path);
                watched_paths.insert(pathbuf.clone());
            }
        }

        // Create a new watcher
        let mut watcher: notify::FsEventWatcher = self.create_watcher(tx)?;

        // Start monitoring each path
        for pathbuf in paths {
            let path = PathBuf::from(pathbuf.base.path);
            if path.exists() {
                watcher.watch(&path, RecursiveMode::Recursive)?;
                info!("Started watching path: {:?}", path);
            } else {
                error!("Path does not exist, cannot watch: {:?}", path);
            }
        }

    // Start the event processor in a separate task
    let app_handle_clone = app_handle.clone();
    let watched_paths = self.watched_paths.clone();

    tokio::spawn(async move {
        // Get the processor state inside the async task
        FileWatcher::process_events(rx, app_handle_clone, watched_paths).await;
    });

    // Store the watcher in app state to keep it alive
    self.app_handle.manage(Arc::new(Mutex::new(watcher)));

    Ok(())
}

    pub async fn stop_watching(&self, path: String) -> AppResult<()> {
        let mut watched_paths = self.watched_paths.lock().unwrap();
        watched_paths.remove(&PathBuf::from(path));
        Ok(())
    }

    fn create_watcher(&self, tx: Sender<Event>) -> AppResult<RecommendedWatcher> {
        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.try_send(event);
                }
            },
            Config::default(),
        )?;

        Ok(watcher)
    }

    async fn process_events(
        mut rx: Receiver<Event>,
        app_handle: AppHandle,
        watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    ) {
        let mut pending_paths: HashSet<PathBuf> = HashSet::new();
        let mut last_event_time = Instant::now();
    
        while let Some(event) = rx.recv().await {
            debug!("Received file event: {:?}", event);
            last_event_time = Instant::now();
    
            // Get the processor state inside the loop
            let processor_state = app_handle.state::<FileProcessorState>();
    
            // Extract paths to process based on event kind
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        if path.is_file() {
                            pending_paths.insert(path);
                        }
                    }
                }

                EventKind::Remove(_) => {
                    if let Ok(guard) = processor_state.0.lock() { // Correct access for std::sync::Mutex
                        // Dereference the MutexGuard to get Option<FileProcessor>
                        if let Some(processor) = &*guard {
                        for path in event.paths {
                            // Handle file deletions by removing from index
                                let db_path = processor.db_path.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = remove_file_from_index(app_handle.clone(),
                                        path.to_string_lossy().to_string(),
                                        db_path,
                                    ).await {
                                        error!("Failed to remove file from index: {:?}", e);
                                    }
                                });
                        }
                    }
                }
                }
                _ => {} // Ignore other event types
            }
    
            // Wait for events to settle before processing
            sleep(Duration::from_millis(DEBOUNCE_TIMEOUT_MS)).await;
    
            // If no new events came in during the debounce period, process the pending files
            if last_event_time.elapsed() >= Duration::from_millis(DEBOUNCE_TIMEOUT_MS) 
                && !pending_paths.is_empty() 
            {
                let paths_to_process: Vec<PathBuf> = pending_paths.drain().collect();
                
                // Convert to strings for processing
                let paths_str: Vec<String> = paths_to_process
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();
                
                // Check if paths are still in watched directories
                let is_watched = {
                    let watched = watched_paths.lock().unwrap();
                    paths_to_process.iter().any(|path| {
                        watched.iter().any(|watched_path| 
                            path.starts_with(watched_path) || 
                            // Also check if we're watching the parent directory
                            if let Some(parent) = path.parent() {
                                watched_path == parent
                            } else {
                                false
                            }
                        )
                    })
                };
                
                if is_watched {
                    info!("Processing changed files: {:?}", paths_str);
                    
                    // Get processor state again inside the condition
                    let processor_state = app_handle.state::<FileProcessorState>();
                    
                    // Only proceed if we can get the processor
                    if let Ok(guard) = processor_state.inner().lock() {
                        if let Some(processor) = guard.clone() {
                            let app_handle_clone = app_handle.clone();
                            let paths_to_process = paths_str.clone();
                            
                            tokio::spawn(async move {
                                let progress_handler = move |status: ProcessingStatus| {
                                    let _ = app_handle_clone.emit("file-indexing-progress", &status);
                                };
                                
                                match processor.process_paths(
                                    paths_to_process,
                                    progress_handler,
                                    app_handle_clone.clone(),
                                ).await {
                                    Ok(_) => info!("Successfully processed file changes"),
                                    Err(e) => error!("Error processing file changes: {:?}", e),
                                }
                            });
                        }
                    }
                }
            }
        }
    }
}

fn is_relevant_file_event(event: &Event, path: &Path) -> bool {
    // Skip temporary files and hidden files
    if let Some(file_name) = path.file_name() {
        let file_name_str = file_name.to_string_lossy();
        if file_name_str.starts_with('.')
            || file_name_str.ends_with('~')
            || file_name_str.starts_with('#')
            || file_name_str.contains(".tmp")
        {
            return false;
        }
    }

    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
            // Only care about real files with valid extensions
            path.is_file() && is_valid_file_extension(path)
        }
        _ => false,
    }
}

async fn remove_file_from_index(
    app_handle: AppHandle,
    file_path: String,     
    db_path: PathBuf,
) -> Result<(), FileProcessorError> {
    let file_path_for_vector_db = file_path.clone();
    let app_handle_ref = app_handle.clone();

    let db_result = task::spawn_blocking(move || -> Result<bool, FileProcessorError> {
        let mut conn = Connection::open(db_path)?;
        let tx = conn.transaction()?;

        let file_id: Option<i64> = tx
                    .query_row(
                        "SELECT id FROM files WHERE path = ?1",
                        [&file_path],
                        |row| row.get(0),
                    )
                    .ok();

        let mut deleted_from_sqlite = false;
        if let Some(id) = file_id {
            tx.execute("DELETE FROM files_fts WHERE rowid = ?1", [id])?;
            let files_deleted_count = tx.execute("DELETE FROM files WHERE id = ?1", [id])?;
            if files_deleted_count > 0 {
                 deleted_from_sqlite = true;
            } 
        } 

        tx.commit()?;
        Ok(deleted_from_sqlite) // Return true if deleted from SQLite, false otherwise
    })
    .await
    .map_err(|e| FileProcessorError::Other(format!("spawn_blocking JoinError: {e}")))?;

    let was_deleted_from_sqlite = db_result?;
    if was_deleted_from_sqlite {
        if let Err(e) =
            VectorDbManager::delete_embedding(&app_handle_ref, &file_path_for_vector_db).await
        {
            return Err(FileProcessorError::Other(format!(
                "Vector DB deletion failed: {}",
                e
            )));
        } 
    } 


    Ok(())
}

// Add these Tauri commands to interact with the watcher

#[tauri::command]
pub async fn start_watching_paths(paths: Vec<String>, app_handle: AppHandle) -> Result<(), String> {
    let processor_state = app_handle.state::<crate::file_processor::FileProcessorState>();
    let processor = {
        let guard = processor_state.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("File processor not initialized")?
            .clone()
    };

    // Get or create the file watcher
    let watcher_state = app_handle.state::<Arc<Mutex<Option<FileWatcher>>>>();
    let mut watcher = {
        let mut guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
        if guard.is_none() {
            *guard = Some(FileWatcher::new(processor, app_handle.clone()));
        }
        guard.as_mut().unwrap()
    };

    // Start watching the paths
    watcher
        .start_watching(paths)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_watching_path(path: String, app_handle: AppHandle) -> Result<(), String> {
    let watcher_state = app_handle.state::<Arc<Mutex<Option<FileWatcher>>>>();
    let watcher = {
        let guard = watcher_state.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("File watcher not initialized")?
            .clone()
    };

    watcher.stop_watching(path).await.map_err(|e| e.to_string())
}

// Add this to your main.rs to initialize the file watcher state
pub fn init_file_watcher(app: &mut tauri::App) -> AppResult<()> {
    app.manage(Arc::new(Mutex::new(Option::<FileWatcher>::None)));
    Ok(())
}
