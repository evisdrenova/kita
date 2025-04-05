/* This file contains methods and functions that interact with the model itself. All of the server functions are in server.rs */

use regex::Regex;
use reqwest::Client;
use tauri::{AppHandle, Manager};
use thiserror::Error;

use crate::vectordb_manager::get_text_chunks_from_similarity_search;
use crate::vectordb_manager::VectorDbManager;

#[derive(serde::Serialize, Debug)]
struct CompletionRequest {
    prompt: String,
    n_predict: i32,
    temperature: f32,
    stop: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CompletionResponse {
    pub content: String,
    pub sources: Vec<String>,
}

#[derive(Error, Debug)]
pub enum LLMModelError {
    #[error("Download failed: {0}")]
    CompletionError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Command execution error: {0}")]
    CommandError(String),
}

pub struct LLMModel {}

impl LLMModel {
    pub async fn send_prompt(&self, prompt: &str, chunks: &str) -> Result<String, LLMModelError> {
        let response = self.send_completion_request(prompt, chunks).await?;
        Ok(response.content)
    }

    async fn send_completion_request(
        &self,
        prompt: &str,
        chunks: &str,
    ) -> Result<CompletionResponse, LLMModelError> {
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
        println!("Sending prompt to {}...", url);

        // Print the request payload for debugging
        println!(
            "Request payload: {:?}",
            serde_json::to_string(&request).unwrap_or_default()
        );

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

            Err(LLMModelError::CommandError(format!(
                "Server returned error {}: {}",
                status, error_body
            )))
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
