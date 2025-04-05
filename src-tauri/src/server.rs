/*
This file contains methods and functions to interact with the llama.cpp server that is serving the LLM model */

use dirs;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::model_registry::{ModelInfo, ModelRegistry, ModelRegistryError};
use crate::settings::SettingsManagerState;
use crate::vectordb_manager::{get_text_chunks_from_similarity_search, VectorDbManager};

#[derive(Error, Debug)]
pub enum LLMServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Command execution error: {0}")]
    CommandError(String),

    #[error("Could not find Downloads directory")]
    DownloadsDirNotFound,

    #[error("Server did not become ready within timeout ({0}s)")]
    ServerReadyTimeout(u64),
}

#[derive(Debug, Deserialize, Serialize)]
struct CompletionRequest {
    prompt: String,
    n_predict: i32,
    temperature: f32,
    stop: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CompletionResponse {
    pub content: String,
    pub sources: Vec<String>,
}

type Result<T, E = LLMServerError> = std::result::Result<T, E>;

pub struct LLMServer {
    server_process: Option<tokio::process::Child>,
    port: u16,
    app_handle: AppHandle,
    model_path: Option<PathBuf>,
}

const SERVER_PORT: u16 = 8080;
const SERVER_BINARY_NAME: &str = "llama-server";
const SERVER_READY_TIMEOUT_SECS: u64 = 180;

impl LLMServer {
    pub async fn new(app_handle: AppHandle) -> Result<Self, LLMServerError> {
        Ok(Self {
            server_process: None,
            port: SERVER_PORT,
            app_handle,
            model_path: None,
        })
    }

    pub async fn start(&mut self, model_name: &str) -> Result<(), LLMServerError> {
        // Check if we have a model path set
        let model_path = if let Some(path) = &self.model_path {
            path.clone()
        } else {
            // Fallback to default behavior if no model path is set
            let downloads_dir = dirs::download_dir().ok_or(LLMServerError::DownloadsDirNotFound)?;
            downloads_dir.join(model_name)
        };

        // Verify the model exists
        if !model_path.exists() {
            return Err(LLMServerError::CommandError(format!(
                "Model file not found at: {}",
                model_path.display()
            )));
        }

        let server_path = self.prepare_server_binary().await?;

        // Start the server
        let child = self.start_server(&server_path, &model_path).await?;
        self.server_process = Some(child);

        // Poll for server readiness
        let ready_timeout = Duration::from_secs(SERVER_READY_TIMEOUT_SECS);
        match timeout(ready_timeout, self.wait_for_server_ready()).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                eprintln!("Error during server readiness check: {}", e);
                let _ = self.stop();
                Err(e)
            }
            Err(_) => {
                eprintln!(
                    "Server did not become ready within {} seconds.",
                    SERVER_READY_TIMEOUT_SECS
                );
                let _ = self.stop();
                Err(LLMServerError::ServerReadyTimeout(
                    SERVER_READY_TIMEOUT_SECS,
                ))
            }
        }
    }

    pub async fn stop(&mut self) -> Result<(), LLMServerError> {
        if let Some(mut child) = self.server_process.take() {
            println!("Stopping server...");
            let _ = child.start_kill();
            // Give it a moment to shut down
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    async fn prepare_server_binary(&self) -> Result<PathBuf, LLMServerError> {
        // First try the src-tauri/resources path (for development)
        let cwd_path = std::env::current_dir()?
            .join("resources")
            .join(SERVER_BINARY_NAME);

        if cwd_path.exists() {
            // Check if we need to set executable permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&cwd_path)?.permissions();
                if perms.mode() & 0o111 == 0 {
                    // Set executable permissions if not already set
                    perms.set_mode(0o755); // rwxr-xr-x
                    fs::set_permissions(&cwd_path, perms)?;
                }
            }

            return Ok(cwd_path);
        }

        // Try the resource directory (for production)
        // should handle this with an envar
        if let Ok(resource_dir) = self.app_handle.path().resource_dir() {
            let resource_path = resource_dir.join(SERVER_BINARY_NAME);
            if resource_path.exists() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&resource_path)?.permissions();
                    if perms.mode() & 0o111 == 0 {
                        perms.set_mode(0o755);
                        fs::set_permissions(&resource_path, perms)?;
                    }
                }

                return Ok(resource_path);
            }
        }

        // Binary not found
        Err(LLMServerError::CommandError(format!(
            "Could not find {}. Searched in src-tauri/resources and resource directory.",
            SERVER_BINARY_NAME
        )))
    }

    async fn start_server(
        &self,
        server_path: &Path,
        model_path: &Path,
    ) -> Result<tokio::process::Child, LLMServerError> {
        println!(
            "Starting server: {} with model: {} on port {}",
            server_path.display(),
            model_path.display(),
            self.port
        );

        let model_path_str = model_path
            .to_str()
            .ok_or_else(|| LLMServerError::CommandError("Invalid model path".into()))?;

        let mut command = Command::new(server_path);
        command
            .args([
                "-m",
                model_path_str,
                "--port",
                &self.port.to_string(),
                "--host",
                "127.0.0.1",
                "-c",
                "2048",
                "--no-system-prompt", // Add other args as needed
                                      // "--threads", "4",  // Uncomment and adjust based on your CPU
                                      // "--log-disable",   // Uncomment to reduce noise
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|e| LLMServerError::CommandError(format!("Failed to spawn server: {}", e)))?;

        println!("Server process started (PID: {})", child.id().unwrap_or(0));

        // Capture and print server output
        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = reader.next_line().await {
                    println!("[SERVER]: {}", line);
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let mut reader = BufReader::new(stderr).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = reader.next_line().await {
                    eprintln!("[SERVER ERROR]: {}", line);
                }
            });
        }

        Ok(child)
    }

    /// checks /health endpoint to see if server is ready
    async fn wait_for_server_ready(&self) -> Result<(), LLMServerError> {
        let client = Client::new();

        let endpoint = format!("http://127.0.0.1:{}/health", self.port);

        println!("Waiting for server to become ready...");

        loop {
            // Sleep for 1s before checking to give the server some time
            tokio::time::sleep(Duration::from_millis(1000)).await;

            match client.get(&endpoint).send().await {
                Ok(response) => {
                    if response.status().is_success() {
                        println!("Server is ready at {}", endpoint);
                        return Ok(());
                    }
                    println!(
                        "Server responded with status: {} at {}",
                        response.status(),
                        endpoint
                    );
                }
                Err(e) => {
                    println!("Server not ready at {}: {}", endpoint, e);
                }
            }
        }
    }

    pub async fn set_model_path(&mut self, path: &str) -> Result<(), LLMServerError> {
        let model_path = PathBuf::from(path);

        // Verify the model exists
        if !model_path.exists() {
            return Err(LLMServerError::CommandError(format!(
                "Model file not found at: {}",
                model_path.display()
            )));
        }

        self.model_path = Some(model_path);
        Ok(())
    }

    fn stop_sync(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            println!("Stopping server synchronously...");
            let _ = child.start_kill();
            // We can't wait asynchronously here, but that's usually okay
            // as the OS will clean up child processes
        }
    }

    pub async fn send_prompt(&self, prompt: &str, chunks: &str) -> Result<String, LLMServerError> {
        let response = self.send_completion_request(prompt, chunks).await?;
        Ok(response.content)
    }

    async fn send_completion_request(
        &self,
        prompt: &str,
        chunks: &str,
    ) -> Result<CompletionResponse, LLMServerError> {
        let client = Client::new();
        let url = format!("http://127.0.0.1:{}/completion", self.port);

        let system_prompt = "
        You are a helpful, accurate, and concise assistant. Your task is to answer questions based ONLY on the provided context.

        When answering:
        1. Use ONLY information from the provided context
        2. If the context doesn't contain the answer, say \"I don't have enough information to answer that\" - never make up information
        3. Cite your sources by referring to the document numbers [1], [2], etc.
        4. Keep responses concise and to the point
        5. If there are contradictions in the context, acknowledge them
        6. Format your response in a readable way using markdown when helpful

        Always prioritize accuracy over comprehensiveness.";

        let formatted_prompt = format!(
            "<s>[INST] {}\n\nCONTEXT:\n{}\n\nQUESTION: {} [/INST]",
            system_prompt, chunks, prompt
        );

        let request = CompletionRequest {
            prompt: formatted_prompt,
            n_predict: 150,
            temperature: 0.7,
            stop: vec!["\nHuman:".to_string(), "\nUser:".to_string()],
        };

        let ready_timeout = Duration::from_secs(5);
        match timeout(ready_timeout, self.wait_for_server_ready()).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(e);
            }
            Err(_) => {
                return Err(LLMServerError::ServerReadyTimeout(30));
            }
        }

        let response = client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            let completion_response = response.json::<CompletionResponse>().await?;

            println!("the competition response: {:?}", completion_response);

            // Extract sources from the response to tell which sources the LLM used to find the answer, the sources == file_id
            let sources = extract_sources(&completion_response.content);

            // Create enhanced response with sources
            let enhanced_response = CompletionResponse {
                content: completion_response.content,
                sources,
            };

            Ok(enhanced_response)
        } else {
            let status = response.status();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Could not read error body".to_string());

            Err(LLMServerError::CommandError(format!(
                "Server returned error {}: {}",
                status, error_body
            )))
        }
    }
}

/// initializes the server with the model
pub fn initialize_server(app: &mut tauri::App) -> Result<()> {
    let registry_exists = app.try_state::<ModelRegistry>().is_some();

    if !registry_exists {
        let registry = ModelRegistry::new();
        registry.initialize();

        // Add registry to the app state
        app.manage(registry);
        println!("Model registry initialized in start_server");
    }

    // Launch background scan and server initialization
    let app_handle = app.app_handle().clone();

    tauri::async_runtime::spawn(async move {
        let registry_state = app_handle.state::<ModelRegistry>();
        // scan for any downlaoded models
        registry_state
            .scan_downloaded_models(&app_handle, None)
            .map_err(|e| {
                ModelRegistryError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Unable to scan downloaded models: {}", e),
                ))
            });

        // get the user selected model and load it down below
        let selected_model_id = match get_selected_model_from_settings(&app_handle) {
            Ok(Some(id)) => id,
            Ok(None) => {
                notify_model_selection_required(&app_handle);
                return;
            }
            Err(e) => {
                eprintln!("Error getting settings: {}", e);
                return;
            }
        };

        // Try to load the selected model
        load_selected_model(&app_handle, &selected_model_id).await;
    });

    Ok(())
}

/// Get the selected model ID from settings
fn get_selected_model_from_settings(app_handle: &AppHandle) -> Result<Option<String>, String> {
    let settings_state = app_handle.state::<SettingsManagerState>();
    let settings = settings_state.0.get_settings().map_err(|e| e.to_string())?;

    Ok(settings.selected_model_id)
}

// load the model in the server
async fn load_selected_model(app_handle: &AppHandle, model_id: &str) {
    let registry_state = app_handle.state::<ModelRegistry>();

    match registry_state.get_model(model_id) {
        Some(model) if model.is_downloaded => {
            // Model exists and is downloaded
            start_server_with_model(app_handle, model).await;
        }
        Some(model) => {
            // Model exists but is not downloadeds
            notify_model_download_required(app_handle, &model.name);
        }
        None => {
            // Model not found
            notify_model_not_found(app_handle);
        }
    }
}

// Start the LLM server with the specified model
async fn start_server_with_model(app_handle: &AppHandle, model: ModelInfo) {
    // Create server
    match LLMServer::new(app_handle.clone()).await {
        Ok(mut server) => {
            // Set the model path
            if let Err(e) = server.set_model_path(&model.path).await {
                eprintln!("Error setting model path: {}", e);
                return;
            }

            // Start the server
            if let Err(e) = server.start(&model.name).await {
                eprintln!("Error starting LLM server: {}", e);
                return;
            }

            // Store the server in app state
            let server_state = app_handle.state::<tokio::sync::Mutex<Option<LLMServer>>>();
            let mut server_guard = server_state.lock().await;
            *server_guard = Some(server);

            println!("LLM server initialized");
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

fn notify_model_not_found(app_handle: &AppHandle) {
    let _ = app_handle.emit(
        "model-selection-required",
        "The previously selected model is no longer available. Please select a new model.",
    );
}

impl Drop for LLMServer {
    fn drop(&mut self) {
        self.stop_sync();
    }
}

pub fn register_llm_commands(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(tokio::sync::Mutex::new(None::<LLMServer>));
    Ok(())
}

// Example of how to use this in a Tauri command
#[tauri::command]
pub async fn ask_llm(app_handle: AppHandle, prompt: String) -> Result<String, String> {
    println!("Incoming prompt: {:?}", prompt);

    // Get the server state
    let server_state = app_handle.state::<tokio::sync::Mutex<Option<LLMServer>>>();
    let server_guard = server_state.lock().await;

    let text_chunks: String = match VectorDbManager::search_similar(&app_handle, &prompt).await {
        Ok(results) => get_text_chunks_from_similarity_search(results)?,
        Err(e) => {
            eprintln!("Unable to get chunks): {}", e);
            String::new()
        }
    };

    println!("the text chunks: {:?}", text_chunks);

    // Check if we have a server instance
    if let Some(server) = &*server_guard {
        server
            .send_prompt(&prompt, &text_chunks)
            .await
            .map_err(|e| format!("Failed to get response: {}", e))
    } else {
        Err("No LLM server is currently running. Please select a model first.".into())
    }
}

fn extract_sources(text: &str) -> Vec<String> {
    // Look for patterns like [1] <source>3</source> or just <source>3</source>
    let re = Regex::new(r"<source>(.*?)</source>").unwrap();

    // Find all unique sources
    let mut sources = Vec::new();
    for cap in re.captures_iter(text) {
        if let Some(source) = cap.get(1) {
            let source_id = source.as_str().to_string();
            if !sources.contains(&source_id) {
                sources.push(source_id);
            }
        }
    }

    println!("the sources: {:?}", sources);

    sources
}
