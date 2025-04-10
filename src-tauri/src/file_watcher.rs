use crate::file_processor::{
    is_valid_file_extension, FileProcessor, FileProcessorError, FileProcessorState,
    ProcessingStatus,
};
use crate::vectordb_manager::VectorDbManager;
use crate::AppResult;
use notify::{
    Config, Error as NotifyError, Event as NotifyEvent, EventKind, RecommendedWatcher,
    RecursiveMode, Watcher,
};
use rusqlite::Connection;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Listener, Manager, State};
use tokio::select;
use tokio::sync::mpsc::Receiver;
use tokio::task;
use tracing::error;

const DEBOUNCE_TIMEOUT_MS: u64 = 1000;

#[derive(Debug, Default, Clone)]
pub struct WatcherState {
    pub watched_roots: HashSet<PathBuf>,
    pub indexed_files: HashSet<PathBuf>,
}

pub fn init_file_watcher(app: &tauri::App) -> AppResult<()> {
    let initial_state = Arc::new(Mutex::new(Option::<WatcherState>::None));
    app.manage(initial_state);
    Ok(())
}

pub fn start_watcher_service(app_handle: AppHandle) -> AppResult<()> {
    println!("Starting File Watcher Service...");

    let (fs_event_tx, fs_event_rx) = tokio::sync::mpsc::channel(100); // filesystem events

    // create the notify watcher
    let watcher_tx = fs_event_tx.clone();
    let watcher = RecommendedWatcher::new(
        move |res: Result<NotifyEvent, NotifyError>| {
            if watcher_tx.try_send(res).is_err() {
                error!("FS Event processing channel error (full or closed). Watcher might stop.");
            }
        },
        Config::default(),
    )?;

    // store the watcher itself in Tauri state to keep it alive and manage the watcher instance separately from the WatcherState data.
    app_handle.manage(Arc::new(std::sync::Mutex::new(watcher)));

    // channel for app events like indexing complete so watcher can work
    let (app_event_tx, app_event_rx) = tokio::sync::mpsc::channel::<Vec<String>>(5); // Channel for Vec<String> payloads

    // Listen for Tauri "indexing_complete" events and forward them to app_event_tx form file_processor so that the watcher can start watching on a given root folder
    let app_event_tx_clone = app_event_tx.clone();
    app_handle.listen("indexing_complete", move |event| {
        println!("Received 'indexing_complete' Tauri event.");
        let paths_str = event.payload();
        match serde_json::from_str::<Vec<String>>(paths_str) {
            Ok(paths) => {
                println!("Forwarding {} indexed paths to watcher task.", paths.len());
                // Use try_send or blocking_send depending on requirements
                if let Err(e) = app_event_tx_clone.try_send(paths) {
                    error!(
                        "Failed to send indexing_complete payload to watcher task: {}",
                        e
                    );
                }
            }
            Err(e) => error!("Failed to parse indexing_complete payload: {}", e),
        }
    });

    // Spawn the main event processing loop
    let watcher_state_arc = app_handle
        .state::<Arc<Mutex<WatcherState>>>()
        .inner()
        .clone();

    let app_handle_clone_processor = app_handle.clone();
    tokio::spawn(async move {
        println!("Watcher event processing task started.");
        process_combined_events(
            fs_event_rx,
            app_event_rx,
            app_handle_clone_processor,
            watcher_state_arc,
        )
        .await;
        println!("Watcher event processing task finished.");
    });

    println!("File Watcher Service started.");
    Ok(())
}

async fn process_combined_events(
    mut fs_event_rx: Receiver<notify::Result<NotifyEvent>>, // Filesystem events
    mut app_event_rx: Receiver<Vec<String>>,                // App events ("indexing_complete")
    app_handle: AppHandle,
    watcher_state: Arc<Mutex<WatcherState>>, // State with indexed_files and watched_roots
) {
    let mut pending_reindex: HashSet<PathBuf> = HashSet::new();
    let mut pending_new: HashSet<PathBuf> = HashSet::new();
    let mut debounce_timer = Option::<tokio::time::Sleep>::None;

    let watcher_state_arc = watcher_state; // Use the passed Arc directly
                                           // Handle for the RecommendedWatcher itself (managed separately)
    let watcher_instance_arc_state: State<'_, Arc<std::sync::Mutex<RecommendedWatcher>>> =
        app_handle.state();
    // ---

    loop {
        select! {
            biased;

            // Timer fires: Process debounced Create/Modify
             _ = async { debounce_timer.as_mut().unwrap() }, if debounce_timer.is_some() && (!pending_reindex.is_empty() || !pending_new.is_empty()) => {

                let paths_to_reindex: Vec<PathBuf> = pending_reindex.drain().collect();
                let paths_to_index_new: Vec<PathBuf> = pending_new.drain().collect();
                debounce_timer = None;

                let mut all_paths_to_process = paths_to_reindex; // No clone needed if drained
                all_paths_to_process.extend(paths_to_index_new);

                if !all_paths_to_process.is_empty() {
                    println!("Debounce finished. Processing changes/additions for: {:?}", all_paths_to_process);

                     let processor_state_handle = app_handle.state::<FileProcessorState>();
                     let maybe_processor_info = {
                           match processor_state_handle.0.lock() {
                               Ok(guard) => guard.as_ref().map(|p| (p.db_path.clone(), p.concurrency_limit)),
                               Err(e) => { error!("Mutex poisoned (debounce processing): {}", e); None }
                           }
                       };
                     if let Some((db_path, concurrency_limit)) = maybe_processor_info {
                        let app_handle_clone = app_handle.clone();
                        let watcher_state_clone = watcher_state_arc.clone(); // Use the Arc obtained before loop


                         tokio::spawn(async move {
                             // ... setup processor, progress_handler ...
                             let processor = FileProcessor { db_path, concurrency_limit }; // Temp instance ok here
                             let handle_for_progress = app_handle_clone.clone();
                             let progress_handler = move |status: ProcessingStatus| { /* ... emit ... */ };
                             let paths_str: Vec<String> = all_paths_to_process.iter().map(|p| p.to_string_lossy().to_string()).collect();

                             let processing_result = processor.process_paths(
                                 paths_str.clone(),
                                 progress_handler,
                                 app_handle_clone,
                             ).await;

                             // Update watcher_state.indexed_files after processing
                              match processing_result {
                                  Ok(_) => {
                                    println!("Successfully processed batch: {:?}", all_paths_to_process);
                                      let mut state_guard = watcher_state_clone.lock().unwrap();
                                      for path in all_paths_to_process {
                                          state_guard.indexed_files.insert(path); // Add/Update
                                      }
                                      println!("Updated indexed files set. Count: {}", state_guard.indexed_files.len());
                                  }
                                  Err(e) => error!("Error processing batch {:?}: {:?}", all_paths_to_process, e),
                              }
                         });
                     } else { error!("FileProcessor not available (debounce processing)."); }
                 }
            } // End timer arm


            // Receive filesystem event
             maybe_fs_event_res = fs_event_rx.recv() => {
                match maybe_fs_event_res {
                    Some(Ok(event)) => {
                        println!("Received FS event: {:?}", event);
                        let mut needs_debounce_reset = false;
                        // --- Logic to check event vs watcher_state.indexed_files ---
                        // --- and add to pending_new/pending_reindex or trigger remove ---
                        for path in &event.paths {
                            if !is_relevant_file_event(&event, path) { continue; }

                            let path_clone = path.clone();
                            let is_indexed = { // Block 1
                                let state_clone = watcher_state_arc.clone(); // Clone the Arc here
                                let guard = state_clone.lock().unwrap(); // Lock the clone
                                guard.indexed_files.contains(&path_clone)
                            }; // state_clone (Arc reference) is dropped here

                             match event.kind {
                                 EventKind::Create(_) => {
                                     if !is_indexed {
                                         if pending_new.insert(path_clone) { needs_debounce_reset = true; }
                                     } else {
                                         if pending_reindex.insert(path_clone) { needs_debounce_reset = true; }
                                     }
                                 },
                                 EventKind::Modify(_) => {
                                     if is_indexed {
                                         if pending_reindex.insert(path_clone) { needs_debounce_reset = true; }
                                     } else {
                                         if pending_new.insert(path_clone) { needs_debounce_reset = true; }
                                     }
                                 },
                                 EventKind::Remove(_) => {
                                     if is_indexed {
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
                                               let watcher_state_clone = watcher_state_arc.clone(); // Pass state Arc

                                               tokio::spawn(async move {
                                                   if let Err(e) = remove_file_from_index(
                                                        path_string.clone(), db_path, watcher_state_clone, // Pass state
                                                   ).await {
                                                        error!("Failed removal process for {}: {:?}", path_string, e);
                                                   }
                                               });
                                          } else { error!("FileProcessor not available (remove event)"); }
                                     }
                                 },
                                 _ => {}
                             } // end match event.kind
                         } // end for path

                         if needs_debounce_reset {
                             debounce_timer = Some(tokio::time::sleep(Duration::from_millis(DEBOUNCE_TIMEOUT_MS)));
                         }
                    },
                    Some(Err(e)) => error!("Error receiving FS event: {:?}", e),
                    None => { println!("FS Event channel closed."); break; } // Filesystem watcher stopped
                }
            } // End fs_event_rx arm


            // Receive application event ("indexing_complete")
            maybe_app_event = app_event_rx.recv() => {
                 if let Some(newly_indexed_paths) = maybe_app_event {
                    println!("Received indexing_complete event with {} paths.", newly_indexed_paths.len());
                     // Update internal state and potentially add new directory watches

                     // This logic needs mutable access to the watcher AND the state, tricky inside async task.
                     // Better: Send a message *back* to main thread or use dedicated actor?
                     // Simpler: Directly lock and modify here, carefully.
                     if let Ok(mut watcher_guard) = watcher_instance_arc_state.lock() {
                        let watcher = &mut *watcher_guard; // Access the watcher

                        // Lock the state containing watched_roots and indexed_files
                        let mut state_guard = watcher_state_arc.lock().unwrap();


                                let mut new_roots_to_check = HashSet::new();
                                for path_str in &newly_indexed_paths {
                                    // Add to indexed files set
                                    state_guard.indexed_files.insert(PathBuf::from(path_str));
                                    // Derive parent
                                     if let Some(parent) = Path::new(path_str).parent() {
                                         if parent.is_dir() { new_roots_to_check.insert(parent.to_path_buf()); }
                                     }
                                }
                                println!("Total tracked indexed files now: {}", state_guard.indexed_files.len());

                                // Add watches for new parent directories
                                for root_dir in new_roots_to_check {
                                    if !root_dir.exists() { continue; }
                                    let already_covered = state_guard.watched_roots.iter().any(|r| root_dir.starts_with(r));
                                    if !already_covered {
                                        if state_guard.watched_roots.insert(root_dir.clone()) {
                                             match watcher.watch(&root_dir, RecursiveMode::Recursive) {
                                                 Ok(_) => println!("Started watching new directory root: {:?}", root_dir),
                                                 Err(e) => {
                                                     error!("Failed to watch new directory {:?}: {}", root_dir, e);
                                                     state_guard.watched_roots.remove(&root_dir);
                                                 }
                                             }
                                        }
                                    }
                                }
                                println!("Currently watching roots: {:?}", state_guard.watched_roots.iter().collect::<Vec<_>>());
                     } else { error!("Watcher Arc Mutex poisoned during indexing_complete handling."); }

                 } else {
                    println!("App event channel closed."); // Should not happen if listener is alive
                 }
             } // End app_event_rx arm

        } // end select!
    } // end loop
} // end process_combined_events

async fn remove_file_from_index(
    file_path: String,
    db_path: PathBuf,
    watcher_state: Arc<Mutex<WatcherState>>,
) -> Result<(), FileProcessorError> {
    let file_path_buf = PathBuf::from(&file_path);
    let file_path_clone_log = file_path.clone();

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
    let mut vector_db_deleted_or_not_needed = false;
    if was_deleted_from_sqlite {
        /* ... call VectorDbManager::delete_embedding ... */
        vector_db_deleted_or_not_needed = true;
    } else {
        vector_db_deleted_or_not_needed = true;
    } // VectorDB part

    if vector_db_deleted_or_not_needed {
        let mut state_guard = watcher_state.lock().unwrap(); // Lock the main state
        if state_guard.indexed_files.remove(&file_path_buf) {
            // Modify indexed_files within WatcherState
            println!(
                "Removed {} from tracked indexed files. Count: {}",
                file_path_clone_log,
                state_guard.indexed_files.len()
            );
            // Optional: Check if removed file's parent directory has any other indexed files.
            // If not, maybe stop watching the parent? Adds complexity.
        } else {
            println!(
                "Tried to remove {} from indexed files set, but it wasn't present.",
                file_path_clone_log
            );
        }
    }
    Ok(())
}

fn is_relevant_file_event(event: &NotifyEvent, path: &Path) -> bool {
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
