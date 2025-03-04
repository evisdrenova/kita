use rusqlite::Connection;
use tauri::AppHandle;
use tauri::Manager;
use thiserror::Error;
use std::path::PathBuf;

#[derive(Error, Debug)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    SQLite(#[from] rusqlite::Error),
    #[error("No app data directory found")]
    NoAppDataDir,
    #[error("Tauri path error: {0}")]
    TauriPath(#[from] tauri::Error),
}

/* FTS5 stays in syc with the for files table using triggers which makes updating our indexes easier.  */

// handles creating the database and returns the db_path
#[tauri::command]
pub fn initialize_database(app_handle: AppHandle) -> Result<PathBuf, DbError> {
    let app_data_dir: PathBuf = app_handle.path().app_data_dir().map_err(|_| DbError::NoAppDataDir)?;
    
    let db_path = app_data_dir.join("kita-database.sqlite");

    let conn = Connection::open(&db_path)?;

    conn.execute_batch(
      r#"
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

      CREATE VIRTUAL TABLE IF NOT EXISTS files_fts
      USING fts5 (
          name,
          path,
          extension,
          content='',
          tokenize='unicode61'
      );

      -- Trigger: after inserting into files, insert into files_fts
      CREATE TRIGGER IF NOT EXISTS files_ai
      AFTER INSERT ON files BEGIN
          INSERT INTO files_fts(rowid, name, path, extension)
          VALUES (new.id, new.name, new.path, new.extension);
      END;

      -- Trigger: after deleting from files, remove from files_fts
      CREATE TRIGGER IF NOT EXISTS files_ad
      AFTER DELETE ON files BEGIN
          DELETE FROM files_fts WHERE rowid = old.id;
      END;

      -- Trigger: after updating files, update files_fts
      CREATE TRIGGER IF NOT EXISTS files_au
      AFTER UPDATE ON files BEGIN
          UPDATE files_fts
          SET name = new.name,
              path = new.path,
              extension = new.extension
          WHERE rowid = old.id;
      END;
      "#
  )?;

  Ok(db_path)
}