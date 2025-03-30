use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use thiserror::Error;

use crate::AppResult;

#[derive(Error, Debug)]
pub enum SettingError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

type Result<T, E = SettingError> = std::result::Result<T, E>;

pub const PREF_SELECTED_MODEL: &str = "selected_model";
pub const PREF_THEME: &str = "theme";
pub const PREF_CUSTOM_MODEL_PATH: &str = "custom_model_path";
pub const PREF_WINDOW_SIZE: &str = "window_size";
pub const PREF_HOTKEY: &str = "hotkey";

pub struct SettingManager {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SettingValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

impl SettingManager {
    // Create new setting manager
    pub fn initialize(db_path: &str) -> Result<Self> {
        let conn = Connection::open(&db_path).map_err(SettingError::Database)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // Set a string setting
    pub fn set_string(&self, key: &str, value: &str) -> Result<()> {
        self.set_value(key, SettingValue::String(value.to_string()))
    }

    // Get a string setting
    pub fn get_string(&self, key: &str) -> Result<Option<String>> {
        match self.get_value(key)? {
            Some(SettingValue::String(s)) => Ok(Some(s)),
            None => Ok(None),
            _ => Err(SettingError::Path(format!(
                "Setting {} is not a string",
                key
            ))),
        }
    }

    // Set a boolean setting
    pub fn set_bool(&self, key: &str, value: bool) -> Result<()> {
        self.set_value(key, SettingValue::Boolean(value))
    }

    // Get a boolean setting
    pub fn get_bool(&self, key: &str) -> Result<Option<bool>> {
        match self.get_value(key)? {
            Some(SettingValue::Boolean(b)) => Ok(Some(b)),
            None => Ok(None),
            _ => Err(SettingError::Path(format!(
                "Setting {} is not a boolean",
                key
            ))),
        }
    }

    // Set a JSON value
    pub fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_value(value)?;
        self.set_value(key, SettingValue::Json(json))
    }

    // Get a JSON value
    pub fn get_json<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>> {
        match self.get_value(key)? {
            Some(SettingValue::Json(j)) => {
                let value: T = serde_json::from_value(j)?;
                Ok(Some(value))
            }
            None => Ok(None),
            _ => Err(SettingError::Path(format!("Setting {} is not JSON", key))),
        }
    }

    // Set a generic setting value
    fn set_value(&self, key: &str, value: SettingValue) -> Result<()> {
        let value_type = match value {
            SettingValue::String(_) => "string",
            SettingValue::Integer(_) => "integer",
            SettingValue::Float(_) => "float",
            SettingValue::Boolean(_) => "boolean",
            SettingValue::Json(_) => "json",
        };

        let value_str = match &value {
            SettingValue::String(s) => s.clone(),
            SettingValue::Integer(i) => i.to_string(),
            SettingValue::Float(f) => f.to_string(),
            SettingValue::Boolean(b) => b.to_string(),
            SettingValue::Json(j) => serde_json::to_string(j)?,
        };

        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value, value_type, updated_at) 
             VALUES (?, ?, ?, CURRENT_TIMESTAMP)",
            params![key, value_str, value_type],
        )
        .map_err(SettingError::Database)?;

        Ok(())
    }

    // Get a generic setting value
    fn get_value(&self, key: &str) -> Result<Option<SettingValue>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT value, value_type FROM settings WHERE key = ?")
            .map_err(SettingError::Database)?;

        let rows = stmt
            .query_map(params![key], |row| {
                let value: String = row.get(0)?;
                let value_type: String = row.get(1)?;

                let pref_value = match value_type.as_str() {
                    "string" => SettingValue::String(value),
                    "integer" => SettingValue::Integer(value.parse().unwrap_or(0)),
                    "float" => SettingValue::Float(value.parse().unwrap_or(0.0)),
                    "boolean" => SettingValue::Boolean(value.parse().unwrap_or(false)),
                    "json" => SettingValue::Json(
                        serde_json::from_str(&value).unwrap_or(serde_json::Value::Null),
                    ),
                    _ => SettingValue::String(value),
                };

                Ok(pref_value)
            })
            .map_err(SettingError::Database)?;

        let mut values: Vec<SettingValue> = rows
            .collect::<SqliteResult<_>>()
            .map_err(SettingError::Database)?;

        Ok(values.pop())
    }

    // Delete a setting
    pub fn delete(&self, key: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM settings WHERE key = ?", params![key])
            .map_err(SettingError::Database)?;

        Ok(())
    }

    // Get all settings
    pub fn get_all(&self) -> Result<Vec<(String, SettingValue)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT key, value, value_type FROM settings ORDER BY key")
            .map_err(SettingError::Database)?;

        let rows = stmt
            .query_map([], |row| {
                let key: String = row.get(0)?;
                let value: String = row.get(1)?;
                let value_type: String = row.get(2)?;

                let pref_value = match value_type.as_str() {
                    "string" => SettingValue::String(value),
                    "integer" => SettingValue::Integer(value.parse().unwrap_or(0)),
                    "float" => SettingValue::Float(value.parse().unwrap_or(0.0)),
                    "boolean" => SettingValue::Boolean(value.parse().unwrap_or(false)),
                    "json" => SettingValue::Json(
                        serde_json::from_str(&value).unwrap_or(serde_json::Value::Null),
                    ),
                    _ => SettingValue::String(value),
                };

                Ok((key, pref_value))
            })
            .map_err(SettingError::Database)?;

        rows.collect::<SqliteResult<_>>()
            .map_err(SettingError::Database)
    }
}

pub struct SettingsManagerState(pub Arc<SettingManager>);

pub fn init_settings(db_path: &str, app_handle: AppHandle) -> AppResult<()> {
    // Try to initialize the settings manager
    let settings_manager = match SettingManager::initialize(db_path) {
        Ok(manager) => manager,
        Err(e) => {
            eprintln!("Failed to initialize settings: {e}");
            return Err(Box::new(Error::new(
                ErrorKind::Other,
                format!("Failed to initialize settings: {}", e),
            )));
        }
    };

    app_handle.manage(SettingsManagerState(Arc::new(settings_manager)));
    println!("Settings successfully initialized.");
    Ok(())
}
