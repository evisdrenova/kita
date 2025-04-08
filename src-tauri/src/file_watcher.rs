use crate::file_processor::{
    is_valid_file_extension, FileProcessor, FileProcessorError, FileProcessorState,
    ProcessingStatus,
};
use crate::vectordb_manager::VectorDbManager;
use crate::AppResult;
use notify::{
    Config, Error as NotifyError, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::select;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::task;
use tracing::{debug, error, info};
const DEBOUNCE_TIMEOUT_MS: u64 = 1000;

pub struct FileWatcher {
    watched_paths: Arc<Mutex<HashSet<PathBuf>>>,
    app_handle: AppHandle,
    watcher: Option<RecommendedWatcher>, // Watcher instance kept alive here
    event_tx: Option<Sender<notify::Result<Event>>>, // Sender for internal channel
    is_processing_active: bool,          // Flag to prevent multiple process_events tasks
}
impl FileWatcher {
    pub fn new(app_handle: AppHandle) -> Self {
        FileWatcher {
            watched_paths: Arc::new(Mutex::new(HashSet::new())),
            app_handle,
            watcher: None,
            event_tx: None,
            is_processing_active: false,
        }
    }

    pub fn start_or_add_paths(&mut self, paths_to_watch: Vec<&String>) -> AppResult<()> {
        let mut new_paths_added = Vec::new();

        // Ensure watcher and processing task are started only once
        if self.watcher.is_none() {
            info!("Initializing file watcher and event processor...");
            let (tx, rx) = channel(100); // Channel for notify events
            self.event_tx = Some(tx.clone());

            let watcher_tx = tx.clone();
            let watcher = RecommendedWatcher::new(
                move |res: Result<Event, NotifyError>| {
                    // Forward result (including errors) to the channel
                    if let Err(e) = watcher_tx.try_send(res) {
                        // This might happen if the channel is full or closed
                        error!("Failed to send file event to processing channel: {}", e);
                    }
                },
                Config::default(),
            )?;
            self.watcher = Some(watcher);

            // Start the event processor task
            let app_handle_clone = self.app_handle.clone();
            tokio::spawn(async move {
                info!("File event processing task started.");
                FileWatcher::process_events(rx, app_handle_clone).await;
                info!("File event processing task finished.");
            });
            self.is_processing_active = true;
        }

        // Add new paths to watch
        let watcher = self.watcher.as_mut().unwrap(); // Safe now because we ensure it's Some
        let mut watched_paths_guard = self.watched_paths.lock().unwrap();

        for path_str in paths_to_watch {
            let path = PathBuf::from(path_str);
            // Only add if not already watched and watch succeeds
            if watched_paths_guard.insert(path.clone()) {
                match watcher.watch(&path, RecursiveMode::Recursive) {
                    Ok(_) => {
                        info!("Started watching path: {:?}", path);
                        new_paths_added.push(path);
                    }
                    Err(e) => {
                        error!("Failed to watch path {:?}: {}", path, e);
                        // Remove from hashset if watch failed
                        watched_paths_guard.remove(&path);
                    }
                }
            }
        }

        // Potentially trigger an initial scan for newly added paths
        if !new_paths_added.is_empty() {
            info!(
                "Consider triggering an initial scan for newly added paths: {:?}",
                new_paths_added
            );
            // You might want to emit an event or call a function here
            // to process the contents of these newly added directories.
            // E.g., self.trigger_scan(new_paths_added);
        }

        Ok(())
    }

    pub fn stop_watching(&mut self, path_str: &str) -> AppResult<()> {
        let path = PathBuf::from(path_str);
        let mut watched_paths_guard = self.watched_paths.lock().unwrap();

        if watched_paths_guard.remove(&path) {
            if let Some(watcher) = self.watcher.as_mut() {
                match watcher.unwatch(&path) {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        error!("Failed to unwatch path {:?}: {}", path, e);
                        // Add it back to the set? Or just log?
                        watched_paths_guard.insert(path); // Add back since unwatch failed
                        Err(e.into()) // Convert notify::Error to AppResult or your error type
                    }
                }
            } else {
                Ok(()) // Not an error if watcher isn't running
            }
        } else {
            Ok(()) // Not an error if path wasn't in the set
        }
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
        mut rx: Receiver<notify::Result<Event>>, // Receives Results now
        app_handle: AppHandle,
    ) {
        let mut pending_create_modify: HashSet<PathBuf> = HashSet::new();
        let mut debounce_timer = Option::<tokio::time::Sleep>::None; // Use tokio's sleep timer

        loop {
            select! {
               // Biased select prefers receiving events over timer firing
               biased;

               // Option 1: Timer fires and we have pending paths
               _ = async { debounce_timer.as_mut().unwrap() }, if debounce_timer.is_some() && !pending_create_modify.is_empty() => {
                   let paths_to_process: Vec<PathBuf> = pending_create_modify.drain().collect();
                   debounce_timer = None; // Reset timer

                   info!("Debounce finished. Processing changes for: {:?}", paths_to_process);

                   // --- Get necessary info from FileProcessorState (read-only access) ---
                   let processor_state_handle = app_handle.state::<FileProcessorState>(); // Get handle outside the block
                   let maybe_processor_info = { // Scope the lock guard
                       match processor_state_handle.0.lock() { // Use the handle here
                           Ok(guard) => {
                               // guard implicitly borrows processor_state_handle
                               guard.as_ref().map(|p| (p.db_path.clone(), p.concurrency_limit))
                           } // guard is dropped here, borrow ends
                           Err(e) => {
                               error!("Mutex poisoned accessing FileProcessorState: {}", e);
                               None
                           }
                       }
                   }; // temporary Result from match is dropped, maybe_processor_info holds the Option
                    // --- Spawn processing task ---
                   if let Some((db_path, concurrency_limit)) = maybe_processor_info {
                        let app_handle_clone = app_handle.clone(); // Clone for the task

                        tokio::spawn(async move {
                            let processor = FileProcessor { db_path, concurrency_limit };

                            // Clone the handle *before* the move closure definition
                            let handle_for_progress = app_handle_clone.clone();
                            // app_handle_clone (the original passed into spawn) is still available to be passed below

                            let progress_handler = move |status: ProcessingStatus| {
                                // Closure captures and owns handle_for_progress
                                let _ = handle_for_progress.emit("file-indexing-progress", &status);
                            };


                            let paths_str: Vec<String> = paths_to_process
                                .iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect();

                            match processor.process_paths(
                                paths_str,
                                progress_handler, // Moves the closure (and handle_for_progress)
                                app_handle_clone, // Pass the original handle (now owned by the task)
                            ).await {
                                Ok(_) => info!("Successfully processed file changes."),
                                Err(e) => error!("Error processing file changes: {:?}", e),
                            }
                        });
                    } else {
                         error!("FileProcessor not available for processing debounced events.");
                    }
                }

                // Option 2: Receive next event from notify watcher
                maybe_event_res = rx.recv() => {
                    match maybe_event_res {
                        Some(Ok(event)) => { // Handle Ok(Event)
                            debug!("Received file event: {:?}", event);
                            let mut needs_debounce_reset = false;

                            match event.kind {
                                EventKind::Create(_) | EventKind::Modify(_) => {
                                    // Iterate by reference
                                    for path in &event.paths {
                                        // Pass &PathBuf to is_relevant_file_event
                                        if is_relevant_file_event(&event, path) {
                                            // .insert() needs an owned PathBuf, so clone path
                                            if pending_create_modify.insert(path.clone()) {
                                                 needs_debounce_reset = true;
                                            }
                                        }
                                    }
                                }
                                EventKind::Remove(_) => {
                                     // Iterate by reference
                                     for path in &event.paths {
                                         // Pass &PathBuf to is_relevant_file_event
                                         if is_relevant_file_event(&event, path) {
                                             info!("Processing remove event for: {:?}", path);
                                             let processor_state = app_handle.state::<FileProcessorState>();
                                             let maybe_db_path = { // Scope lock
                                                 match processor_state.0.lock() {
                                                      Ok(guard) => guard.as_ref().map(|p| p.db_path.clone()),
                                                      Err(e) => {
                                                           error!("Mutex poisoned accessing FileProcessorState for remove: {}", e);
                                                           None
                                                      }
                                                 }
                                             };

                                             if let Some(db_path) = maybe_db_path {
                                                  let app_handle_clone = app_handle.clone();
                                                  // Convert &PathBuf to owned String for the task
                                                  let path_string = path.to_string_lossy().to_string();
                                                  tokio::spawn(async move {
                                                      if let Err(e) = remove_file_from_index(
                                                          app_handle_clone,
                                                          path_string, // Pass owned String
                                                          db_path,
                                                      ).await {
                                                          // Use {:?} for PathBuf if needed, but path_string is now String
                                                          error!("Failed to remove file from index: {:?}", e);
                                                      }
                                                  });
                                             } else {
                                                  error!("FileProcessor not available for processing remove event for {:?}", path);
                                             }
                                         }
                                     }
                                }
                                _ => {} // Ignore other kinds like Access, Other, Any
                            }

                            // If relevant create/modify events occurred, reset the timer
                            if needs_debounce_reset {
                                debounce_timer = Some(tokio::time::sleep(Duration::from_millis(DEBOUNCE_TIMEOUT_MS)));
                                debug!("Debounce timer reset.");
                            }
                       }
                       Some(Err(e)) => { // Handle notify::Error
                            error!("Error receiving file event: {:?}", e);
                            // Decide if error is fatal (e.g., watcher stopped)
                            // Maybe break the loop or log and continue
                            if matches!(e.kind, notify::ErrorKind::PathNotFound | notify::ErrorKind::WatchNotFound) {
                            }
                       }
                       None => {
                            info!("Event channel closed. Exiting event processing task.");
                            break; // Channel closed, exit loop
                       }
                   } // end match maybe_event_res
                } // end recv arm
            } // end select!
        } // end loop
    } // end process_events
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

// Add this to your main.rs to initialize the file watcher state
pub fn init_file_watcher_state(
    app_builder: tauri::Builder<tauri::Wry>,
) -> tauri::Builder<tauri::Wry> {
    app_builder.manage(Arc::new(Mutex::new(Option::<FileWatcher>::None)))
}
