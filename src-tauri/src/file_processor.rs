use arrow_array::{Array, RecordBatch};
use rusqlite::{params, Connection, Rows};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Semaphore;
use tokio::task;
use tracing::error;
use walkdir::WalkDir;

use crate::tokenizer::{build_doc_text, build_trigrams};

use crate::utils::get_category_from_extension;

use crate::chunker::{ChunkerConfig, ChunkerOrchestrator};

use crate::embedder::Embedder;
use crate::vectordb_manager::VectorDbManager;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSectionType {
    Files,
    Apps,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseMetadata {
    pub id: Option<i64>,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    #[serde(flatten)]
    pub base: BaseMetadata,

    #[serde(rename = "type")]
    pub file_type: SearchSectionType,

    pub extension: String,
    pub size: i64,
    pub updated_at: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppMetadata {
    #[serde(flatten)]
    pub base: BaseMetadata,

    #[serde(rename = "type")]
    pub app_type: SearchSectionType,

    pub is_running: bool,
    pub memory_usage: Option<f64>,
    pub cpu_usage: Option<f64>,
    pub icon_data_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMetadata {
    #[serde(flatten)]
    pub base: BaseMetadata,

    #[serde(rename = "type")]
    pub semantic_type: SearchSectionType,

    pub extension: String,
    pub distance: f64,
    pub content: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingStatus {
    pub total: usize,
    pub processed: usize,
    pub percentage: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum FileProcessorError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Clone)]
pub struct FileProcessor {
    pub db_path: PathBuf, // sqlite db path - TODO: convert to db pool using r2d2_rusqlite
    pub concurrency_limit: usize,
}

impl FileProcessor {
    /// Main async method to process all the given paths:
    /// 1) collect files
    /// 2) spawn tasks with concurrency limit
    /// 3) process files by storing them, creating chunks, embeddings and storing in vectordb
    /// 4) track progress and emit Tauri events
    /// If successful then this function doesn't return anything
    /// If error, then it returns the number of errors, the file path that caused it and the error
    pub async fn process_paths(
        &self,
        paths: Vec<String>,
        on_progress: impl Fn(ProcessingStatus) + Send + Sync + Clone + 'static,
        app_handle: AppHandle,
    ) -> Result<serde_json::Value, FileProcessorError> {
        println!("The paths {:?}", paths);

        // TODO: we might want to  parallelize this and the chunking instead of doing it sequentlly, essentially walk the tree, get to leaves, chunk and embed them and keep going

        // get all file paths that need to be processed
        let files: Vec<FileMetadata> = self.collect_all_files(&paths).await?;
        let total_files: usize = files.len();

        // early return if no files
        if total_files == 0 {
            return Ok(serde_json::json!({
                "success": true,
                "totalFiles": 0,
                "errors": []
            }));
        }

        // create new semaphore to handle concurrency limits
        let sem = Arc::new(Semaphore::new(self.concurrency_limit));

        let num_processed_files = Arc::new(AtomicUsize::new(0));

        // channel to collect errors
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel();

        let mut task_handles = Vec::with_capacity(total_files);

        for file in &files {
            // semaphore is shared but each task needs its own reference for concurrency limit
            let permit = sem.clone();
            // each task needs a reference to the current process files so it can update it
            let pc = num_processed_files.clone();
            // task needs its own channel sender for errors
            let err_sender: UnboundedSender<(String, String)> = err_tx.clone();
            // each task needs a reference to the processor object to call process function
            let this = self.clone();
            // each task needs its own reference to the progress function to update it
            let progress_fn = on_progress.clone();

            let task_handle: task::JoinHandle<()> = create_path_embedding(
                this.db_path,
                file,
                permit,
                err_sender,
                total_files,
                pc,
                progress_fn,
                app_handle.clone(),
            );

            task_handles.push(task_handle);
        }

        // Wait for all tasks and process results
        drop(err_tx);
        futures::future::join_all(task_handles).await;

        // Collect errors with file paths
        let mut detailed_errors = Vec::new();
        while let Ok((file_path, error_msg)) = err_rx.try_recv() {
            detailed_errors.push(serde_json::json!({
                "path": file_path,
                "error": error_msg
            }));
        }

        let success = detailed_errors.is_empty();
        let processed_count = num_processed_files.load(Ordering::SeqCst);

        let result = serde_json::json!({
            "success": success,
            "totalFiles": total_files,
            "processedFiles": processed_count,
            "errors": detailed_errors
        });

        Ok(result)
    }

    /// Given a vector of paths, this walks the tree and collects all children paths
    async fn collect_all_files(
        &self,
        paths: &[String],
    ) -> Result<Vec<FileMetadata>, FileProcessorError> {
        let path_vec: Vec<String> = paths.to_vec();

        task::spawn_blocking(move || {
            let mut all_files: Vec<FileMetadata> = Vec::new();
            for path_str in path_vec {
                let path: &Path = Path::new(&path_str);
                if path.is_dir() {
                    for entry in WalkDir::new(path) {
                        let entry: walkdir::DirEntry = match entry {
                            Ok(e) => e,
                            Err(e) => {
                                eprintln!("Error walking dir: {e}");
                                continue;
                            }
                        };
                        if entry.file_type().is_file() {
                            let _ = get_file_metadata(entry.path(), &mut all_files);
                        }
                    }
                } else {
                    let _ = get_file_metadata(path, &mut all_files);
                }
            }
            Ok::<_, FileProcessorError>(all_files)
        })
        .await
        .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
    }
}

fn create_path_embedding(
    db_path: PathBuf,
    file_metadata: &FileMetadata,
    permit: Arc<Semaphore>,
    err_sender: UnboundedSender<(String, String)>,
    total_files: usize,
    pc: Arc<AtomicUsize>,
    progress_fn: impl Fn(ProcessingStatus) + Send + Sync + Clone + 'static,
    app_handle: AppHandle,
) -> tokio::task::JoinHandle<()> {
    println!("creating embedding for file {:?}", file_metadata.base.path);

    let fm_clone = file_metadata.clone();
    let file_path = fm_clone.base.path.clone();

    tokio::spawn(async move {
        // Acquire concurrency permit
        let _permit = match permit.acquire().await {
            Ok(permit) => permit,
            Err(_) => {
                let _ =
                    err_sender.send((file_path, "Failed to acquire semaphore permit".to_string()));
                return;
            }
        };

        let saved_file_id: String = match save_file_to_db(db_path.clone(), &fm_clone).await {
            Ok(file_id) => file_id,
            Err(e) => {
                let _ =
                    err_sender.send((file_path.clone(), format!("File processing error: {:?}", e)));
                return;
            }
        };

        // Skip empty files
        if fm_clone.size == 0 {
            return;
        }

        let config = ChunkerConfig {
            chunk_size: 100,
            chunk_overlap: 2,
            normalize_text: true,
            extract_metadata: true,
            max_concurrent_files: 4,
            use_gpu_acceleration: true,
        };

        let orchestrator = ChunkerOrchestrator::new(config);

        let embedder_state: State<'_, Arc<Embedder>> = app_handle.state::<Arc<Embedder>>();

        let embedder: Arc<Embedder> = Arc::clone(&embedder_state.inner());

        match orchestrator.chunk_file(&fm_clone, embedder).await {
            Ok(chunk_embeddings) => {
                if chunk_embeddings.is_empty() {
                    let _ =
                        err_sender.send((file_path, "No valid embeddings generated".to_string()));
                } else {
                    println!("the chunk and embedding: {:?}", chunk_embeddings);
                    VectorDbManager::insert_embeddings(
                        &app_handle,
                        &saved_file_id,
                        chunk_embeddings,
                    )
                    .await
                    .unwrap_or_else(|e| {
                        let _ = err_sender.send((
                            file_path.clone(),
                            format!("Failed to insert embeddings: {}", e),
                        ));
                    });
                    // Update progress
                    let processed: usize = pc.fetch_add(1, Ordering::SeqCst) + 1;
                    let percentage: usize =
                        ((processed as f64 / total_files as f64) * 100.0).round() as usize;
                    progress_fn(ProcessingStatus {
                        total: total_files,
                        processed,
                        percentage,
                    });
                }
            }
            Err(e) => {
                let _ = err_sender.send((file_path, format!("Chunking/embedding error: {}", e)));
            }
        }
    })
}

/// Saves a single file to the db and to fts
/// returns the stringified file id on success
async fn save_file_to_db(
    db_path: PathBuf,
    file: &FileMetadata,
) -> Result<String, FileProcessorError> {
    let file = file.clone();

    task::spawn_blocking({
        let db_path = db_path;
        move || -> Result<String, FileProcessorError> {
            // Fixed error handling with map_err instead of map
            let conn = Connection::open(db_path)
            .map_err(|e| FileProcessorError::Db(e))?;

            // Set pragmas for better performance
            conn.execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                "#,
            )?;

            // Insert file metadata
            conn.execute(
                r#"
                INSERT OR IGNORE INTO files (path, name, extension, size, category, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
                "#,
                params![
                    file.base.path,
                    file.base.name,
                    file.extension,
                    file.size,
                    get_category_from_extension(&file.extension)
                ],
            )?;

            // Get the file ID for FTS insertion
            let file_id: i64 = conn.query_row(
                "SELECT id FROM files WHERE path = ?1",
                [file.base.path.clone()],
                |row| row.get(0),
            )?;

            // Build document text from file metadata for search indexing
            let doc_text = build_doc_text(&file.base.name, &file.base.path, &file.extension);

            // Insert into full-text search table
            conn.execute(
                r#"
                INSERT INTO files_fts(rowid, doc_text)
                VALUES (?1, ?2)
                "#,
                params![file_id, doc_text],
            )?;

            Ok(file_id.to_string())
        }
    }).await.map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
}

/// Get metadata for a given file path
pub fn get_file_metadata(
    path: &Path,
    all_files: &mut Vec<FileMetadata>,
) -> Result<(), FileProcessorError> {
    let meta = std::fs::metadata(path)?;
    let size = meta.len() as i64;
    let ext = path
        .extension()
        .map(|os| os.to_string_lossy().into_owned())
        .unwrap_or_default();

    all_files.push(FileMetadata {
        base: BaseMetadata {
            id: None,
            name: path
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| "unknown".into()),
            path: path.to_string_lossy().into_owned(),
        },
        file_type: SearchSectionType::Files,
        extension: ext,
        size,
        updated_at: None,
        created_at: None,
    });

    Ok(())
}

#[derive(Default)]
pub struct FileProcessorState(Mutex<Option<FileProcessor>>);

#[tauri::command]
pub fn initialize_file_processor(
    db_path: String,
    concurrency: usize,
    app_handle: AppHandle,
) -> Result<(), String> {
    let state: State<'_, FileProcessorState> = app_handle.state::<FileProcessorState>();
    let mut processor_guard = state.0.lock().map_err(|e| e.to_string())?;
    *processor_guard = Some(FileProcessor {
        db_path: PathBuf::from(db_path),
        concurrency_limit: concurrency,
    });
    Ok(())
}

#[tauri::command]
pub async fn process_paths_command(
    paths: Vec<String>,
    state: tauri::State<'_, FileProcessorState>,
    app_handle: AppHandle,
) -> Result<serde_json::Value, String> {
    let processor: FileProcessor = {
        let guard: std::sync::MutexGuard<'_, Option<FileProcessor>> =
            state.0.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(p) => p.clone(),
            None => return Err("File processor not initialized".to_string()),
        }
    };

    let app_handle_for_progress = app_handle.clone();

    let progress_handler = move |status: ProcessingStatus| {
        let _ = app_handle_for_progress.emit("file-processing-progress", &status);
    };

    processor
        .process_paths(paths, progress_handler, app_handle)
        .await
        .map_err(|e: FileProcessorError| e.to_string())
}

// #[tauri::command]
// pub async fn get_files_data(
//     query: String,
//     state: State<'_, FileProcessorState>,
//     app_handle: AppHandle,
// ) -> Result<Vec<FileMetadata>, String> {
//     let processor: FileProcessor = {
//         let guard: std::sync::MutexGuard<'_, Option<FileProcessor>> =
//             state.0.lock().map_err(|e| e.to_string())?;
//         guard
//             .as_ref()
//             .ok_or("File processor not initialized".to_string())?
//             .clone()
//     };

//     let conn =
//         Connection::open(processor.db_path).map_err(|e| format!("Failed to open database: {e}"))?;

//     // If user typed nothing, return first 50 files but we can be smarter here and check based on recents
//     if query.trim().is_empty() {
//         let mut stmt = conn
//             .prepare(
//                 r#"
//              SELECT
//               id,
//               name,
//               path,
//               extension,
//               size,
//               created_at,
//               updated_at
//             FROM files
//             LIMIT 50
//         "#,
//             )
//             .map_err(|e| format!("Failed to prepare statement: {e}"))?;

//         let mut rows = stmt.query([]).map_err(|e| format!("Query error: {e}"))?;

//         let mut files = Vec::new();
//         while let Some(row) = rows.next().map_err(|e| format!("Row error: {e}"))? {
//             files.push(FileMetadata {
//                 base: BaseMetadata {
//                     id: Some(row.get(0).map_err(|e| e.to_string())?),
//                     name: row.get(1).map_err(|e| e.to_string())?,
//                     path: row.get(2).map_err(|e| e.to_string())?,
//                 },
//                 file_type: SearchSectionType::Files,
//                 extension: row.get(3).map_err(|e| e.to_string())?,
//                 size: row.get(4).map_err(|e| e.to_string())?,
//                 created_at: row.get(5).ok(),
//                 updated_at: row.get(6).ok(),
//             });
//         }
//         return Ok(files);
//     }

//     if query.len() < 3 {
//         let like_pattern = format!("%{}%", query);

//         let mut stmt = conn
//             .prepare(
//                 r#"
//             SELECT
//               id,
//               name,
//               path,
//               extension,
//               size,
//               created_at,
//               updated_at
//             FROM files
//             WHERE name LIKE ?1 OR path LIKE ?2 OR extension LIKE ?3
//             LIMIT 50
//             "#,
//             )
//             .map_err(|e| format!("Failed to prepare statement: {e}"))?;

//         let mut rows = stmt
//             .query(params![&like_pattern, &like_pattern, &like_pattern])
//             .map_err(|e| format!("Query error: {e}"))?;

//         let mut files = Vec::new();
//         while let Some(row) = rows.next().map_err(|e| format!("Row error: {e}"))? {
//             files.push(FileMetadata {
//                 base: BaseMetadata {
//                     id: Some(row.get(0).map_err(|e| e.to_string())?),
//                     name: row.get(1).map_err(|e| e.to_string())?,
//                     path: row.get(2).map_err(|e| e.to_string())?,
//                 },
//                 file_type: SearchSectionType::Files,
//                 extension: row.get(3).map_err(|e| e.to_string())?,
//                 size: row.get(4).map_err(|e| e.to_string())?,
//                 created_at: row.get(5).ok(),
//                 updated_at: row.get(6).ok(),
//             });
//         }
//         return Ok(files);
//     }

//     let search_trigrams = build_trigrams(&query);
//     println!(
//         "more than 3 in search query, the search trigrams: {}",
//         search_trigrams
//     );

//     // do an FTS search on doc_text
//     let mut stmt = conn
//         .prepare(
//             r#"
//         SELECT
//           f.id,
//           f.name,
//           f.path,
//           f.extension,
//           f.size,
//           f.created_at,
//           f.updated_at
//         FROM files_fts ft
//         JOIN files f ON ft.rowid = f.id
//         WHERE ft.doc_text MATCH ?1
//         LIMIT 50
//         "#,
//         )
//         .map_err(|e| format!("Failed to prepare statement: {e}"))?;

//     // Fix: Use search_trigrams instead of raw query
//     let mut rows = stmt
//         .query([search_trigrams.as_str()])
//         .map_err(|e| format!("Query error: {e}"))?;

//     let mut files = Vec::new();
//     while let Some(row) = rows.next().map_err(|e| format!("Row error: {e}"))? {
//         files.push(FileMetadata {
//             base: BaseMetadata {
//                 id: Some(row.get(0).map_err(|e| e.to_string())?),
//                 name: row.get(1).map_err(|e| e.to_string())?,
//                 path: row.get(2).map_err(|e| e.to_string())?,
//             },
//             file_type: SearchSectionType::Files,
//             extension: row.get(3).map_err(|e| e.to_string())?,
//             size: row.get(4).map_err(|e| e.to_string())?,
//             created_at: row.get(5).ok(),
//             updated_at: row.get(6).ok(),
//         });
//     }

//     // get back file_ids of files that semantically match the query
//     let mut semantic_rows = VectorDbManager::search_similar(&app_handle, &query)
//         .await
//         .map_err(|e| format!("Query error: {e}"))?;

//     // translate the file_ids into FileMetadata to return
//     let mut files = Vec::new();
//     // while let Some(row) = semantic_rows.next().map_err(|e| format!("Row error: {e}"))? {
//     //     files.push(FileMetadata {
//     //         base: BaseMetadata {
//     //             id: Some(row.get(0).map_err(|e| e.to_string())?),
//     //             name: row.get(1).map_err(|e| e.to_string())?,
//     //             path: row.get(2).map_err(|e| e.to_string())?,
//     //         },
//     //         file_type: SearchSectionType::Files,
//     //         extension: row.get(3).map_err(|e| e.to_string())?,
//     //         size: row.get(4).map_err(|e| e.to_string())?,
//     //         created_at: row.get(5).ok(),
//     //         updated_at: row.get(6).ok(),
//     //     });
//     // }

//     Ok(files)
// }

#[tauri::command]
pub async fn get_files_data(
    query: String,
    state: State<'_, FileProcessorState>,
    app_handle: AppHandle,
) -> Result<Vec<FileMetadata>, String> {
    let processor: FileProcessor = get_processor(&state)?;

    let conn: Connection = Connection::open(&processor.db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;

    // Handle empty queries
    if query.trim().is_empty() {
        return get_recent_files(&conn);
    }

    // Handle short queries with LIKE
    if query.len() < 3 {
        return search_files_by_like(&conn, &query);
    }

    // For queries with >3 characters, first do an FTS search
    let files = search_files_by_fts(&conn, &query)?;

    // Get semantic results if we have vector search capability
    let semantic_files: Vec<FileMetadata> =
        match VectorDbManager::search_similar(&app_handle, &query).await {
            Ok(results) => convert_search_results_to_metadata(results, &conn)?,
            Err(e) => {
                // Log the error but continue with just FTS results
                eprintln!(
                    "Semantic search error (continuing with text search only): {}",
                    e
                );
                Vec::new()
            }
        };

    // Combine and deduplicate results
    let combined_files = combine_search_results(files, semantic_files);

    Ok(combined_files)
}

fn get_processor(state: &State<'_, FileProcessorState>) -> Result<FileProcessor, String> {
    let processor: FileProcessor = {
        let guard: std::sync::MutexGuard<'_, Option<FileProcessor>> =
            state.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("File processor not initialized".to_string())?
            .clone()
    };

    Ok(processor)
}

// Get recent files when query is empty
fn get_recent_files(conn: &Connection) -> Result<Vec<FileMetadata>, String> {
    let mut stmt = conn
        .prepare(
            r#"
             SELECT
              id,
              name,
              path,
              extension,
              size,
              created_at,
              updated_at
            FROM files
            ORDER BY updated_at DESC
            LIMIT 50
        "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {e}"))?;

    let rows = stmt.query([]).map_err(|e| format!("Query error: {e}"))?;

    rows_to_file_metadata(rows)
}

// Search files using LIKE for short queries
fn search_files_by_like(conn: &Connection, query: &str) -> Result<Vec<FileMetadata>, String> {
    let like_pattern = format!("%{}%", query);

    let mut stmt = conn
        .prepare(
            r#"
            SELECT
              id,
              name,
              path,
              extension,
              size,
              created_at,
              updated_at
            FROM files
            WHERE name LIKE ?1 OR path LIKE ?2 OR extension LIKE ?3
            LIMIT 50
        "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {e}"))?;

    let rows = stmt
        .query(params![&like_pattern, &like_pattern, &like_pattern])
        .map_err(|e| format!("Query error: {e}"))?;

    rows_to_file_metadata(rows)
}

// Search files using full-text search
fn search_files_by_fts(conn: &Connection, query: &str) -> Result<Vec<FileMetadata>, String> {
    let search_trigrams = build_trigrams(query);

    let mut stmt = conn
        .prepare(
            r#"
        SELECT
          f.id,
          f.name,
          f.path,
          f.extension,
          f.size,
          f.created_at,
          f.updated_at
        FROM files_fts ft
        JOIN files f ON ft.rowid = f.id
        WHERE ft.doc_text MATCH ?1
        LIMIT 50
        "#,
        )
        .map_err(|e| format!("Failed to prepare statement: {e}"))?;

    let rows = stmt
        .query([search_trigrams.as_str()])
        .map_err(|e| format!("Query error: {e}"))?;

    rows_to_file_metadata(rows)
}

// convert sqlite rows to FileMetadata type
fn rows_to_file_metadata(mut rows: Rows) -> Result<Vec<FileMetadata>, String> {
    let mut files: Vec<FileMetadata> = Vec::new();

    while let Some(row) = rows.next().map_err(|e| format!("Row error: {e}"))? {
        files.push(FileMetadata {
            base: BaseMetadata {
                id: Some(row.get(0).map_err(|e| e.to_string())?),
                name: row.get(1).map_err(|e| e.to_string())?,
                path: row.get(2).map_err(|e| e.to_string())?,
            },
            file_type: SearchSectionType::Files,
            extension: row.get(3).map_err(|e| e.to_string())?,
            size: row.get(4).map_err(|e| e.to_string())?,
            created_at: row.get(5).ok(),
            updated_at: row.get(6).ok(),
        });
    }

    Ok(files)
}

// Convert vector search results to FileMetadata
fn convert_search_results_to_metadata(
    results: Vec<RecordBatch>,
    conn: &Connection,
) -> Result<Vec<FileMetadata>, String> {
    // If no results, return empty vector
    if results.is_empty() {
        return Ok(Vec::new());
    }

    // Extract file_ids from results
    let mut file_ids = Vec::new();

    for batch in &results {
        // Get the file_id column (assuming it's at a specific position or name)
        if let Some(column) = batch.column_by_name("file_id") {
            // Convert Arrow array to StringArray
            if let Some(string_array) = column.as_any().downcast_ref::<arrow_array::StringArray>() {
                // Extract each string value
                for i in 0..string_array.len() {
                    let file_id = string_array.value(i);
                    file_ids.push(file_id.to_string());
                }
            }
        }
    }

    // If we couldn't extract any file_ids
    if file_ids.is_empty() {
        return Ok(Vec::new());
    }
    // Build a query to fetch file metadata by ids
    let placeholders = file_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(",");

    let query = format!(
        r#"
        SELECT id, name, path, extension, size, created_at, updated_at
        FROM files
        WHERE id IN ({})
        "#,
        placeholders
    );

    let mut stmt = conn
        .prepare(&query)
        .map_err(|e| format!("Failed to prepare statement: {e}"))?;

    // Convert file_ids to params
    let params: Vec<&dyn rusqlite::ToSql> = file_ids
        .iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();

    let rows = stmt
        .query(params.as_slice())
        .map_err(|e| format!("Query error: {e}"))?;

    rows_to_file_metadata(rows)
}

// Combine and deduplicate search results
fn combine_search_results(
    mut text_results: Vec<FileMetadata>,
    mut semantic_results: Vec<FileMetadata>,
) -> Vec<FileMetadata> {
    // If either result set is empty, return the other
    if text_results.is_empty() {
        return semantic_results;
    }
    if semantic_results.is_empty() {
        return text_results;
    }

    // Use a HashSet to track seen file IDs for deduplication
    let mut seen_ids = HashSet::new();

    // First add all text search results
    let mut combined = Vec::with_capacity(text_results.len() + semantic_results.len());

    for file in text_results.drain(..) {
        if let Some(id) = &file.base.id {
            seen_ids.insert(id.clone());
        }
        combined.push(file);
    }

    // Then add semantic results that aren't duplicates
    for file in semantic_results.drain(..) {
        if let Some(id) = &file.base.id {
            if !seen_ids.contains(id) {
                combined.push(file);
            }
        } else {
            // If no ID (unusual), just add it
            combined.push(file);
        }
    }

    combined
}

#[tauri::command]
pub fn open_file(file_path: &str) -> Result<(), String> {
    let status = Command::new("open")
        .arg(file_path)
        .status()
        .map_err(|e| format!("Failed to open file: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Failed to open file, exit code: {:?}",
            status.code()
        ))
    }
}
