use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

/// Parser for plain text files
#[derive(Default)]
pub struct TxtChunker;

#[async_trait]
impl Chunker for TxtChunker {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["text/plain"]
    }

    fn can_chunk_file_type(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "txt" {
                return true;
            }
        }

        // Try to detect by MIME type
        match util::detect_mime_type(path) {
            Ok(mime) => mime == "text/plain",
            Err(_) => false,
        }
    }

    async fn chunk_file(
        &self,
        file: &FileMetadata,
        config: &ChunkerConfig,
        embedder: Arc<Embedder>,
    ) -> ChunkerResult<Vec<(Chunk, Vec<f32>)>> {
        println!("creating chunk for file {:?}", file.base.path);
        let path = Path::new(&file.base.path);

        // Get chunks based on file size
        let chunks = if file.size > 10_000_000 {
            // For large files, use streaming approach
            get_chunks_from_large_file(path, config).await?
        } else {
            // For smaller files, read all at once
            get_chunks_from_small_file(path, config).await?
        };

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Process embeddings in a single batch
        tokio::task::spawn_blocking(move || {
            // Extract just the text content for embedding and convert from chunks to strings
            let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.content.as_str()).collect();

            // Generate embeddings in one batch call
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

/// Handle very large files in a streaming fashion
async fn get_chunks_from_large_file(
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    debug!("Processing large file: {}", path.display());

    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut line_count = 0;
    let mut chunk_idx = 0;

    // Read and process line by line
    while let Some(line) = lines.next_line().await? {
        buffer.push_str(&line);
        buffer.push('\n');
        line_count += 1;

        // Process when enough lines accumulate
        if line_count >= config.chunk_size {
            let normalized = if config.normalize_text {
                util::normalize_text(&buffer)
            } else {
                buffer.clone()
            };

            // Create chunk
            chunks.push(Chunk {
                content: normalized,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: chunk_idx,
                    total_chunks: None, // Will update later
                    page_number: None,
                    section: None,
                    mime_type: "text/plain".to_string(),
                },
            });

            // Handle overlap
            if config.chunk_overlap > 0 && config.chunk_overlap < line_count {
                // Keep overlap lines
                let mut newlines_found = 0;
                let mut overlap_pos = buffer.len();

                for (i, c) in buffer.chars().rev().enumerate() {
                    if c == '\n' {
                        newlines_found += 1;
                        if newlines_found >= config.chunk_overlap {
                            overlap_pos = buffer.len() - i - 1;
                            break;
                        }
                    }
                }

                buffer = buffer[overlap_pos..].to_string();
                line_count = buffer.lines().count();
            } else {
                buffer.clear();
                line_count = 0;
            }

            chunk_idx += 1;
        }
    }

    // Process remaining content
    if !buffer.is_empty() {
        let normalized = if config.normalize_text {
            util::normalize_text(&buffer)
        } else {
            buffer
        };

        chunks.push(Chunk {
            content: normalized,
            metadata: ChunkMetadata {
                source_path: path.to_path_buf(),
                chunk_index: chunk_idx,
                total_chunks: None,
                page_number: None,
                section: None,
                mime_type: "text/plain".to_string(),
            },
        });
    }

    // Update total_chunks
    let total = chunks.len();
    if total > 0 {
        for chunk in &mut chunks {
            chunk.metadata.total_chunks = Some(total);
        }
    }

    Ok(chunks)
}

/// Split text into chunks with optional overlap
async fn get_chunks_from_small_file(
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    // Read the entire file
    let content = tokio::fs::read_to_string(path).await?;

    // Process content
    let processed_content = if config.normalize_text {
        util::normalize_text(&content)
    } else {
        content
    };

    // Create text chunks
    let text_chunks = chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

    if text_chunks.is_empty() {
        return Ok(Vec::new());
    }

    // Create chunks
    let total_chunks = text_chunks.len();
    let chunks = text_chunks
        .into_iter()
        .enumerate()
        .map(|(idx, content)| Chunk {
            content,
            metadata: ChunkMetadata {
                source_path: path.to_path_buf(),
                chunk_index: idx,
                total_chunks: Some(total_chunks),
                page_number: None,
                section: None,
                mime_type: "text/plain".to_string(),
            },
        })
        .collect();

    Ok(chunks)
}

/// Chunks texts based on a configured chunk_size and overlap
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    // gets all of the words in the file and collects them into a vector
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![text.to_string()];
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut start: usize = 0;

    while start < words.len() {
        // if the total amount of words is less than the chunk size then just return the entire text
        // otherwise create a chunk of the chunk size + the start position and put it into the vector
        let end: usize = std::cmp::min(start + chunk_size, words.len());
        let chunk: String = words[start..end].join(" ");
        chunks.push(chunk);

        // Calculate next position with overlap
        if end == words.len() {
            break; // We've reached the end
        } else {
            // Move forward by (chunk_size - overlap)
            start = std::cmp::min(start + chunk_size - overlap, words.len() - 1);
        }
    }
    chunks
}
