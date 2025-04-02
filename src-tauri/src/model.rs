use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use dirs;
use reqwest::Client;
use tauri::{AppHandle, Manager};
use thiserror::Error;

const SERVER_PORT: u16 = 8080;
const SERVER_BINARY_NAME: &str = "llama-server";

// !! Define your model filename in Downloads folder !!
const MODEL_FILENAME: &str = "mistral-7b-instruct-v0.2.Q5_K_M.gguf";

const SERVER_READY_TIMEOUT_SECS: u64 = 180;

#[derive(Error, Debug)]
pub enum LLMServerError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Command execution error: {0}")]
    CommandError(String),

    #[error("Could not find Downloads directory")]
    DownloadsDirNotFound,

    #[error("Server did not become ready within timeout ({0}s)")]
    ServerReadyTimeout(u64),

    #[error("Could not extract server binary")]
    ResourceExtractionError,
}

#[derive(serde::Serialize, Debug)]
struct CompletionRequest {
    prompt: String,
    n_predict: i32,
    temperature: f32,
    stop: Vec<String>,
}

#[derive(serde::Deserialize, Debug)]
struct CompletionResponse {
    content: String,
}
pub struct LLMServer {
    server_process: Option<tokio::process::Child>,
    port: u16,
    app_handle: AppHandle,
    model_path: Option<PathBuf>,
}

impl LLMServer {
    pub async fn new(app_handle: AppHandle) -> Result<Self, LLMServerError> {
        Ok(Self {
            server_process: None,
            port: SERVER_PORT,
            app_handle,
            model_path: None,
        })
    }

    pub async fn start(&mut self) -> Result<(), LLMServerError> {
        // Check if we have a model path set
        let model_path = if let Some(path) = &self.model_path {
            path.clone()
        } else {
            // Fallback to default behavior if no model path is set
            let downloads_dir = dirs::download_dir().ok_or(LLMServerError::DownloadsDirNotFound)?;
            downloads_dir.join(MODEL_FILENAME)
        };

        println!("Using model path: {}", model_path.display());

        // Verify the model exists
        if !model_path.exists() {
            return Err(LLMServerError::CommandError(format!(
                "Model file not found at: {}",
                model_path.display()
            )));
        }

        // Prepare the server binary from resources
        let server_path = self.prepare_server_binary().await?;
        println!("Using server binary: {}", server_path.display());

        // Start the server
        let child = self.start_server(&server_path, &model_path).await?;
        self.server_process = Some(child);

        // Poll for server readiness
        let ready_timeout = Duration::from_secs(SERVER_READY_TIMEOUT_SECS);
        match timeout(ready_timeout, self.wait_for_server_ready()).await {
            Ok(Ok(_)) => {
                println!("Server is ready!");
                Ok(())
            }
            Ok(Err(e)) => {
                eprintln!("Error during server readiness check: {}", e);
                // Kill the server process if it's still running
                if let Some(mut child) = self.server_process.take() {
                    let _ = child.start_kill();
                }
                Err(e)
            }
            Err(_) => {
                eprintln!(
                    "Server did not become ready within {} seconds.",
                    SERVER_READY_TIMEOUT_SECS
                );
                // Kill the server process if it's still running
                if let Some(mut child) = self.server_process.take() {
                    let _ = child.start_kill();
                }
                Err(LLMServerError::ServerReadyTimeout(
                    SERVER_READY_TIMEOUT_SECS,
                ))
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

    pub async fn send_prompt(&self, prompt: &str) -> Result<String, LLMServerError> {
        let response = self.send_completion_request(prompt).await?;
        Ok(response.content)
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
        println!("Checking for server binary in current directory...");

        // First try the src-tauri/resources path (for development)
        let cwd_path = std::env::current_dir()?
            .join("resources")
            .join(SERVER_BINARY_NAME);

        println!("Looking for binary at: {}", cwd_path.display());

        if cwd_path.exists() {
            println!(
                "Found server binary in development resources: {}",
                cwd_path.display()
            );

            // Check if we need to set executable permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&cwd_path)?.permissions();
                if perms.mode() & 0o111 == 0 {
                    // Set executable permissions if not already set
                    perms.set_mode(0o755); // rwxr-xr-x
                    fs::set_permissions(&cwd_path, perms)?;
                    println!("Set executable permissions on development binary");
                }
            }

            return Ok(cwd_path);
        }

        // Try the resource directory (for production)
        // should handle this with an envar
        if let Ok(resource_dir) = self.app_handle.path().resource_dir() {
            let resource_path = resource_dir.join(SERVER_BINARY_NAME);
            println!(
                "Looking for binary at resource path: {}",
                resource_path.display()
            );

            if resource_path.exists() {
                println!(
                    "Found server binary in resources: {}",
                    resource_path.display()
                );

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
                // Add other args as needed
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

    async fn wait_for_server_ready(&self) -> Result<(), LLMServerError> {
        let client = Client::new();

        // Try both /health and root endpoint
        let endpoints = vec![
            format!("http://127.0.0.1:{}/health", self.port),
            format!("http://127.0.0.1:{}", self.port),
        ];

        println!("Waiting for server to become ready...");

        loop {
            // Sleep before checking to give the server some time
            tokio::time::sleep(Duration::from_millis(1000)).await;

            let mut success = false;

            // Try each endpoint
            for endpoint in &endpoints {
                match client.get(endpoint).send().await {
                    Ok(response) => {
                        if response.status().is_success() {
                            println!("Server is ready at {}", endpoint);
                            success = true;
                            break;
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

            if success {
                return Ok(());
            }
        }
    }

    async fn send_completion_request(
        &self,
        prompt: &str,
    ) -> Result<CompletionResponse, LLMServerError> {
        let client = Client::new();
        let url = format!("http://127.0.0.1:{}/completion", self.port);

        let request = CompletionRequest {
            prompt: prompt.to_string(),
            n_predict: 150,
            temperature: 0.7,
            stop: vec!["\nHuman:".to_string(), "\nUser:".to_string()],
        };

        println!("Sending prompt to {}...", url);

        // Print the request payload for debugging
        println!(
            "Request payload: {:?}",
            serde_json::to_string(&request).unwrap_or_default()
        );

        let response = client.post(&url).json(&request).send().await?;

        if response.status().is_success() {
            Ok(response.json::<CompletionResponse>().await?)
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

impl Drop for LLMServer {
    fn drop(&mut self) {
        if let Some(mut child) = self.server_process.take() {
            println!("Automatically stopping server on drop...");
            let _ = child.start_kill();
        }
    }
}

// Example of how to use this in a Tauri command
#[tauri::command]
pub async fn ask_llm(app_handle: AppHandle, prompt: String) -> Result<String, String> {
    println!("Incoming prompt: {:?}", prompt);

    // Get the server state
    let server_state = app_handle.state::<tokio::sync::Mutex<Option<LLMServer>>>();
    let server_guard = server_state.lock().await;

    // Check if we have a server instance
    if let Some(server) = &*server_guard {
        // Use the existing server to send the prompt
        server
            .send_prompt(&prompt)
            .await
            .map_err(|e| format!("Failed to get response: {}", e))
    } else {
        // No server instance available
        Err("No LLM server is currently running. Please select a model first.".into())
    }
}

pub fn register_llm_commands(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    app.manage(tokio::sync::Mutex::new(None::<LLMServer>));
    Ok(())
}

// #[tauri::command]
// pub async fn change_llm_model(app_handle: AppHandle, model_id: String) -> Result<(), String> {
//     // Get model info
//     let registry_state = app_handle.state::<ModelRegistry>();
//     let model = registry_state
//         .get_model(&model_id)
//         .ok_or_else(|| "Model not found".to_string())?;

//     if !model.is_downloaded {
//         return Err("Selected model is not downloaded".to_string());
//     }

//     // Get the server state
//     let server_state = app_handle.state::<tokio::sync::Mutex<Option<LLMServer>>>();
//     let mut server_guard = server_state.lock().await;

//     // Restart the server with the new model
//     if let Some(mut server) = server_guard.take() {
//         // Stop existing server
//         if let Err(e) = server.stop().await {
//             eprintln!("Error stopping LLM server: {}", e);
//             // Continue anyway, we'll try to start a new one
//         }
//     }

//     // Create a new server
//     let mut new_server = LLMServer::new(app_handle.clone())
//         .await
//         .map_err(|e| format!("Failed to create server: {}", e))?;

//     // Set the model path
//     new_server
//         .set_model_path(&model.path)
//         .await
//         .map_err(|e| format!("Failed to set model path: {}", e))?;

//     // Start the server
//     new_server
//         .start()
//         .await
//         .map_err(|e| format!("Failed to start server: {}", e))?;

//     // Store the new server
//     *server_guard = Some(new_server);

//     // Update the selected model in settings
//     let settings_state = app_handle.state::<SettingsManagerState>();
//     settings_state
//         .0
//         .set_selected_model(Some(model_id))
//         .map_err(|e| format!("Failed to update settings: {}", e))?;

//     Ok(())
// }
