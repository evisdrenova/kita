use crate::AppResult;
use arrow_array::{Array, RecordBatch};
use rusqlite::{params, Connection, Rows};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Error, ErrorKind};
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

use crate::chunker::{ChunkerConfig, ChunkerOrchestrator};
use crate::embedder::Embedder;
use crate::tokenizer::{build_doc_text, build_trigrams};
use crate::utils::get_category_from_extension;
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
    pub size: i64,
    pub extension: String,
    pub distance: f32,
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
    pub db_path: PathBuf,
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
        println!("Processing paths: {:?}", paths);

        // Get all file paths and directories that need to be processed
        let (files, unique_directories) = self.collect_all_files(&paths).await?;
        let total_files: usize = files.len();
        let total_directories: usize = unique_directories.len();

        println!(
            "Found {} files and {} unique directories",
            total_files, total_directories
        );

        // Early return if no files
        if total_files == 0 {
            return Ok(serde_json::json!({
                "success": true,
                "totalFiles": 0,
                "errors": []
            }));
        }

        // First, save all directories to the database (as a batch for efficiency)
        if !unique_directories.is_empty() {
            println!(
                "Saving {} directories to database",
                unique_directories.len()
            );
            if let Err(e) = save_directories_to_db(self.db_path.clone(), &unique_directories).await
            {
                return Err(FileProcessorError::Other(format!(
                    "Failed to save directories: {}",
                    e
                )));
            }
        }

        // Create new semaphore to handle concurrency limits
        let sem = Arc::new(Semaphore::new(self.concurrency_limit));
        let num_processed_files = Arc::new(AtomicUsize::new(0));

        // Channel to collect errors
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut task_handles = Vec::with_capacity(total_files);

        // Now process files with concurrency
        for file in &files {
            // Semaphore is shared but each task needs its own reference for concurrency limit
            let permit = sem.clone();
            // Each task needs a reference to the current process files so it can update it
            let pc = num_processed_files.clone();
            // Task needs its own channel sender for errors
            let err_sender: UnboundedSender<(String, String)> = err_tx.clone();
            // Each task needs a reference to the processor object to call process function
            let this = self.clone();
            // Each task needs its own reference to the progress function to update it
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

        // When process is complete, emit an event with the paths to watch
        if success {
            println!("successfully processed all files during index");
            // Convert the directory paths to strings for the event payload
            let dir_paths: Vec<String> = unique_directories
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect();

            // Emit the indexing_complete event with directory paths
            // Don't serialize the vector again - Tauri will handle that
            if let Err(e) = app_handle.emit("indexing_complete", &dir_paths) {
                println!("Warning: Failed to emit indexing_complete event: {}", e);
            } else {
                println!(
                    "Successfully emitted indexing_complete event with {} paths",
                    dir_paths.len()
                );
            }

            println!("successfully emitted indexing_complete event");
        }

        let result = serde_json::json!({
            "success": success,
            "totalFiles": total_files,
            "processedFiles": processed_count,
            "totalDirectories": total_directories,
            "errors": detailed_errors
        });

        Ok(result)
    }

    /// Given a vector of paths, this walks the tree and collects all children paths and their parent directories
    async fn collect_all_files(
        &self,
        paths: &[String],
    ) -> Result<(Vec<FileMetadata>, HashSet<PathBuf>), FileProcessorError> {
        let path_vec: Vec<String> = paths.to_vec();

        task::spawn_blocking(move || {
            let mut all_files: Vec<FileMetadata> = Vec::new();
            let mut unique_directories: HashSet<PathBuf> = HashSet::new();

            for path_str in path_vec {
                let path: &Path = Path::new(&path_str);
                if path.is_dir() {
                    // Add the root directory itself
                    unique_directories.insert(PathBuf::from(path));

                    for entry in WalkDir::new(path) {
                        let entry: walkdir::DirEntry = match entry {
                            Ok(e) => e,
                            Err(e) => {
                                eprintln!("Error walking dir: {e}");
                                continue;
                            }
                        };

                        // Skip hidden files
                        if let Some(file_name) = entry.file_name().to_str() {
                            if file_name.starts_with(".") {
                                continue;
                            }
                        }

                        if entry.file_type().is_file() {
                            // Check if the file has a valid extension before processing
                            if is_valid_file_extension(entry.path()) {
                                // Add the parent directory
                                if let Some(parent) = entry.path().parent() {
                                    unique_directories.insert(PathBuf::from(parent));
                                }

                                let _ = get_file_metadata(entry.path(), &mut all_files);
                            }
                        } else if entry.file_type().is_dir() {
                            // Add all directories to our set
                            unique_directories.insert(entry.path().to_path_buf());
                        }
                    }
                } else {
                    // Handle single file case
                    if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                        if file_name.starts_with(".") {
                            continue;
                        }
                    }

                    // Check if the file has a valid extension before processing
                    if is_valid_file_extension(path) {
                        // Add the parent directory
                        if let Some(parent) = path.parent() {
                            unique_directories.insert(PathBuf::from(parent));
                        }

                        let _ = get_file_metadata(path, &mut all_files);
                    }
                }
            }
            Ok::<_, FileProcessorError>((all_files, unique_directories))
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
    let fm_clone = file_metadata.clone();
    let file_path = fm_clone.base.path.clone();

    println!(
        "saving the path to db and creating embedding: {}",
        file_metadata.base.path
    );

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
                let _ = err_sender.send((file_path, format!("File processing error: {:?}", e)));
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

    println!("saving the file in the db:{:?}", file.base.path);

    task::spawn_blocking({
        let db_path = db_path;
        move || -> Result<String, FileProcessorError> {
            // Fixed error handling with map_err instead of map
            let conn = Connection::open(db_path).map_err(|e| FileProcessorError::Db(e))?;

            // Set pragmas for better performance
            conn.execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                "#,
            )?;

            // Get the filename part
            let path = Path::new(&file.base.path);
            let filename = path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| file.base.name.clone());

            // Get the parent directory
            let parent_path = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| String::from(""));

            // Get directory_id (it should already exist from the batch insert)
            let directory_id: i64 = match conn.query_row(
                "SELECT id FROM directories WHERE path = ?1",
                [&parent_path],
                |row| row.get(0),
            ) {
                Ok(id) => id,
                Err(rusqlite::Error::QueryReturnedNoRows) => {
                    // Directory not found - insert it as a fallback
                    conn.execute(
                        r#"
                        INSERT OR IGNORE INTO directories (path)
                        VALUES (?1);
                        "#,
                        params![parent_path],
                    )?;

                    conn.query_row(
                        "SELECT id FROM directories WHERE path = ?1",
                        [&parent_path],
                        |row| row.get(0),
                    )?
                }
                Err(e) => return Err(FileProcessorError::Db(e)),
            };

            // Insert file metadata with directory_id
            conn.execute(
                r#"
                INSERT OR IGNORE INTO files (directory_id, path, name, extension, size, category)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6);
                "#,
                params![
                    directory_id,
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
    })
    .await
    .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
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
pub struct FileProcessorState(pub Mutex<Option<FileProcessor>>);

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

#[tauri::command]
pub async fn get_semantic_files_data(
    query: String,
    state: State<'_, FileProcessorState>,
    app_handle: AppHandle,
) -> Result<Vec<SemanticMetadata>, String> {
    let processor: FileProcessor = get_processor(&state)?;

    let conn: Connection = Connection::open(&processor.db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;

    // Do a vector similarity search
    let semantic_files: Vec<SemanticMetadata> =
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

    Ok(semantic_files)
}

#[tauri::command]
pub async fn get_files_data(
    query: String,
    state: State<'_, FileProcessorState>,
) -> Result<Vec<FileMetadata>, String> {
    let processor: FileProcessor = get_processor(&state)?;

    let conn: Connection = Connection::open(&processor.db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;

    // Handle short que
    if query.len() < 3 {
        return search_files_by_like(&conn, &query);
    }

    // For queries with >3 characters, first do an FTS search
    let files = search_files_by_fts(&conn, &query)?;

    Ok(files)
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

fn rows_to_semantic_metadata(
    mut rows: Rows,
    distances: &HashMap<String, f32>,
) -> Result<Vec<SemanticMetadata>, String> {
    let mut files: Vec<SemanticMetadata> = Vec::new();

    while let Some(row) = rows.next().map_err(|e| format!("Row error: {e}"))? {
        let id: i64 = row.get(0).map_err(|e| e.to_string())?;

        let distance = *distances.get(&id.to_string()).unwrap_or(&1.0);
        files.push(SemanticMetadata {
            base: BaseMetadata {
                id: Some(id.clone()),
                name: row.get(1).map_err(|e| e.to_string())?,
                path: row.get(2).map_err(|e| e.to_string())?,
            },
            size: row.get(4).map_err(|e| e.to_string())?,
            semantic_type: SearchSectionType::Semantic,
            extension: row.get(3).map_err(|e| e.to_string())?,
            distance: distance,
            content: None, // update this later to return the exact content
        });
    }

    Ok(files)
}

// Convert vector search results to FileMetadata
fn convert_search_results_to_metadata(
    results: Vec<RecordBatch>,
    conn: &Connection,
) -> Result<Vec<SemanticMetadata>, String> {
    // If no results, return empty vector
    if results.is_empty() {
        return Ok(Vec::new());
    }

    let mut file_id_distances: HashMap<String, f32> = HashMap::new();

    // Extract data from results
    for batch in &results {
        if let Some(distance_column) = batch.column_by_name("_distance") {
            if let Some(file_id_column) = batch.column_by_name("file_id") {
                if let (Some(distance_array), Some(file_id_array)) = (
                    distance_column
                        .as_any()
                        .downcast_ref::<arrow_array::Float32Array>(),
                    file_id_column
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>(),
                ) {
                    // Iterate through rows
                    for i in 0..distance_array.len() {
                        if !distance_array.is_null(i) {
                            let distance = distance_array.value(i);
                            if distance < 0.85 {
                                let file_id = file_id_array.value(i);
                                if !file_id_distances.contains_key(file_id)
                                    || file_id_distances[file_id] > distance
                                {
                                    file_id_distances.insert(file_id.to_string(), distance);
                                    println!(
                                        "Relevant match: file_id={}, distance={}",
                                        file_id, distance
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if file_id_distances.is_empty() {
        return Ok(Vec::new());
    }

    // extract the file ids to retrieve from DB
    let file_ids: Vec<String> = file_id_distances.keys().cloned().collect();

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

    rows_to_semantic_metadata(rows, &file_id_distances)
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

pub fn init_file_processor(
    db_path: &str,
    concurrency: usize,
    app_handle: AppHandle,
) -> AppResult<()> {
    let state: State<'_, FileProcessorState> = app_handle.state::<FileProcessorState>();
    let lock_result = state.0.lock();

    match lock_result {
        Ok(mut processor_guard) => {
            *processor_guard = Some(FileProcessor {
                db_path: PathBuf::from(db_path),
                concurrency_limit: concurrency,
            });

            println!("File processor initialized.");
            Ok(())
        }
        Err(e) => {
            let error_msg = format!("Failed to initialize file processor: {}", e);
            eprintln!("{}", error_msg);
            Err(Box::new(Error::new(ErrorKind::Other, error_msg)))
        }
    }
}

pub fn is_valid_file_extension(path: &Path) -> bool {
    let valid_extensions: HashSet<&str> = ["txt", "pdf", "docx", "md", "yaml", "yml"]
        .iter()
        .cloned()
        .collect();

    if let Some(extension) = path.extension() {
        if let Some(ext_str) = extension.to_str() {
            return valid_extensions.contains(ext_str.to_lowercase().as_str());
        }
    }
    false
}

/// Saves directories to the database, handling duplicates via the UNIQUE constraint
async fn save_directories_to_db(
    db_path: PathBuf,
    directories: &HashSet<PathBuf>,
) -> Result<(), FileProcessorError> {
    if directories.is_empty() {
        return Ok(());
    }

    // Convert directories to strings for insertion
    let directories_vec: Vec<String> = directories
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    task::spawn_blocking({
        let dirs = directories_vec.clone();

        move || -> Result<(), FileProcessorError> {
            let mut conn = Connection::open(db_path).map_err(|e| FileProcessorError::Db(e))?;

            // Set pragmas for better performance
            conn.execute_batch(
                r#"
                PRAGMA journal_mode = WAL;
                PRAGMA synchronous = NORMAL;
                "#,
            )?;

            let tx = conn.transaction()?;

            {
                let mut stmt = tx.prepare(
                    r#"
                    INSERT OR IGNORE INTO directories (path, created_at, updated_at)
                    VALUES (?1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);
                    "#,
                )?;

                for dir_path in dirs {
                    stmt.execute(params![dir_path])?;
                }
            }
            tx.commit()?;

            Ok(())
        }
    })
    .await
    .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
}
