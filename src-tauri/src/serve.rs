use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Model not found: {0}")]
    NotFound(String),

    #[error("Download failed: {0}")]
    DownloadFailed(String),

    #[error("Invalid model configuration")]
    InvalidConfig,
}

type Result<T> = std::result::Result<T, ModelError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HuggingFaceModelInfo {
    id: String,
    name: String,
    repo_id: String,
    filename: String,
    size: u64, // Size in MB
    quantization: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub size: u64, // Size in MB
    pub path: String,
    pub quantization: String,
    pub is_downloaded: bool,
}

// struct to maintain available and downloaded models
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

    pub fn find_downloaded_models(&self, app_handle: &AppHandle) -> Result<()> {
        let models_dir = get_models_dir(app_handle)?;
        let mut downloaded = Vec::new(); //Vec<ModelInfo>

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
}
