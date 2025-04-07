use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

// Parser for markdown files
#[derive(Default)]
pub struct MarkdownChunker;

#[async_trait]
impl Chunker for MarkdownChunker {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["text/markdown", "text/x-markdown"]
    }

    fn can_chunk_file_type(&self, path: &Path) -> bool {
        match util::detect_mime_type(path) {
            Ok(mime) => {
                mime == "text/markdown" || mime == "text/x-markdown" || mime == "text/plain"
            }
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
                Err(_) => Err(ChunkerError::TextFileError(
                    "Failed to generate embeddings".to_string(),
                )),
            }
        })
        .await
        .map_err(|e| ChunkerError::TextFileError(format!("Thread error: {:?}", e)))?
    }
}

/// Handle very large files in a streaming fashion
async fn get_chunks_from_large_file(
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    let file = File::open(path).await?;
    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let mut chunks = Vec::new();
    let mut buffer = String::new();
    let mut current_section = String::new();
    let mut line_count = 0;
    let mut chunk_idx = 0;

    // Read and process line by line
    while let Some(line) = lines.next_line().await? {
        // Check for markdown headers to identify sections
        if line.starts_with("#") {
            // If we encounter a new section and have content in our buffer,
            // create a chunk from the existing content first
            if !buffer.is_empty() && line_count > 0 {
                let normalized = if config.normalize_text {
                    util::normalize_text(&buffer)
                } else {
                    buffer.clone()
                };

                // Create chunk with section information
                chunks.push(Chunk {
                    content: normalized,
                    metadata: ChunkMetadata {
                        source_path: path.to_path_buf(),
                        chunk_index: chunk_idx,
                        total_chunks: None, // Will update later
                        page_number: None,
                        section: Some(current_section.clone()),
                        mime_type: "text/markdown".to_string(),
                    },
                });

                chunk_idx += 1;

                // Keep overlap if configured
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
            }

            // Extract the header text (removing # symbols)
            let header_text = line.trim_start_matches(|c| c == '#' || c == ' ').trim();
            current_section = header_text.to_string();
        }

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
                    section: Some(current_section.clone()),
                    mime_type: "text/markdown".to_string(),
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
                section: Some(current_section),
                mime_type: "text/markdown".to_string(),
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

    // Extract sections from markdown
    let sections = extract_markdown_sections(&content);

    let mut chunks = Vec::new();
    let mut chunk_idx = 0;

    // Process each section
    for (section_title, section_content) in sections {
        let processed_content = if config.normalize_text {
            util::normalize_text(&section_content)
        } else {
            section_content
        };

        // Create text chunks for this section
        let text_chunks =
            util::chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

        for content in text_chunks {
            chunks.push(Chunk {
                content,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: chunk_idx,
                    total_chunks: None, // Will update after processing all
                    page_number: None,
                    section: Some(section_title.clone()),
                    mime_type: "text/markdown".to_string(),
                },
            });

            chunk_idx += 1;
        }
    }

    // If no sections were found (flat document), process as a normal text file
    if chunks.is_empty() {
        let processed_content = if config.normalize_text {
            util::normalize_text(&content)
        } else {
            content
        };

        let text_chunks =
            util::chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

        chunks = text_chunks
            .into_iter()
            .enumerate()
            .map(|(idx, content)| Chunk {
                content,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: idx,
                    total_chunks: None,
                    page_number: None,
                    section: None,
                    mime_type: "text/markdown".to_string(),
                },
            })
            .collect();
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

fn extract_markdown_sections(content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_section_title = "Introduction".to_string();
    let mut current_section_content = String::new();
    let mut line_buffer = String::new();

    for line in content.lines() {
        // Check if this line is a header
        if line.starts_with('#') {
            // Save previous section if it has content
            if !current_section_content.is_empty() {
                sections.push((current_section_title, current_section_content));
                current_section_content = String::new();
            }

            // Extract new section title (remove # and trim)
            current_section_title = line
                .trim_start_matches(|c| c == '#' || c == ' ')
                .trim()
                .to_string();

            // Add the header line to the new section content
            line_buffer.push_str(line);
            line_buffer.push('\n');
        } else {
            // Add to current section
            line_buffer.push_str(line);
            line_buffer.push('\n');
        }

        // If we've accumulated some lines, add them to the current section
        if !line_buffer.is_empty() {
            current_section_content.push_str(&line_buffer);
            line_buffer.clear();
        }
    }

    // Add the last section if it has content
    if !current_section_content.is_empty() {
        sections.push((current_section_title, current_section_content));
    }

    // If no sections were created (no headers in document),
    // treat the entire document as one section
    if sections.is_empty() && !content.is_empty() {
        sections.push(("Document".to_string(), content.to_string()));
    }

    sections
}
