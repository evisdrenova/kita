use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use tracing::{debug, instrument};

use super::common::{ChunkMetadata, ParsedChunk, ParserConfig, ParserError, ParserResult};
use super::util;
use super::Parser;

/// Parser for plain text files
#[derive(Default)]
pub struct TxtParser;

#[async_trait]
impl Parser for TxtParser {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["text/plain"]
    }

    fn can_parse_file_type(&self, path: &Path) -> bool {
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

    async fn parse(&self, path: &Path, config: &ParserConfig) -> ParserResult<Vec<ParsedChunk>> {
        debug!("Parsing text file: {}", path.display());

        // Open the file
        let file: File = File::open(path).await?;
        let reader: BufReader<File> = BufReader::new(file);

        // Read the entire file content
        let mut lines: io::Lines<BufReader<File>> = reader.lines();
        let mut content = String::new();

        while let Some(line) = lines.next_line().await? {
            content.push_str(&line);
            content.push('\n');
        }

        // Normalize text if configured
        let processed_content: String = if config.normalize_text {
            util::normalize_text(&content)
        } else {
            content
        };

        // Chunk the text
        let chunks = util::chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

        // Create ParsedChunk objects
        let result = chunks
            .into_iter()
            .enumerate()
            .map(|(idx, chunk_content)| {
                ParsedChunk {
                    content: chunk_content,
                    metadata: ChunkMetadata {
                        source_path: path.to_path_buf(),
                        chunk_index: idx,
                        total_chunks: None, // Will be updated after all chunks are created
                        page_number: None,
                        section: None,
                        mime_type: "text/plain".to_string(),
                    },
                }
            })
            .collect::<Vec<_>>();

        // Update total_chunks
        let total = result.len();
        let result = result
            .into_iter()
            .map(|mut chunk| {
                chunk.metadata.total_chunks = Some(total);
                chunk
            })
            .collect();

        Ok(result)
    }
}

// Implement a streaming version for better memory efficiency with large files
pub async fn stream_text_file(
    path: &Path,
) -> ParserResult<impl tokio::stream::Stream<Item = io::Result<String>>> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    Ok(reader.lines())
}

// Function to read text files with very large content in a memory-efficient way
pub async fn process_large_text_file<F, Fut>(
    path: &Path,
    config: &ParserConfig,
    mut processor: F,
) -> ParserResult<()>
where
    F: FnMut(String, usize) -> Fut,
    Fut: std::future::Future<Output = ParserResult<()>>,
{
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut buffer = String::new();
    let mut line_count = 0;
    let mut chunk_idx = 0;

    while let Some(line) = lines.next_line().await? {
        buffer.push_str(&line);
        buffer.push('\n');
        line_count += 1;

        // Once we've accumulated enough lines, process them
        if line_count >= config.chunk_size {
            let normalized = if config.normalize_text {
                util::normalize_text(&buffer)
            } else {
                buffer.clone()
            };

            processor(normalized, chunk_idx).await?;

            // Keep overlap lines if configured
            if config.chunk_overlap > 0 && config.chunk_overlap < line_count {
                // Extract the last N lines for overlap
                let overlap_start = buffer.len();
                let mut newlines_found = 0;

                for (i, c) in buffer.chars().rev().enumerate() {
                    if c == '\n' {
                        newlines_found += 1;
                        if newlines_found >= config.chunk_overlap {
                            let overlap_start = buffer.len() - i - 1;
                            buffer = buffer[overlap_start..].to_string();
                            break;
                        }
                    }
                }

                // If we couldn't find enough newlines, just clear the buffer
                if newlines_found < config.chunk_overlap {
                    buffer.clear();
                }
            } else {
                buffer.clear();
            }

            line_count = buffer.lines().count();
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

        processor(normalized, chunk_idx).await?;
    }

    Ok(())
}
