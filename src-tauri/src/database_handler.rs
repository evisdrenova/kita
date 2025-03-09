use rusqlite::Connection;
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    SQLite(#[from] rusqlite::Error),
    #[error("No app data directory found")]
    NoAppDataDir,
    #[error("Tauri path error: {0}")]
    TauriPath(#[from] tauri::Error),
}

// handles creating the database and returns the db_path
#[tauri::command]
pub fn initialize_database(app_handle: AppHandle) -> Result<PathBuf, DbError> {
    let app_data_dir: PathBuf = app_handle
        .path()
        .app_data_dir()
        .map_err(|_| DbError::NoAppDataDir)?;

    let db_path: PathBuf = app_data_dir.join("kita-database.sqlite");

    let conn: Connection = Connection::open(&db_path)?;

    let statements = [
        r#"CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE,
            name TEXT,
            extension TEXT,
            size INTEGER,
            category TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );"#,
        r#"CREATE TABLE IF NOT EXISTS embeddings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL,
            embedding TEXT NOT NULL,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
        );"#,
        r#"CREATE TABLE IF NOT EXISTS recents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE,
            lastClicked DATETIME DEFAULT CURRENT_TIMESTAMP
        );"#,
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS files_fts
        USING fts5 (
            doc_text,
            content=''
        );"#,
    ];

    for (i, stmt) in statements.iter().enumerate() {
        match conn.execute(stmt, []) {
            Ok(_) => println!("Statement #{} executed successfully", i + 1),
            Err(e) => {
                println!("Error executing statement #{}: {}", i + 1, e);
                return Err(e.into());
            }
        }
    }

    Ok(db_path)
}
