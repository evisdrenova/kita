use async_trait::async_trait;
use serde_json::{Map, Value};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tracing::debug;

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

/// Parser for JSON files
#[derive(Default)]
pub struct JsonChunker;

#[async_trait]
impl Chunker for JsonChunker {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["application/json"]
    }

    fn can_chunk_file_type(&self, path: &Path) -> bool {
        match util::detect_mime_type(path) {
            Ok(mime) => mime == "application/json",
            Err(_) => false,
        }
    }

    async fn chunk_file(
        &self,
        file: &FileMetadata,
        config: &ChunkerConfig,
        embedder: Arc<Embedder>,
    ) -> ChunkerResult<Vec<(Chunk, Vec<f32>)>> {
        debug!("Creating JSON chunks for file {:?}", file.base.path);

        let path = Path::new(&file.base.path);

        // Read the JSON file
        let mut file = File::open(path).await?;
        let mut content = String::new();
        file.read_to_string(&mut content).await?;

        // Parse the JSON content
        let json_value: Value = match serde_json::from_str(&content) {
            Ok(value) => value,
            Err(e) => {
                return Err(ChunkerError::Other(format!("Failed to parse JSON: {}", e)));
            }
        };

        // Generate chunks based on JSON structure
        let chunks = chunk_json_value(json_value, path, config)?;

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Process embeddings in a single batch
        tokio::task::spawn_blocking(move || {
            // Extract just the text content for embedding
            let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.content.as_str()).collect();

            // Generate embeddings
            match embedder.model.embed(texts, None) {
                Ok(embeddings) => {
                    // Pair chunks with their embeddings
                    let chunk_embeddings: Vec<(Chunk, Vec<f32>)> = chunks
                        .into_iter()
                        .zip(embeddings.into_iter())
                        .filter(|(_, embedding)| !embedding.is_empty())
                        .collect();

                    Ok(chunk_embeddings)
                }
                Err(_) => Err(ChunkerError::Other(
                    "Failed to generate embeddings".to_string(),
                )),
            }
        })
        .await
        .map_err(|e| ChunkerError::Other(format!("Thread error: {:?}", e)))?
    }
}

/// Function to chunk JSON values recursively
fn chunk_json_value(
    value: Value,
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    let mut chunks = Vec::new();

    match value {
        Value::Object(map) => {
            // Process JSON objects
            chunks.extend(chunk_json_object(map, path, config)?);
        }
        Value::Array(arr) => {
            // Process each array element
            for (idx, item) in arr.into_iter().enumerate() {
                let section = Some(format!("array_item_{}", idx));
                let item_chunks = process_json_value(item, path, config, section)?;
                chunks.extend(item_chunks);
            }
        }
        // For primitive values, just add as a single chunk
        _ => {
            let content = value.to_string();
            if !content.is_empty() {
                chunks.push(create_chunk(content, path, 0, Some(1), None));
            }
        }
    }

    // Update total chunks
    let total = chunks.len();
    if total > 0 {
        for (idx, chunk) in chunks.iter_mut().enumerate() {
            chunk.metadata.chunk_index = idx;
            chunk.metadata.total_chunks = Some(total);
        }
    }

    Ok(chunks)
}

/// Process JSON objects by breaking them down into meaningful chunks
fn chunk_json_object(
    map: Map<String, Value>,
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    let mut chunks = Vec::new();

    // Group related key-value pairs if possible
    if map.len() <= 5 {
        // Small objects can be kept together
        let content = serde_json::to_string_pretty(&Value::Object(map))
            .map_err(|e| ChunkerError::Other(format!("JSON serialization error: {}", e)))?;

        if !content.is_empty() {
            chunks.push(create_chunk(content, path, 0, None, None));
        }
    } else {
        // For larger objects, process each key-value pair
        for (key, value) in map {
            // Use the key as the section name
            let section = Some(key.clone());

            match value {
                Value::Object(_) | Value::Array(_) => {
                    // Recursively process complex values
                    let value_chunks = process_json_value(value, path, config, section.clone())?;
                    chunks.extend(value_chunks);
                }
                _ => {
                    // For primitive values, create a key-value pair representation
                    let content = format!("\"{}\" : {}", key, value);
                    if !content.is_empty() {
                        chunks.push(create_chunk(content, path, chunks.len(), None, section));
                    }
                }
            }
        }
    }

    Ok(chunks)
}

/// Helper to process any JSON value with section information
fn process_json_value(
    value: Value,
    path: &Path,
    config: &ChunkerConfig,
    section: Option<String>,
) -> ChunkerResult<Vec<Chunk>> {
    let mut value_chunks = chunk_json_value(value, path, config)?;

    // Update section if provided
    if let Some(section_name) = section {
        for chunk in &mut value_chunks {
            chunk.metadata.section = match &chunk.metadata.section {
                Some(existing) => Some(format!("{}.{}", section_name, existing)),
                None => Some(section_name.clone()),
            };
        }
    }

    Ok(value_chunks)
}

/// Helper to create a chunk with standard metadata
fn create_chunk(
    content: String,
    path: &Path,
    index: usize,
    total: Option<usize>,
    section: Option<String>,
) -> Chunk {
    Chunk {
        content,
        metadata: ChunkMetadata {
            source_path: path.to_path_buf(),
            chunk_index: index,
            total_chunks: total,
            page_number: None,
            section,
            mime_type: "application/json".to_string(),
        },
    }
}
