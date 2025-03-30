use rusqlite::Connection;
use std::io::{Error, ErrorKind};
use std::path::PathBuf;
use tauri::AppHandle;
use tauri::Manager;

use crate::AppResult;

/// Initialize the database and return the path to the created database file
pub fn init_database(app_handle: AppHandle) -> AppResult<std::path::PathBuf> {
    let app_data_dir: PathBuf = match app_handle.path().app_data_dir() {
        Ok(dir) => dir,
        Err(_) => {
            let error_msg = "Failed to get app data directory";
            eprintln!("{}", error_msg);
            return Err(Box::new(Error::new(ErrorKind::NotFound, error_msg)));
        }
    };

    let db_path: PathBuf = app_data_dir.join("kita-database.sqlite");

    let conn: Connection = match Connection::open(&db_path) {
        Ok(conn) => conn,
        Err(e) => {
            let error_msg = format!("Failed to open database connection: {}", e);
            eprintln!("{}", error_msg);
            return Err(Box::new(Error::new(ErrorKind::Other, error_msg)));
        }
    };

    let files_table = r#"CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT UNIQUE,
            name TEXT,
            extension TEXT,
            size INTEGER,
            category TEXT,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
        );"#;

    let settings_table = r#"CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                value_type TEXT NOT NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );"#;

    let fts_table = r#"CREATE VIRTUAL TABLE IF NOT EXISTS files_fts
        USING fts5 (
            doc_text,
            content=''
        );"#;

    let statements = vec![files_table, settings_table, fts_table];

    for (i, stmt) in statements.iter().enumerate() {
        if let Err(e) = conn.execute(stmt, []) {
            let error_msg = format!("Error executing statement #{}: {}", i + 1, e);
            eprintln!("{}", error_msg);
            return Err(Box::new(Error::new(ErrorKind::Other, error_msg)));
        }
    }

    println!("Database successfully initialized at {}", db_path.display());
    Ok(db_path)
}
