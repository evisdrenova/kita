use async_trait::async_trait;
use docx_rs::read_docx;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

/// Extremely simplified Parser for DOCX files that just extracts all text
#[derive(Default)]
pub struct DocxChunker;

#[async_trait]
impl Chunker for DocxChunker {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "application/msword",
        ]
    }

    fn can_chunk_file_type(&self, path: &Path) -> bool {
        match util::detect_mime_type(path) {
            Ok(mime) => {
                mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    || mime == "application/msword"
            }
            Err(_) => {
                // Fallback to extension check
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    ext_str == "docx" || ext_str == "doc"
                } else {
                    false
                }
            }
        }
    }

    async fn chunk_file(
        &self,
        file: &FileMetadata,
        config: &ChunkerConfig,
        embedder: Arc<Embedder>,
    ) -> ChunkerResult<Vec<(Chunk, Vec<f32>)>> {
        println!("Creating DOCX chunks for file {:?}", file.base.path);

        let path = Path::new(&file.base.path);
        let path_buf = path.to_path_buf();
        let config_clone = config.clone();

        // Read the DOCX file
        let mut file = File::open(path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        // Extract text and create chunks
        let chunks = tokio::task::spawn_blocking(move || {
            // Parse the DOCX document and extract all text using a simplified approach
            let extracted_text = match extract_text_from_docx(&buffer) {
                Ok(text) => text,
                Err(e) => return Err(e),
            };

            // Apply text normalization if configured
            let processed_text = if config_clone.normalize_text {
                util::normalize_text(&extracted_text)
            } else {
                extracted_text
            };

            // Use the common text chunking utility
            let text_chunks = util::chunk_text(
                &processed_text,
                config_clone.chunk_size,
                config_clone.chunk_overlap,
            );

            // Create chunks with metadata
            let total_chunks = text_chunks.len();
            let chunks: Vec<Chunk> = text_chunks
                .into_iter()
                .enumerate()
                .map(|(idx, content)| Chunk {
                    content,
                    metadata: ChunkMetadata {
                        source_path: path_buf.clone(),
                        chunk_index: idx,
                        total_chunks: Some(total_chunks),
                        page_number: None,
                        section: None,
                        mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
                    },
                })
                .collect();

            Ok(chunks)
        })
        .await
        .map_err(|e| ChunkerError::Other(format!("Thread error: {:?}", e)))??;

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Process embeddings in a single batch
        tokio::task::spawn_blocking(move || {
            let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.content.as_str()).collect();

            match embedder.model.embed(texts, None) {
                Ok(embeddings) => {
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

/// Extract all text from a DOCX file using a much simpler approach
fn extract_text_from_docx(buffer: &[u8]) -> ChunkerResult<String> {
    // Parse the DOCX document
    let doc = read_docx(buffer)
        .map_err(|e| ChunkerError::Other(format!("Failed to parse DOCX: {:?}", e)))?;

    // Ultra-simplified approach: extract text from paragraphs only
    let mut text = String::new();

    // Walk through the document structure
    extract_text_recursive(&doc.document, &mut text);

    Ok(text)
}

/// Recursively extract text from a document structure
fn extract_text_recursive(doc: &docx_rs::Document, text: &mut String) {
    for child in &doc.children {
        match child {
            docx_rs::DocumentChild::Paragraph(para) => {
                // Process paragraphs
                for run_child in &para.children {
                    if let docx_rs::ParagraphChild::Run(run) = run_child {
                        for text_child in &run.children {
                            if let docx_rs::RunChild::Text(t) = text_child {
                                text.push_str(&t.text);
                            }
                        }
                    }
                }
                text.push('\n'); // Add newline after paragraphs
            }
            _ => {
                // Skip other types - this is the ultra-simplified approach
                // In a real implementation, you'd handle tables, etc.
            }
        }
    }
}
