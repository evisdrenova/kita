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
use tracing::{debug, error, info, warn};

const DEBOUNCE_TIMEOUT_MS: u64 = 1000;

pub struct FileWatcher {
    watched_roots: Arc<Mutex<HashSet<PathBuf>>>, // keeps track of the parent directories of the files that we've indexed which is where we listen for files being added/deleted from those parent directories
    // Keep track of the *specific file paths* known to be indexed
    indexed_files: Arc<Mutex<HashSet<PathBuf>>>,
    app_handle: AppHandle,
    watcher: Option<RecommendedWatcher>,
    event_tx: Option<Sender<notify::Result<Event>>>,
}
impl FileWatcher {
    pub fn new(app_handle: AppHandle) -> Self {
        FileWatcher {
            watched_roots: Arc::new(Mutex::new(HashSet::new())),
            indexed_files: Arc::new(Mutex::new(HashSet::new())),
            app_handle,
            watcher: None,
            event_tx: None,
        }
    }

    /// handles adding watchers for the root files and the parent dirctories
    pub fn add_files_and_watch_parents(&mut self, newly_indexed_files: &[String]) -> AppResult<()> {
        if newly_indexed_files.is_empty() {
            info!("No new files provided to add/watch.");
            return Ok(());
        }

        println!(
            "Adding {} newly indexed files and ensuring parents are watched...",
            newly_indexed_files.len()
        );

        // 1. Add newly indexed files to the tracking set (NO CLEARING)
        {
            let mut indexed_files_guard = self.indexed_files.lock().unwrap();
            for path_str in newly_indexed_files {
                indexed_files_guard.insert(PathBuf::from(path_str));
            }
        }

        // 2. Determine unique parent directories from THIS NEW BATCH
        let mut new_root_dirs_to_check = HashSet::new();
        for path_str in newly_indexed_files {
            if let Some(parent) = Path::new(path_str).parent() {
                if parent.is_dir() {
                    // Check if it's actually a directory
                    new_root_dirs_to_check.insert(parent.to_path_buf());
                } else {
                    warn!("Parent of {} is not a directory, cannot watch.", path_str);
                }
            } else {
                // Handle files in root? e.g., C:\file.txt - parent is C:\
                // This case might need specific handling depending on OS and requirements.
                warn!("Could not get parent directory for new file: {}", path_str);
            }
        }
        debug!(
            "Parent directories derived from new batch: {:?}",
            new_root_dirs_to_check
        );

        // 3. Ensure watcher and processing task are started (only if first time)
        if self.watcher.is_none() {
            info!("Initializing file watcher and event processor (first run)...");
            let (tx, rx) = channel(100);
            self.event_tx = Some(tx.clone());

            let watcher_tx = tx.clone();
            let watcher = RecommendedWatcher::new(
                move |res: Result<Event, NotifyError>| {
                    if watcher_tx.try_send(res).is_err() {
                        error!(
                            "Event processing channel error (full or closed). Watcher might stop."
                        );
                    }
                },
                Config::default(),
            )?;
            self.watcher = Some(watcher);

            let app_handle_clone = self.app_handle.clone();
            let indexed_files_clone = self.indexed_files.clone();
            tokio::spawn(async move {
                info!("File event processing task started.");
                FileWatcher::process_events(rx, app_handle_clone, indexed_files_clone).await;
                info!("File event processing task finished.");
            });
        }

        // 4. Watch any NEW parent directories found in this batch if not already covered
        let watcher = self.watcher.as_mut().unwrap();
        let mut watched_roots_guard = self.watched_roots.lock().unwrap();
        let mut actually_watched_new_roots = Vec::new();

        for root_dir in new_root_dirs_to_check {
            // Iterate only parents derived from the NEW batch
            if !root_dir.exists() {
                warn!(
                    "Parent directory {:?} does not exist, cannot watch.",
                    root_dir
                );
                continue;
            }

            // Check if this root_dir or any of its ancestors are already being watched.
            let already_covered = watched_roots_guard.iter().any(|existing_root| {
                root_dir.starts_with(existing_root) // Is it inside or equal to an already watched root?
            });

            if !already_covered {
                // Optional: Check if this new root_dir is an ANCESTOR of any currently watched roots.
                // If so, we could potentially remove the more specific watches and just watch the ancestor.
                // Example: Watching /A/B, now add /A. Best to unwatch /A/B and just watch /A.
                // This adds complexity, skipping for now.

                // Add to our tracked set *before* attempting watch
                // Use insert check to avoid duplicate logging/watching attempts within this loop
                if watched_roots_guard.insert(root_dir.clone()) {
                    match watcher.watch(&root_dir, RecursiveMode::Recursive) {
                        Ok(_) => {
                            info!("Started watching new directory root: {:?}", root_dir);
                            actually_watched_new_roots.push(root_dir);
                        }
                        Err(e) => {
                            error!("Failed to watch new directory {:?}: {}", root_dir, e);
                            watched_roots_guard.remove(&root_dir); // Remove if watch failed
                        }
                    }
                }
            } else {
                debug!(
                    "Parent directory {:?} is already covered by existing recursive watches.",
                    root_dir
                );
            }
        } // End loop through new potential roots

        if !actually_watched_new_roots.is_empty() {
            info!(
                "Watching newly added root directories: {:?}",
                actually_watched_new_roots
            );
        }
        info!(
            "Currently watching root directories: {:?}",
            watched_roots_guard.iter().collect::<Vec<_>>()
        );

        Ok(())
    }

    pub fn stop_watching_root(&mut self, path_str: &str) -> AppResult<()> {
        let path = PathBuf::from(path_str);
        let mut watched_roots_guard = self.watched_roots.lock().unwrap();

        if watched_roots_guard.remove(&path) {
            if let Some(watcher) = self.watcher.as_mut() {
                match watcher.unwatch(&path) {
                    Ok(_) => {
                        info!("Stopped watching root directory: {:?}", path);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to unwatch root directory {:?}: {}", path, e);
                        watched_roots_guard.insert(path); // Add back since unwatch failed
                        Err(e.into())
                    }
                }
            } else {
                warn!("Attempted to stop watching root, but watcher was not initialized.");
                Ok(())
            }
        } else {
            info!("Root directory was not actively being watched: {:?}", path);
            Ok(())
        }
    }

    async fn process_events(
        mut rx: Receiver<notify::Result<Event>>,
        app_handle: AppHandle,
        indexed_files: Arc<Mutex<HashSet<PathBuf>>>, // Shared set of known indexed files
    ) {
        // Separate sets for debouncing different actions
        let mut pending_reindex: HashSet<PathBuf> = HashSet::new(); // Files needing re-indexing (Modify)
        let mut pending_new: HashSet<PathBuf> = HashSet::new(); // New files needing indexing (Create)
        let mut debounce_timer = Option::<tokio::time::Sleep>::None;

        loop {
            select! {
               biased;

               // Timer fires: Process BOTH pending sets
               _ = async { debounce_timer.as_mut().unwrap() }, if debounce_timer.is_some() && (!pending_reindex.is_empty() || !pending_new.is_empty()) => {
                   let paths_to_reindex: Vec<PathBuf> = pending_reindex.drain().collect();
                   let paths_to_index_new: Vec<PathBuf> = pending_new.drain().collect();
                   debounce_timer = None; // Reset timer

                   // Combine lists for processing, maybe tag them later if process_paths differs
                   let mut all_paths_to_process = paths_to_reindex.clone(); // Clone if needed elsewhere
                   all_paths_to_process.extend(paths_to_index_new.clone()); // Clone if needed elsewhere

                   if !all_paths_to_process.is_empty() {
                       info!("Debounce finished. Processing changes/additions for: {:?}", all_paths_to_process);

                       let processor_state_handle = app_handle.state::<FileProcessorState>();
                       let maybe_processor_info = {
                           match processor_state_handle.0.lock() {
                               Ok(guard) => guard.as_ref().map(|p| (p.db_path.clone(), p.concurrency_limit)),
                               Err(e) => { error!("Mutex poisoned (debounce processing): {}", e); None }
                           }
                       };

                       if let Some((db_path, concurrency_limit)) = maybe_processor_info {
                           let app_handle_clone = app_handle.clone();
                           let indexed_files_clone = indexed_files.clone(); // Clone Arc for task

                           tokio::spawn(async move {
                               let processor = FileProcessor { db_path, concurrency_limit };
                               let handle_for_progress = app_handle_clone.clone();
                               let progress_handler = move |status: ProcessingStatus| {
                                   let _ = handle_for_progress.emit("file-indexing-progress", &status);
                               };

                               // Process all paths together
                               let paths_str: Vec<String> = all_paths_to_process.iter().map(|p| p.to_string_lossy().to_string()).collect();

                               let processing_result = processor.process_paths(
                                   paths_str.clone(), // Pass owned strings
                                   progress_handler,
                                   app_handle_clone,
                               ).await;

                               match processing_result {
                                   Ok(_) => {
                                       info!("Successfully processed batch: {:?}", all_paths_to_process);
                                       // Update the central indexed_files list
                                       let mut indexed_files_guard = indexed_files_clone.lock().unwrap();
                                       for path in all_paths_to_process { // Add all successfully processed paths
                                           indexed_files_guard.insert(path);
                                       }
                                       info!("Updated indexed files set. Count: {}", indexed_files_guard.len());
                                   }
                                   Err(e) => {
                                       error!("Error processing batch {:?}: {:?}", all_paths_to_process, e);
                                       // Potentially try to revert adding to indexed_files if needed, complex.
                                   }
                               }
                           });
                       } else {
                           error!("FileProcessor not available (debounce processing).");
                       }
                   }
                } // End timer arm

                // Receive next event from notify watcher
                maybe_event_res = rx.recv() => {
                   match maybe_event_res {
                       Some(Ok(event)) => {
                           debug!("Received notify event: {:?}", event);
                           let mut needs_debounce_reset = false;

                           for path in &event.paths { // Iterate by reference
                               if !is_relevant_file_event(&event, path) {
                                   continue;
                               }

                               let path_clone = path.clone(); // Clone for use in multiple branches
                               let is_indexed = { // Scope the lock
                                   let guard = indexed_files.lock().unwrap();
                                   guard.contains(&path_clone)
                               };

                               match event.kind {
                                   EventKind::Create(_) => {
                                       if !is_indexed {
                                           info!("Detected new relevant file: {:?}", path_clone);
                                           if pending_new.insert(path_clone) { // Add to NEW set
                                               needs_debounce_reset = true;
                                           }
                                       } else {
                                           debug!("Create event for already indexed file (Treating as Modify): {:?}", path_clone);
                                            if pending_reindex.insert(path_clone) { // Add to REINDEX set
                                               needs_debounce_reset = true;
                                           }
                                       }
                                   }
                                   EventKind::Modify(_) => {
                                       if is_indexed {
                                           info!("Detected modification to indexed file: {:?}", path_clone);
                                           if pending_reindex.insert(path_clone) { // Add to REINDEX set
                                               needs_debounce_reset = true;
                                           }
                                       } else {
                                           debug!("Modify event for non-indexed file (Treating as Create): {:?}", path_clone);
                                            // Treat modify on non-indexed as a Create event
                                            if pending_new.insert(path_clone) { // Add to NEW set
                                                needs_debounce_reset = true;
                                            }
                                       }
                                   }
                                   EventKind::Remove(_) => {
                                       if is_indexed {
                                           info!("Detected removal of indexed file: {:?}", path_clone);
                                           // Remove from pending lists if present
                                           pending_reindex.remove(&path_clone);
                                           pending_new.remove(&path_clone);

                                           // --- Trigger Immediate Removal ---
                                           let processor_state_handle = app_handle.state::<FileProcessorState>();
                                           let maybe_db_path = {
                                               match processor_state_handle.0.lock() {
                                                   Ok(guard) => guard.as_ref().map(|p| p.db_path.clone()),
                                                   Err(e) => { error!("Mutex poisoned (remove event): {}", e); None }
                                               }
                                           };
                                           if let Some(db_path) = maybe_db_path {
                                               let handle_clone = app_handle.clone();
                                               let path_string = path_clone.to_string_lossy().to_string();
                                               let indexed_files_clone = indexed_files.clone();
                                               tokio::spawn(async move {
                                                   if let Err(e) = remove_file_from_index(
                                                       handle_clone,
                                                       path_string.clone(),
                                                       db_path,
                                                       indexed_files_clone, // Pass Arc to update set
                                                   ).await {
                                                       error!("Failed removal process for {}: {:?}", path_string, e);
                                                   }
                                               });
                                           } else {
                                                error!("FileProcessor not available (remove event) for {:?}", path_clone);
                                           }
                                           // --- End Immediate Removal ---
                                       } else {
                                           debug!("Ignoring Remove event for non-indexed file: {:?}", path_clone);
                                       }
                                   }
                                   _ => {} // Ignore other kinds
                               } // end match event.kind
                           } // end for path

                           if needs_debounce_reset {
                               debounce_timer = Some(tokio::time::sleep(Duration::from_millis(DEBOUNCE_TIMEOUT_MS)));
                               debug!("Debounce timer reset/started.");
                           }
                       }
                       Some(Err(e)) => { /* ... handle notify::Error ... */ error!("Error receiving file event: {:?}", e); }
                       None => { /* ... handle channel closed ... */ info!("Event channel closed. Exiting event processing task."); break; }
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
    indexed_files: Arc<Mutex<HashSet<PathBuf>>>, // Accept the shared set
) -> Result<(), FileProcessorError> {
    let file_path_buf = PathBuf::from(&file_path); // Keep PathBuf for set operations
    let file_path_clone_log = file_path.clone(); // Clone for logging messages
    let app_handle_ref = app_handle.clone();

    // --- Start SQLite Deletion (Blocking Task) ---
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
            deleted_from_sqlite = files_deleted_count > 0;
        }
        tx.commit()?;
        Ok(deleted_from_sqlite)
    })
    .await
    .map_err(|e| FileProcessorError::Other(format!("spawn_blocking JoinError: {e}")))?;

    let was_deleted_from_sqlite = db_result?;
    // --- End SQLite Deletion ---

    // --- Start Vector DB Deletion (Async) ---
    let mut vector_db_deleted = false;
    if was_deleted_from_sqlite {
        info!(
            "SQLite removal successful for: {}. Attempting vector DB removal...",
            file_path_clone_log
        );
        match VectorDbManager::delete_embedding(&app_handle_ref, &file_path_clone_log).await {
            Ok(_) => {
                info!("Vector DB removal successful for: {}", file_path_clone_log);
                vector_db_deleted = true;
            }
            Err(e) => {
                error!(
                    "Failed vector DB removal for {}: {:?}",
                    file_path_clone_log, e
                );
                // Don't update indexed_files set if vector delete fails? Or log and proceed?
                // Let's proceed to update the set but log the error prominently.
                return Err(FileProcessorError::Other(format!(
                    "Vector DB deletion failed: {}",
                    e
                ))); // Optionally return error
            }
        }
    } else {
        info!(
            "Skipping vector DB removal, file not deleted from SQLite: {}",
            file_path_clone_log
        );
        // If not in SQLite, it shouldn't be in VectorDB either (ideally).
        // We should still remove it from the indexed_files set as it's gone.
        vector_db_deleted = true; // Treat as "successfully removed" from vector perspective if not in SQLite
    }
    // --- End Vector DB Deletion ---

    // --- Update Indexed Files Set ---
    if vector_db_deleted {
        // Only update set if vector deletion succeeded (or wasn't needed)
        let mut guard = indexed_files.lock().unwrap();
        if guard.remove(&file_path_buf) {
            info!(
                "Removed {} from tracked indexed files. Count: {}",
                file_path_clone_log,
                guard.len()
            );
        } else {
            // This might happen if a remove event fires twice quickly
            warn!(
                "Tried to remove {} from indexed files set, but it wasn't present.",
                file_path_clone_log
            );
        }
    }
    // --- End Update Set ---

    Ok(())
}

pub fn init_file_watcher(app: &tauri::App) -> AppResult<()> {
    // Create the initial state value
    let initial_state = Arc::new(Mutex::new(Option::<FileWatcher>::None));
    // Manage the state
    app.manage(initial_state);
    // Explicitly return Ok
    Ok(())
}
