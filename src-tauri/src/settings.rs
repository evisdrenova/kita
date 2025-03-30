use rusqlite::{params, Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager};
use thiserror::Error;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct AppSettings {
    pub theme: Option<String>,
    pub custom_model_path: Option<String>,
    pub selected_model_id: Option<String>,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    pub global_hotkey: Option<String>,
    pub index_concurrency: Option<usize>,
    pub selected_categories: Option<Vec<String>>,
}

#[derive(Error, Debug)]
pub enum SettingsError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Path error: {0}")]
    Path(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

type Result<T, E = SettingsError> = std::result::Result<T, E>;

pub struct SettingsManager {
    settings: Mutex<AppSettings>,
    db_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SettingValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
}

impl SettingsManager {
    pub fn new(db_path: &str) -> Self {
        Self {
            settings: Mutex::new(AppSettings::default()),
            db_path: db_path.to_string(),
        }
    }

    fn get_connection(&self) -> Result<Connection> {
        Connection::open(&self.db_path).map_err(SettingsError::Database)
    }

    pub fn initialize(&self) -> Result<()> {
        let conn = self.get_connection()?;

        // Try to load existing settings
        let mut stmt = conn.prepare("SELECT data FROM settings WHERE id = 1")?;
        let settings_result = stmt.query_row([], |row| {
            let json: String = row.get(0)?;
            Ok(json)
        });

        match settings_result {
            Ok(json) => {
                let loaded_settings: AppSettings = serde_json::from_str(&json)?;

                let mut settings = self.settings.lock().unwrap();
                *settings = loaded_settings;
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                // No settings found, save defaults
                self.save()?;
            }
            Err(e) => return Err(SettingsError::Database(e)),
        }

        Ok(())
    }

    // Save current settings to database
    pub fn save(&self) -> Result<()> {
        let settings = self.settings.lock().unwrap();
        let json = serde_json::to_string(&*settings)?;

        let conn = self.get_connection()?;
        conn.execute(
            "INSERT OR REPLACE INTO settings_json (id, data, updated_at) 
             VALUES (1, ?, CURRENT_TIMESTAMP)",
            params![json],
        )?;

        Ok(())
    }

    // Get a copy of all settings
    pub fn get_all(&self) -> Result<AppSettings> {
        let settings = self.settings.lock().unwrap();
        Ok(settings.clone())
    }

    // Update specific settings
    pub fn update<F>(&self, updater: F) -> Result<()>
    where
        F: FnOnce(&mut AppSettings),
    {
        let mut settings = self.settings.lock().unwrap();
        updater(&mut settings);
        drop(settings); // Release the lock
        self.save()?;
        Ok(())
    }

    // Set the selected model
    pub fn set_selected_model(&self, model_id: Option<String>) -> Result<()> {
        self.update(|settings| {
            settings.selected_model_id = model_id;
        })
    }

    // Get the selected model
    pub fn get_selected_model(&self) -> Result<Option<String>> {
        let settings = self.settings.lock().unwrap();
        Ok(settings.selected_model_id.clone())
    }

    // Set the theme
    pub fn set_theme(&self, theme: String) -> Result<()> {
        self.update(|settings| {
            settings.theme = Some(theme);
        })
    }

    // Get the theme
    pub fn get_theme(&self) -> Result<Option<String>> {
        let settings = self.settings.lock().unwrap();
        Ok(settings.theme.clone())
    }
}

pub struct SettingsManagerState(pub Arc<SettingsManager>);

// Initialize settings for the app
pub fn init_settings(
    db_path: &str,
    app_handle: AppHandle,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    // Create settings manager
    let settings_manager = SettingsManager::new(db_path);

    // Initialize settings (load or create default)
    settings_manager.initialize()?;

    // Store in app state
    app_handle.manage(SettingsManagerState(Arc::new(settings_manager)));

    println!("Settings successfully initialized.");
    Ok(())
}

#[tauri::command]
pub async fn get_settings(
    settings_manager: tauri::State<'_, SettingsManagerState>,
) -> Result<AppSettings, String> {
    settings_manager
        .0
        .get_all()
        .map_err(|e| format!("Failed to get settings: {}", e))
}

#[tauri::command]
pub async fn update_settings(
    settings_manager: tauri::State<'_, SettingsManagerState>,
    settings: AppSettings,
) -> Result<(), String> {
    settings_manager
        .0
        .update(|current| {
            *current = settings;
        })
        .map_err(|e| format!("Failed to update settings: {}", e))
}

#[tauri::command]
pub async fn set_selected_model(
    settings_manager: tauri::State<'_, SettingsManagerState>,
    model_id: String,
) -> Result<(), String> {
    settings_manager
        .0
        .set_selected_model(Some(model_id))
        .map_err(|e| format!("Failed to set selected model: {}", e))
}

#[tauri::command]
pub async fn get_selected_model(
    settings_manager: tauri::State<'_, SettingsManagerState>,
) -> Result<Option<String>, String> {
    settings_manager
        .0
        .get_selected_model()
        .map_err(|e| format!("Failed to get selected model: {}", e))
}
