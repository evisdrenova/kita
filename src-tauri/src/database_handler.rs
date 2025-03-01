use rusqlite::Connection;
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

// handles creating the database
#[tauri::command]
pub fn initialize_database(app_handle: AppHandle) -> Result<(), DbError> {
    let app_data_dir = app_handle.path().app_data_dir().map_err(|_| DbError::NoAppDataDir)?;
    
    let mut db_path = app_data_dir;
    db_path.push("kita-database.sqlite");

    let conn = Connection::open(db_path)?;

    conn.execute_batch(
      "
      CREATE TABLE IF NOT EXISTS files (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT UNIQUE,
        name TEXT,
        extension TEXT,
        size INTEGER,
        category TEXT,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
      );
  
      CREATE TABLE IF NOT EXISTS embeddings (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        file_id INTEGER NOT NULL,
        embedding TEXT NOT NULL,
        created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (file_id) REFERENCES files(id) ON DELETE CASCADE
      );
  
      CREATE TABLE IF NOT EXISTS recents (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT UNIQUE,
        lastClicked DATETIME DEFAULT CURRENT_TIMESTAMP
      );
      ",
    )?;
  
    Ok(())
  }