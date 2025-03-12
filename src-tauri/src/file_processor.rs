use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::Semaphore;
use tokio::task;
use walkdir::WalkDir;
use tracing::{error, info, warn, Level};

use crate::tokenizer::{build_doc_text, build_trigrams};

use crate::utils::get_category_from_extension;

use crate::parser::{ParsedChunk, ParsingOrchestrator, ParserConfig};

use crate::parser::runner::parse_with_tokio;

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
    pub db_path: PathBuf, // sqlite db path - TODO, i think we may want to convert this to a db pool and manage a pool of connections using r2d2_rusqlite
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
            let pb = PathBuf::from(file.base.path.clone());

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

            let config = ParserConfig {
                chunk_size: 100,
                chunk_overlap: 2,
                normalize_text: true,
                extract_metadata: true,
                max_concurrent_files: 4,
                use_gpu_acceleration: true,
            };


            // Create parsing orchestrator
            let orchestrator = ParsingOrchestrator::new(config);

            // Collect all files to parse
            // let files = collect_files(inputs)?;
            // info!("Found {} files to parse", files.len());

            // // Parse files
            // let chunks = if rayon {
            //     // Use Rayon for CPU parallelization
            //     parse_with_rayon(&orchestrator, files).await?
            // } else {

            // let mut chunksVec: Vec<PathBuf> = Vec::new();

            // for path in files.iter() {
            //     let pb: PathBuf = PathBuf::from(&path.base.path);
            //     chunksVec.push(pb)
            // }


            // let pb = PathBuf::from(file.base.path.clone());

               // Use Tokio for async parallelization
                let chunks = parse_with_tokio(&orchestrator, vec![pb]).await.map_err(|e| FileProcessorError::Other(format!("spawn_blocking error: {e}")))?;
            // };

            info!("Parsed {} chunks", chunks.len());

            // Output chunks

                // Print summary to stdout
                for (i, chunk) in chunks.iter().enumerate().take(5) {
                    println!(
                        "Chunk {}: {} bytes, from {}",
                        i,
                        chunk.content.len(),
                        chunk.metadata.source_path.display()
                    );
                }
                if chunks.len() > 5 {
                    println!("... and {} more chunks", chunks.len() - 5);
                }
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
    state: State<'_, FileProcessorState>,
) -> Result<Vec<FileMetadata>, String> {
    let processor = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("File processor not initialized".to_string())?
            .clone()
    };

    let conn =
        Connection::open(processor.db_path).map_err(|e| format!("Failed to open database: {e}"))?;

    // If user typed nothing, return first 50 files but we can be smarter here and check based on recents
    if query.trim().is_empty() {
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
            LIMIT 50
        "#,
            )
            .map_err(|e| format!("Failed to prepare statement: {e}"))?;

        let mut rows = stmt.query([]).map_err(|e| format!("Query error: {e}"))?;

        let mut files = Vec::new();
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
        return Ok(files);
    }

    if query.len() < 3 {
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

        // Provide all three parameters the query expects
        let mut rows = stmt
            .query(params![&like_pattern, &like_pattern, &like_pattern])
            .map_err(|e| format!("Query error: {e}"))?;

        let mut files = Vec::new();
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
        return Ok(files);
    }

    let search_trigrams = build_trigrams(&query);
    println!(
        "more than 3 in search query, the search trigrams: {}",
        search_trigrams
    );

    // do an FTS search on doc_text
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

    // Fix: Use search_trigrams instead of raw query
    let mut rows = stmt
        .query([search_trigrams.as_str()])
        .map_err(|e| format!("Query error: {e}"))?;

    let mut files = Vec::new();
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

#[tauri::command]
pub fn check_fts_table(state: State<'_, FileProcessorState>) -> Result<String, String> {
    let processor = {
        let guard = state.0.lock().map_err(|e| e.to_string())?;
        guard
            .as_ref()
            .ok_or("File processor not initialized".to_string())?
            .clone()
    };

    let conn =
        Connection::open(processor.db_path).map_err(|e| format!("Failed to open database: {e}"))?;

    let mut result = String::new();

    // Check total count of entries in files_fts table
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM files_fts", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count FTS rows: {e}"))?;

    result.push_str(&format!("Total entries in files_fts table: {}\n\n", count));

    // If there are entries, check some examples
    if count > 0 {
        // Get a sample of the entries
        let mut stmt = conn
            .prepare("SELECT rowid, doc_text FROM files_fts LIMIT 5")
            .map_err(|e| format!("Failed to prepare statement: {e}"))?;

        let rows = stmt
            .query_map([], |row| {
                let rowid: i64 = row.get(0)?;
                let doc_text: String = row.get(1)?;
                Ok((rowid, doc_text))
            })
            .map_err(|e| format!("Query error: {e}"))?;

        result.push_str("Sample entries from files_fts:\n");
        for entry in rows {
            if let Ok((rowid, doc_text)) = entry {
                result.push_str(&format!("rowid: {}, doc_text: {}\n", rowid, doc_text));
            }
        }

        // Get and check a "notes" file if one exists
        if let Ok((rowid, doc_text)) = conn.query_row(
            "SELECT ft.rowid, ft.doc_text FROM files_fts ft JOIN files f ON ft.rowid = f.id WHERE f.name LIKE '%notes%' LIMIT 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        ) {
            result.push_str(&format!("\nFound 'notes' file - rowid: {}, doc_text: {}\n", rowid, doc_text));
            
            // Test a search that should match this "notes" file
            let trigram_search = build_trigrams("note");
            let count_match: i64 = conn.query_row(
                "SELECT COUNT(*) FROM files_fts WHERE doc_text MATCH ?",
                [trigram_search.as_str()],
                |row| row.get(0)
            ).unwrap_or(0);
            
            result.push_str(&format!("\nFiles matching 'note' trigrams ({}): {}\n", trigram_search, count_match));
        }
    }

    // Check the configuration of the FTS table
    result.push_str("\nFTS5 table configuration:\n");
    if let Ok(config) = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE name = 'files_fts'",
        [],
        |row| row.get::<_, String>(0),
    ) {
        result.push_str(&format!("Table SQL: {}\n", config));
    }

    Ok(result)
}
