use async_trait::async_trait;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};
use tracing::debug;

use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::util;
use super::Chunker;

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
    ) -> ChunkerResult<Vec<Chunk>> {
        // For very large files, use streaming approach
        if file.size > 10_000_000 {
            // 10MB threshold
            return chunk_large_file(path, config).await;
        }

        // For smaller files, read all at once
        let content = tokio::fs::read_to_string(path).await?;

        // Process content
        let processed_content = if config.normalize_text {
            util::normalize_text(&content)
        } else {
            content
        };

        // Chunk the text
        let chunks = chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

        // Early return for empty chunks
        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Create metadata-enriched chunks
        let total_chunks = chunks.len();
        let result = chunks
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

        Ok(result)
    }
}

/// Handle very large files in a streaming fashion
async fn chunk_large_file(path: &Path, config: &ChunkerConfig) -> ChunkerResult<Vec<Chunk>> {
    debug!("Streaming large file: {}", path.display());

    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut line_count = 0;
    let mut chunk_idx = 0;

    // Read line by line
    while let Some(line) = lines.next_line().await? {
        buffer.push_str(&line);
        buffer.push('\n');
        line_count += 1;

        // Process when we've accumulated enough lines
        if line_count >= config.chunk_size {
            let normalized = if config.normalize_text {
                util::normalize_text(&buffer)
            } else {
                buffer.clone()
            };

            // Add chunk
            chunks.push(Chunk {
                content: normalized,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: chunk_idx,
                    total_chunks: None, // Will update after all chunks are collected
                    page_number: None,
                    section: None,
                    mime_type: "text/plain".to_string(),
                },
            });

            // Handle overlap
            if config.chunk_overlap > 0 && config.chunk_overlap < line_count {
                // Keep last N lines for overlap
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

    // Process any remaining content
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

    // Update total_chunks for all chunks
    let total = chunks.len();
    if total > 0 {
        for chunk in &mut chunks {
            chunk.metadata.total_chunks = Some(total);
        }
    }

    Ok(chunks)
}

/// Split text into chunks with optional overlap
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < words.len() {
        let end = std::cmp::min(start + chunk_size, words.len());
        let chunk = words[start..end].join(" ");
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
