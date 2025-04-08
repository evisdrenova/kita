use crate::AppResult;
use futures::stream::StreamExt;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::time::sleep;
use tracing::{debug, error, info};

use crate::file_processor::{
    get_file_metadata, is_valid_file_extension, FileMetadata, FileProcessor, FileProcessorError,
    ProcessingStatus,
};

const DEBOUNCE_TIMEOUT_MS: u64 = 1000;

pub struct FileWatcher {
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    processor: Arc<FileProcessor>,
    app_handle: AppHandle,
    event_tx: Option<Sender<Event>>,
}

impl FileWatcher {
    pub fn new(processor: FileProcessor, app_handle: AppHandle) -> Self {
        FileWatcher {
            watched_paths: Arc::new(Mutex::new(HashSet::new())),
            processor: Arc::new(processor),
            app_handle,
            event_tx: None,
        }
    }

    pub async fn start_watching(&mut self, paths: Vec<String>) -> AppResult<()> {
        let (tx, rx) = channel(100);
        self.event_tx = Some(tx.clone());

        // Add paths to watched set
        {
            let mut watched_paths = self.watched_paths.lock().unwrap();
            for path_str in &paths {
                let path = PathBuf::from(path_str);
                watched_paths.insert(path.clone());
            }
        }

        // Create a new watcher
        let watcher = self.create_watcher(tx)?;

        // Start monitoring each path
        for path_str in paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                watcher.watch(&path, RecursiveMode::Recursive)?;
                info!("Started watching path: {:?}", path);
            } else {
                error!("Path does not exist, cannot watch: {:?}", path);
            }
        }

        // Start the event processor in a separate task
        let processor = self.processor.clone();
        let app_handle = self.app_handle.clone();
        let watched_paths = self.watched_paths.clone();

        tokio::spawn(async move {
            Self::process_events(rx, processor, app_handle, watched_paths).await;
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
                    // Filter out events for files we don't care about
                    if let Some(path) = event.paths.first() {
                        if is_relevant_file_event(&event, path) {
                            let _ = tx.try_send(event);
                        }
                    }
                }
            },
            Config::default(),
        )?;

        Ok(watcher)
    }
    async fn process_events(
        mut rx: Receiver<Event>,
        processor: Arc<FileProcessor>,
        app_handle: AppHandle,
        watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    ) {
        let mut pending_paths: HashSet<PathBuf> = HashSet::new();
        let mut last_event_time = Instant::now();

        while let Some(event) = rx.recv().await {
            debug!("Received file event: {:?}", event);
            last_event_time = Instant::now();

            // Extract paths to process based on event kind
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        // Only process files, not directories
                        if path.is_file() && is_valid_file_extension(&path) {
                            pending_paths.insert(path);
                        }
                    }
                }
                EventKind::Remove(_) => {
                    for path in event.paths {
                        // Handle file deletions by removing from index
                        if is_valid_file_extension(&path) {
                            tokio::spawn(remove_file_from_index(
                                path.to_string_lossy().to_string(),
                                processor.db_path.clone(),
                            ));
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
                        watched.iter().any(|watched_path| {
                            path.starts_with(watched_path) || watched_path.starts_with(path)
                        })
                    })
                };

                if is_watched {
                    info!("Processing changed files: {:?}", paths_str);

                    let app_handle_clone = app_handle.clone();
                    let processor_clone = processor.clone();

                    tokio::spawn(async move {
                        let progress_handler = move |status: ProcessingStatus| {
                            let _ = app_handle_clone.emit("file-indexing-progress", &status);
                        };

                        match processor_clone
                            .process_paths(paths_str, progress_handler, app_handle_clone.clone())
                            .await
                        {
                            Ok(_) => info!("Successfully processed file changes"),
                            Err(e) => error!("Error processing file changes: {:?}", e),
                        }
                    });
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
    file_path: String,
    db_path: PathBuf,
) -> Result<(), FileProcessorError> {
    use rusqlite::Connection;
    use tokio::task;

    task::spawn_blocking(move || -> Result<(), FileProcessorError> {
        let conn = Connection::open(db_path)?;

        // Begin transaction for atomicity
        let tx = conn.transaction()?;

        // Get file ID
        let file_id: Option<i64> = tx
            .query_row(
                "SELECT id FROM files WHERE path = ?1",
                [&file_path],
                |row| row.get(0),
            )
            .ok();

        if let Some(id) = file_id {
            // Remove from FTS index
            tx.execute("DELETE FROM files_fts WHERE rowid = ?1", [id])?;

            // Remove from files table
            tx.execute("DELETE FROM files WHERE id = ?1", [id])?;

            // In your actual implementation, you would also need to:
            // 1. Remove from vector database (using VectorDbManager)
            // 2. Remove any chunks associated with this file

            info!("Removed deleted file from index: {}", file_path);
        }

        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
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
