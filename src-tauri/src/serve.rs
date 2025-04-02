use futures_util::StreamExt;
use reqwest::Client;
use rusqlite::types::Null;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use thiserror::Error;

use crate::{model::LLMServer, settings::SettingsManagerState};

// --- Error Handling ---
#[derive(Error, Debug)]
pub enum ModelError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Download failed: {0}")]
    DownloadFailed(String),
}

// Type alias for results
type Result<T, E = ModelError> = std::result::Result<T, E>;

// --- Model Data Structures ---

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HuggingFaceModelInfo {
    id: String,
    name: String,
    repo_id: String,
    filename: String,
    size: u64, // Size in MB
    quantization: String,
}

/// this represnts models that we download locally
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size: u64, // Size in MB
    pub path: String,
    pub quantization: String,
    pub is_downloaded: bool,
}

pub struct ModelRegistry {
    available_models: Mutex<Vec<HuggingFaceModelInfo>>,
    downloaded_models: Mutex<Vec<ModelInfo>>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            available_models: Mutex::new(Vec::new()),
            downloaded_models: Mutex::new(Vec::new()),
        }
    }

    // Initialize with pre-defined list of models
    pub fn initialize(&self) {
        let models = vec![
            HuggingFaceModelInfo {
                id: "mistral-7b-instruct-v0.2-q4".to_string(),
                name: "Mistral 7B Instruct (Q4_K_M)".to_string(),
                repo_id: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".to_string(),
                filename: "mistral-7b-instruct-v0.2.Q4_K_M.gguf".to_string(),
                size: 4200,
                quantization: "Q4_K_M".to_string(),
            },
            HuggingFaceModelInfo {
                id: "mistral-7b-instruct-v0.2-q5".to_string(),
                name: "Mistral 7B Instruct (Q5_K_M)".to_string(),
                repo_id: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".to_string(),
                filename: "mistral-7b-instruct-v0.2.Q5_K_M.gguf".to_string(),
                size: 5100,
                quantization: "Q5_K_M".to_string(),
            },
            HuggingFaceModelInfo {
                id: "llama-2-7b-chat-q4".to_string(),
                name: "Llama 2 7B Chat (Q4_K_M)".to_string(),
                repo_id: "TheBloke/Llama-2-7B-Chat-GGUF".to_string(),
                filename: "llama-2-7b-chat.Q4_K_M.gguf".to_string(),
                size: 4100,
                quantization: "Q4_K_M".to_string(),
            },
        ];

        let mut available = self.available_models.lock().unwrap();
        *available = models;
    }

    // Scan the models directory to find downloaded models with option for custom path
    pub fn scan_downloaded_models(
        &self,
        app_handle: &AppHandle,
        custom_path: Option<&str>,
    ) -> Result<()> {
        let models_dir = get_models_dir(app_handle, custom_path)?;
        let mut downloaded = Vec::new();

        // Ensure directory exists
        if !models_dir.exists() {
            fs::create_dir_all(&models_dir)?;
            self.downloaded_models.lock().unwrap().clear();
            return Ok(());
        }

        // Read all files in models directory
        let entries = fs::read_dir(&models_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Check if it's a .gguf file
            if let Some(ext) = path.extension() {
                if ext == "gguf" {
                    // Find corresponding model in available_models
                    let filename = path.file_name().unwrap().to_string_lossy();
                    let available = self.available_models.lock().unwrap();

                    if let Some(model) = available.iter().find(|m| m.filename == filename) {
                        let model_info = ModelInfo {
                            id: model.id.clone(),
                            name: model.name.clone(),
                            size: model.size,
                            path: path.to_string_lossy().to_string(),
                            quantization: model.quantization.clone(),
                            is_downloaded: true,
                        };
                        downloaded.push(model_info);
                    }
                }
            }
        }

        *self.downloaded_models.lock().unwrap() = downloaded;
        Ok(())
    }

    // Get all available models (combining available and downloaded info)
    pub fn get_available_models(&self) -> Vec<ModelInfo> {
        let available = self.available_models.lock().unwrap();
        let downloaded = self.downloaded_models.lock().unwrap();

        available
            .iter()
            .map(|model| {
                // Check if this model has been downloaded
                let downloaded_model = downloaded.iter().find(|m| m.id == model.id);

                if let Some(dm) = downloaded_model {
                    dm.clone()
                } else {
                    ModelInfo {
                        id: model.id.clone(),
                        name: model.name.clone(),
                        size: model.size,
                        path: String::new(), // Empty path for not-downloaded models
                        quantization: model.quantization.clone(),
                        is_downloaded: false,
                    }
                }
            })
            .collect()
    }

    // Get info for a specific model
    pub fn get_model(&self, model_id: &str) -> Option<ModelInfo> {
        // First check downloaded models
        let downloaded = self.downloaded_models.lock().unwrap();
        if let Some(model) = downloaded.iter().find(|m| m.id == model_id) {
            return Some(model.clone());
        }

        // Then check available models
        let available = self.available_models.lock().unwrap();
        available
            .iter()
            .find(|m| m.id == model_id)
            .map(|model| ModelInfo {
                id: model.id.clone(),
                name: model.name.clone(),
                size: model.size,
                path: String::new(),
                quantization: model.quantization.clone(),
                is_downloaded: false,
            })
    }

    // Get HuggingFace info for a model
    pub fn get_hf_model_info(&self, model_id: &str) -> Option<HuggingFaceModelInfo> {
        let available = self.available_models.lock().unwrap();
        available.iter().find(|m| m.id == model_id).cloned()
    }

    // Register a downloaded model
    pub fn register_downloaded_model(&self, model_info: ModelInfo) {
        let mut downloaded = self.downloaded_models.lock().unwrap();

        // Check if model already exists, update if it does
        if let Some(idx) = downloaded.iter().position(|m| m.id == model_info.id) {
            downloaded[idx] = model_info;
        } else {
            downloaded.push(model_info);
        }
    }
}

// Get models directory path
fn get_models_dir(app_handle: &AppHandle, custom_path: Option<&str>) -> Result<PathBuf> {
    // If custom path is provided, use it
    if let Some(path) = custom_path {
        return Ok(PathBuf::from(path));
    }

    // Otherwise use app data directory
    let app_data_dir = app_handle.path().app_data_dir().map_err(|_| {
        ModelError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Model file directory not found",
        ))
    })?;

    Ok(app_data_dir.join("models"))
}

// Get HuggingFace download URL for a model
fn get_hf_download_url(repo_id: &str, filename: &str) -> String {
    format!(
        "https://huggingface.co/{}/resolve/main/{}",
        repo_id, filename
    )
}

// Progress data structure
#[derive(Clone, Serialize, Deserialize)]
struct DownloadProgress {
    progress: f64,
    model_id: String,
}

// Download a model from HuggingFace with option for custom path
async fn download_model_from_hf(
    app_handle: &AppHandle,
    model_info: &HuggingFaceModelInfo,
    custom_path: Option<&str>,
) -> Result<PathBuf> {
    // Create models directory if it doesn't exist
    let models_dir = get_models_dir(app_handle, custom_path)?;
    if !models_dir.exists() {
        fs::create_dir_all(&models_dir)?;
    }

    // Setup paths for the downloaded file
    let file_path = models_dir.join(&model_info.filename);
    let temp_path = models_dir.join(format!("{}.downloading", &model_info.filename));

    // Check for existing downloads
    if file_path.exists() {
        // If the file exists, check if it's complete by trying to verify its size
        let metadata = fs::metadata(&file_path)?;
        let file_size = metadata.len();

        // If we know the expected size and it matches, assume file is complete
        if model_info.size > 0 && file_size == model_info.size * 1024 * 1024 {
            // Convert MB to bytes
            println!("Model already downloaded and complete");
            return Ok(file_path);
        } else {
            // File exists but is the wrong size - delete it
            println!("Found incomplete or corrupted download, removing...");
            fs::remove_file(&file_path)?;
        }
    }

    // Also check for any temporary download in progress
    if temp_path.exists() {
        println!("Found partial download, removing...");
        fs::remove_file(&temp_path)?;
    }

    // Get download URL
    let url = get_hf_download_url(&model_info.repo_id, &model_info.filename);
    println!("Downloading model from: {}", url);

    // Start download
    let client = Client::new();
    let res = client.get(&url).send().await?;

    // Check response
    if !res.status().is_success() {
        return Err(ModelError::DownloadFailed(format!(
            "Server returned: {}",
            res.status()
        )));
    }

    // Get total size
    let total_size = res.content_length().unwrap_or(0);

    // Create temporary file for writing
    let mut file = fs::File::create(&temp_path)?;

    // Download the file in chunks, updating progress
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        file.write_all(&chunk)?;

        downloaded += chunk.len() as u64;
        let progress = if total_size > 0 {
            (downloaded as f64 / total_size as f64) * 100.0
        } else {
            0.0
        };

        // Emit progress event
        let _ = app_handle.emit(
            "model-download-progress",
            DownloadProgress {
                progress,
                model_id: model_info.id.clone(),
            },
        );
    }

    file.flush()?;

    // Only after successful download, move the temporary file to the final location
    fs::rename(&temp_path, &file_path)?;

    // Double check the final file size if we know the expected size
    if model_info.size > 0 {
        let metadata = fs::metadata(&file_path)?;
        let file_size = metadata.len();
        let expected_size = model_info.size * 1024 * 1024; // Convert MB to bytes

        if file_size != expected_size {
            println!(
                "Warning: Downloaded file size ({} bytes) differs from expected size ({} bytes)",
                file_size, expected_size
            );
        }
    }

    Ok(file_path)
}

// --- Tauri Commands ---

#[tauri::command]
pub async fn get_available_models(
    app_handle: AppHandle,
    model_registry: State<'_, ModelRegistry>,
    custom_path: Option<String>,
) -> std::result::Result<Vec<ModelInfo>, String> {
    model_registry
        .scan_downloaded_models(&app_handle, custom_path.as_deref())
        .map_err(|e| e.to_string())?;

    Ok(model_registry.get_available_models())
}

#[tauri::command]
pub async fn start_model_download(
    app_handle: AppHandle,
    model_registry: State<'_, ModelRegistry>,
    model_id: String,
    custom_path: Option<String>,
) -> Result<String, String> {
    // Get model info
    let hf_model_info = model_registry
        .get_hf_model_info(&model_id)
        .ok_or_else(|| format!("Model {} not found", model_id))?;

    // Clone what we need for the async task
    let app_handle_clone = app_handle.clone();
    let model_id_clone = model_id.clone();
    let hf_model_info_clone = hf_model_info.clone();
    let custom_path_clone = custom_path.clone();

    // Start download in background
    tokio::spawn(async move {
        match download_model_from_hf(
            &app_handle_clone,
            &hf_model_info_clone,
            custom_path_clone.as_deref(),
        )
        .await
        {
            Ok(file_path) => {
                // Register the downloaded model
                let model_info = ModelInfo {
                    id: hf_model_info_clone.id,
                    name: hf_model_info_clone.name,
                    size: hf_model_info_clone.size,
                    path: file_path.to_string_lossy().to_string(),
                    quantization: hf_model_info_clone.quantization,
                    is_downloaded: true,
                };

                // Update registry
                let registry = app_handle_clone.state::<ModelRegistry>();
                registry.register_downloaded_model(model_info);

                // Notify frontend of completion
                let _ = app_handle_clone.emit("model-download-complete", model_id_clone);
            }
            Err(e) => {
                eprintln!("Download error: {}", e);
                // Notify frontend of error
                let _ = app_handle_clone.emit(
                    "model-download-error",
                    serde_json::json!({
                        "model_id": model_id_clone,
                        "error": e.to_string()
                    }),
                );
            }
        }
    });

    Ok("Download started".to_string())
}

#[tauri::command]
pub async fn check_model_exists(
    app_handle: AppHandle,
    model_registry: State<'_, ModelRegistry>,
    model_id: String,
    custom_path: Option<String>,
) -> Result<bool, String> {
    // Rescan to make sure we have the latest info
    model_registry
        .scan_downloaded_models(&app_handle, custom_path.as_deref())
        .map_err(|_| "Error scanning models".to_string())?;

    // Check if model exists
    let model_exists = model_registry
        .get_model(&model_id)
        .map(|m| m.is_downloaded)
        .unwrap_or(false);

    Ok(model_exists)
}

pub fn initialize_model_registry(app: &mut tauri::App) -> Result<()> {
    // Create and initialize the registry
    let registry = ModelRegistry::new();
    registry.initialize();

    // Add registry to the app state
    app.manage(registry);

    // Launch background scan and server initialization
    let app_handle = app.app_handle().clone();
    tauri::async_runtime::spawn(async move {
        scan_and_initialize_server(app_handle).await;
    });

    Ok(())
}

// Scan models and initialize server if appropriate
async fn scan_and_initialize_server(app_handle: AppHandle) {
    // First scan for models
    let registry_state = app_handle.state::<ModelRegistry>();
    match registry_state.scan_downloaded_models(&app_handle, None) {
        Ok(_) => {
            initialize_server_from_settings(&app_handle).await;
        }
        Err(e) => {
            eprintln!("Error scanning models during init: {}", e);
        }
    }
}

async fn initialize_server_from_settings(app_handle: &AppHandle) {
    // Get selected model from settings
    let selected_model_id = match get_selected_model_from_settings(app_handle) {
        Ok(Some(id)) => id,
        Ok(None) => {
            notify_model_selection_required(app_handle);
            return;
        }
        Err(e) => {
            eprintln!("Error getting settings: {}", e);
            return;
        }
    };

    // Try to load the selected model
    handle_selected_model(app_handle, &selected_model_id).await;
}

// Get the selected model ID from settings
fn get_selected_model_from_settings(app_handle: &AppHandle) -> Result<Option<String>, String> {
    let settings_state = app_handle.state::<SettingsManagerState>();
    let settings = settings_state.0.get_settings().map_err(|e| e.to_string())?;

    Ok(settings.selected_model_id)
}

// Handle the selected model: check if it exists and is downloaded
async fn handle_selected_model(app_handle: &AppHandle, model_id: &str) {
    let registry_state = app_handle.state::<ModelRegistry>();

    match registry_state.get_model(model_id) {
        Some(model) if model.is_downloaded => {
            // Model exists and is downloaded
            start_server_with_model(app_handle, model).await;
        }
        Some(model) => {
            // Model exists but is not downloaded
            notify_model_download_required(app_handle, &model.name);
        }
        None => {
            // Model not found
            notify_model_not_found(app_handle, model_id);
        }
    }
}

// Start the LLM server with the specified model
async fn start_server_with_model(app_handle: &AppHandle, model: ModelInfo) {
    println!("Starting LLM server with model: {}", model.name);

    // Create server
    match LLMServer::new(app_handle.clone()).await {
        Ok(mut server) => {
            // Set the model path
            if let Err(e) = server.set_model_path(&model.path).await {
                eprintln!("Error setting model path: {}", e);
                return;
            }

            // Start the server
            if let Err(e) = server.start().await {
                eprintln!("Error starting LLM server: {}", e);
                return;
            }

            // Store the server in app state
            let server_state = app_handle.state::<tokio::sync::Mutex<Option<LLMServer>>>();
            let mut server_guard = server_state.lock().await;
            *server_guard = Some(server);

            println!("LLM server started successfully");
        }
        Err(e) => {
            eprintln!("Failed to create LLM server: {}", e);
        }
    }
}

// Notification functions
fn notify_model_selection_required(app_handle: &AppHandle) {
    let _ = app_handle.emit(
        "model-selection-required",
        "Please select a model to use for AI features",
    );
}

fn notify_model_download_required(app_handle: &AppHandle, model_name: &str) {
    let _ = app_handle.emit(
        "model-download-required",
        format!(
            "The selected model '{}' needs to be downloaded before use",
            model_name
        ),
    );
}

fn notify_model_not_found(app_handle: &AppHandle, model_id: &str) {
    let _ = app_handle.emit(
        "model-selection-required",
        "The previously selected model is no longer available. Please select a new model.",
    );
}
