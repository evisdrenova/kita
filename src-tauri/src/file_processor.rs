use rusqlite::{params, Connection, Error as RusqliteError};
use tauri::{AppHandle, Emitter};
use std::sync::{Arc, Mutex};
use tokio::sync::{Semaphore, Barrier};
use tokio::task;
use serde::{Serialize, Deserialize};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
    pub db_path: PathBuf, // sqlite db path
    pub concurrency_limit: usize, // max concurrency limit
}
impl FileProcessor {

// gathers file metadata from the given path
// uses blocking  i/o, so we do spawn_blocking to run the file processing

async fn collect_all_files(&self, paths: &[String]) -> Result<Vec<FileMetadata>, FileProcessorError> {
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

    // Step 2: now 'outer' is Result<Vec<FileMetadata>, FileProcessorError>. 
    //         We unwrap the inner result with '?'.
    let files: Vec<FileMetadata> = outer?;

    // Finally return the vector
    Ok(files)
}


/// Process a single file: do DB writes, text extraction, embedding, etc.
    /// We'll do this in a blocking task because rusqlite is blocking.
    async fn process_one_file(&self, file: FileMetadata) -> Result<(), FileProcessorError> {
        let handle = task::spawn_blocking({
            let db_path = self.db_path.clone();
            move || -> Result<(), FileProcessorError> {
                let conn = Connection::open(db_path)?;
    
                // Example table creation
                conn.execute_batch(
                    r#"
                    PRAGMA journal_mode = WAL;
                    PRAGMA synchronous = NORMAL;
                    CREATE TABLE IF NOT EXISTS files (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        path TEXT UNIQUE,
                        name TEXT,
                        extension TEXT,
                        size INTEGER,
                        created_at TEXT,
                        updated_at TEXT
                    );
                    "#,
                )?;
    
                // Insert or update
                conn.execute(
                    r#"
                    INSERT OR IGNORE INTO files (path, name, extension, size, created_at, updated_at)
                    VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'));
                    "#,
                    params![file.base.path, file.base.name, file.extension, file.size],
                )?;
    
                Ok(())
            }
        });
        
        handle.await.map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?
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
                    progress_fn(status); // Use the cloned function here
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

pub fn process_file(path: &Path, all_files: &mut Vec<FileMetadata>) -> Result<(), FileProcessorError> {
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
pub async fn init_file_processor(
    db_path: String, 
    concurrency: usize, 
    state: tauri::State<'_, FileProcessorState>
) -> Result<(), String> {
    let mut processor_guard = state.0.lock().map_err(|e| e.to_string())?;
    *processor_guard = Some(FileProcessor {
        db_path: PathBuf::from(db_path),
        concurrency_limit: concurrency,
    });
    Ok(())
}

#[tauri::command]
pub async fn process_paths_tauri(
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
    }; // MutexGuard is dropped here
    
    // Create a progress handler that emits Tauri event
    let progress_handler = move |status: ProcessingStatus| {
        let _ = app_handle.emit("file-processing-progress", &status);
    };
    
    processor
        .process_paths(paths, progress_handler)
        .await
        .map_err(|e| e.to_string())
}