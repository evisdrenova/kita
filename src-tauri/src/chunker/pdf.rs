use async_trait::async_trait;
use pdf_extract::extract_text;
use std::path::Path;
use std::sync::Arc;

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

#[derive(Default)]
pub struct PdfChunker;

#[async_trait]
impl Chunker for PdfChunker {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["application/pdf"]
    }

    fn can_chunk_file_type(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "pdf" {
                return true;
            }
        }

        match util::detect_mime_type(path) {
            Ok(mime) => mime == "application/pdf",
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

        // Extract text from PDF
        let pdf_text = extract_pdf_text(path).await?;

        let chunks = chunk_pdf_text(&pdf_text, path, config).await?;

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        tokio::task::spawn_blocking(move || {
            let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.content.as_str()).collect();

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
                Err(_) => Err(ChunkerError::PdfError(
                    "Failed to generate embeddings".to_string(),
                )),
            }
        })
        .await
        .map_err(|e| ChunkerError::PdfError(format!("Thread error: {:?}", e)))?
    }
}

async fn extract_pdf_text(path: &Path) -> ChunkerResult<String> {
    // Use blocking operation in a spawn_blocking task since PDF processing can be intensive
    let path_str = path.to_string_lossy().to_string();

    let text = tokio::task::spawn_blocking(move || match extract_text(&path_str) {
        Ok(text) => Ok(text),
        Err(e) => Err(ChunkerError::PdfError(format!(
            "Failed to extract PDF text: {}",
            e
        ))),
    })
    .await
    .map_err(|e| ChunkerError::PdfError(format!("Thread error: {:?}", e)))??;

    Ok(text)
}

async fn chunk_pdf_text(
    text: &str,
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    // Process content
    let processed_content = if config.normalize_text {
        util::normalize_text(text)
    } else {
        text.to_string()
    };

    // Create text chunks using the same function as for TXT files
    let text_chunks = util::chunk_text(&processed_content, config.chunk_size, config.chunk_overlap);

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
                mime_type: "application/pdf".to_string(),
            },
        })
        .collect();

    Ok(chunks)
}
