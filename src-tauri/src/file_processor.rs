use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Semaphore;
use tokio::task;
use walkdir::WalkDir;
use std::process::Command;


use crate::utils::get_category_from_extension;

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
    pub db_path: PathBuf,         // sqlite db path - TODO, i think we may want to convert this to a db pool and manage a pool of connections using r2d2_rusqlite
    pub concurrency_limit: usize, // max concurrency limit
}
impl FileProcessor {
    // gathers file metadata from the given path
    // uses blocking  i/o, so we do spawn_blocking to run the file processing
    async fn collect_all_files(
        &self,
        paths: &[String],
    ) -> Result<Vec<FileMetadata>, FileProcessorError> {
        let pvec = paths.to_vec();
        let outer = task::spawn_blocking(move || {
            let mut all_files = Vec::new();
            for path_str in pvec {
                let path = Path::new(&path_str);
                if path.is_dir() {
                    for entry in WalkDir::new(path) {
                        let entry = match entry {
                            Ok(e) => e,
                            Err(e) => {
                                eprintln!("Error walking dir: {e}");
                                continue;
                            }
                        };
                        if entry.file_type().is_file() {
                            let _ = process_file(entry.path(), &mut all_files);
                        }
                    }
                } else {
                    let _ = process_file(path, &mut all_files);
                }
            }
            Ok::<_, FileProcessorError>(all_files)
        })
        .await
        .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?;

        let files: Vec<FileMetadata> = outer?;

        Ok(files)
    }

    /// Process a single file: do DB writes, text extraction, embedding, etc.
    /// We'll do this in a blocking task because rusqlite is blocking.
    async fn process_one_file(&self, file: FileMetadata) -> Result<(), FileProcessorError> {
        let handle = task::spawn_blocking({
            let db_path = self.db_path.clone();
            move || -> Result<(), FileProcessorError> {
                let conn = Connection::open(db_path)?;
    
                // Set pragmas for better performance
                conn.execute_batch(
                    r#"
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    "#,
                )?;
    
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
    
                Ok(())
            }
        });
    
        handle
            .await
            .map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
    }

    /// Main async method to process all the given paths:
    /// 1) collect files
    /// 2) spawn tasks with concurrency limit
    /// 3) track progress and optionally emit Tauri events
    pub async fn process_paths(
        &self,
        paths: Vec<String>,
        on_progress: impl Fn(ProcessingStatus) + Send + Sync + Clone + 'static,
    ) -> Result<serde_json::Value, FileProcessorError> {

        println!("The paths {:?}", paths);

        // 1) gather all files
        let files = self.collect_all_files(&paths).await?;
        let total = files.len();

        // concurrency
        let sem = Arc::new(Semaphore::new(self.concurrency_limit));
        let processed = Arc::new(AtomicUsize::new(0));

        // channel for collecting errors
        let (err_tx, mut err_rx) = tokio::sync::mpsc::unbounded_channel();

        // spawn tasks
        let mut handles = Vec::with_capacity(total);
        for file in files {
            let permit = sem.clone();
            let pc = processed.clone();
            let err_sender = err_tx.clone();
            let this = self.clone();
            let progress_fn = on_progress.clone(); // Clone the progress function for each task

            let handle = tokio::spawn(async move {
                let _permit = permit.acquire().await.unwrap(); // concurrency gate
                if let Err(e) = this.process_one_file(file).await {
                    let _ = err_sender.send(e);
                }
                let done = pc.fetch_add(1, Ordering::SeqCst) + 1;
                // report progress
                if total > 0 {
                    let percentage = ((done as f64 / total as f64) * 100.0).round() as usize;
                    let status = ProcessingStatus {
                        total,
                        processed: done,
                        percentage,
                    };
                    progress_fn(status);
                }
            });
            handles.push(handle);
        }
        drop(err_tx);

        // wait for all tasks
        futures::future::join_all(handles).await;

        // gather errors
        let mut errors = Vec::new();
        while let Ok(e) = err_rx.try_recv() {
            errors.push(format!("{:?}", e));
        }

        let success = errors.is_empty();
        let result = serde_json::json!({
            "success": success,
            "totalFiles": processed.load(Ordering::SeqCst),
            "errors": errors
        });
        Ok(result)
    }





    
}

pub fn process_file(
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

// sets up initialize file processor state and store in the tauri instance
#[tauri::command]
pub fn initialize_file_processor(
    db_path: String,
    concurrency: usize,
    app_handle: AppHandle,
) -> Result<(), String> {
    let state = app_handle.state::<FileProcessorState>();
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
    // Create a cloned instance of the processor
    let processor = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        match guard.as_ref() {
            Some(p) => p.clone(),
            None => return Err("File processor not initialized".to_string()),
        }
    };

    // Create a progress handler that emits Tauri event
    let progress_handler = move |status: ProcessingStatus| {
        let _ = app_handle.emit("file-processing-progress", &status);
    };

    processor
        .process_paths(paths, progress_handler)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_files_data(
    query: String,
    state: State<'_, FileProcessorState>
) -> Result<Vec<FileMetadata>, String> {
    let processor = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        guard.as_ref().ok_or("File processor not initialized".to_string())?.clone()
    };

    let conn = Connection::open(processor.db_path)
        .map_err(|e| format!("Failed to open database: {e}"))?;

    // If query is empty, show last 50 inserted or last 50 updated
    // TODO: we can be smarter here and understand the files the user commonly ccesses and show them here using a last_accessed_date or recents or something 
    if query.trim().is_empty() {
        let mut stmt = conn.prepare(
            r#"
            SELECT id, name, path, extension, size, created_at, updated_at
            FROM files
            ORDER BY updated_at DESC
            LIMIT 50
            "#
        ).map_err(|e| format!("Failed to prepare statement: {e}"))?;

        let files_iter = stmt.query_map([], |row| {
            Ok(FileMetadata {
                base: BaseMetadata {
                    id: Some(row.get(0)?),
                    name: row.get(1)?,
                    path: row.get(2)?,
                },
                file_type: SearchSectionType::Files,
                extension: row.get(3)?,
                size: row.get(4)?,
                created_at: row.get(5).ok(),
                updated_at: row.get(6).ok(),
            })
        }).map_err(|e| format!("Query execution error: {e}"))?;

        let mut files = Vec::new();
        for file in files_iter {
            files.push(file.map_err(|e| format!("Row mapping error: {e}"))?);
        }
        return Ok(files);
    }

    // For non-empty query, do an FTS5 MATCH
    // Example: simple usage might treat query as a phrase. 
    // For partial matches or multiple tokens, see notes below.

    let mut stmt = conn.prepare(
        r#"
        SELECT
          f.id,
          f.name,
          f.path,
          f.extension,
          f.size,
          f.created_at,
          f.updated_at
        FROM files_fts AS ft
        JOIN files f ON ft.rowid = f.id
        WHERE ft MATCH ?
        LIMIT 50
        "#
    ).map_err(|e| format!("Failed to prepare statement: {e}"))?;

    let files_iter = stmt.query_map([query.as_str()], |row| {
        Ok(FileMetadata {
            base: BaseMetadata {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                path: row.get(2)?,
            },
            file_type: SearchSectionType::Files,
            extension: row.get(3)?,
            size: row.get(4)?,
            created_at: row.get(5).ok(),
            updated_at: row.get(6).ok(),
        })
    }).map_err(|e| format!("Query execution error: {e}"))?;

    let mut files = Vec::new();
    for file in files_iter {
        files.push(file.map_err(|e| format!("Row mapping error: {e}"))?);
    }

    Ok(files)
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
        Err(format!("Failed to open file, exit code: {:?}", status.code()))
    }
}