use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{error, instrument};

pub mod txt;

use crate::file_processor::FileMetadata;

pub use self::common::{Chunk, ChunkerConfig, ChunkerError, ChunkerResult};

pub mod common {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct Chunk {
        pub content: String,
        pub metadata: ChunkMetadata,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChunkMetadata {
        pub source_path: PathBuf,
        pub chunk_index: usize,
        pub total_chunks: Option<usize>,
        pub page_number: Option<usize>,
        pub section: Option<String>,
        pub mime_type: String,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChunkerConfig {
        pub chunk_size: usize,
        pub chunk_overlap: usize,
        pub normalize_text: bool,
        pub extract_metadata: bool,
        pub max_concurrent_files: usize,
        pub use_gpu_acceleration: bool,
    }

    pub type ChunkerResult<T> = Result<T, ChunkerError>;

    #[derive(Error, Debug)]
    pub enum ChunkerError {
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),

        #[error("File format error: {0}")]
        Format(String),

        #[error("Unsupported file type: {0}")]
        UnsupportedType(String),

        #[error("PDF parsing error: {0}")]
        PdfError(String),

        #[error("DOCX parsing error: {0}")]
        DocxError(String),

        #[error("XLS parsing error: {0}")]
        XlsError(String),

        #[error("Encoding error: {0}")]
        EncodingError(String),

        #[error("Channel error")]
        ChannelError,

        #[error("Task join error: {0}")]
        JoinError(String),

        #[error("Other error: {0}")]
        Other(String),
    }
}

// chunker trait that each chunker needs to explicitly implement
#[async_trait]
pub trait Chunker: Send + Sync {
    // returns a vector of MIME types
    fn supported_mime_types(&self) -> Vec<&str>;

    fn can_chunk_file_type(&self, path: &Path) -> bool;

    async fn chunk_file(
        &self,
        file: &FileMetadata,
        config: &ChunkerConfig,
    ) -> ChunkerResult<Vec<Chunk>>;
}

pub struct ChunkerOrchestrator {
    chunkers: Vec<Box<dyn Chunker>>, //a vector of available chunkers like txt, pdf, etc.
    config: ChunkerConfig,
}

impl ChunkerOrchestrator {
    pub fn new(config: ChunkerConfig) -> Self {
        let mut orchestrator = Self {
            chunkers: Vec::new(),
            config,
        };

        orchestrator.register_chunker(Box::new(txt::TxtChunker::default()));

        orchestrator
    }

    pub fn register_chunker(&mut self, chunker: Box<dyn Chunker>) {
        self.chunkers.push(chunker);
    }

    fn find_chunker_for_file(&self, path: &Path) -> Option<&dyn Chunker> {
        for chunker in &self.chunkers {
            if chunker.can_chunk_file_type(path) {
                return Some(chunker.as_ref());
            }
        }
        None
    }

    /// Find the right chunker for the file and chunk a single file
    #[instrument(skip(self))]
    pub async fn chunk_file(&self, file: &FileMetadata) -> ChunkerResult<Vec<Chunk>> {
        let chunker = self
            .find_chunker_for_file(Path::new(&file.base.path))
            .ok_or_else(|| ChunkerError::UnsupportedType(file.extension.clone()))?;

        chunker.chunk_file(file, &self.config).await
    }
}

impl Clone for ChunkerOrchestrator {
    fn clone(&self) -> Self {
        // We need to re-register parsers when cloning
        let mut new_instance: ChunkerOrchestrator = Self {
            chunkers: Vec::new(),
            config: self.config.clone(),
        };

        // Re-register the default parsers
        new_instance.register_chunker(Box::new(txt::TxtChunker::default()));

        new_instance
    }
}

// Utility functions for file type detection
pub mod util {
    use super::*;
    use infer::Infer;
    use std::io::Read;

    /// Detect MIME type by reading magic bytes
    pub fn detect_mime_type(path: &Path) -> ChunkerResult<String> {
        let mut file: std::fs::File = std::fs::File::open(path)?;
        let mut buffer: [u8; 8192] = [0u8; 8192]; // Read 8KB for signature detection
        let bytes_read: usize = file.read(&mut buffer)?;

        let infer = Infer::new();
        if let Some(kind) = infer.get(&buffer[..bytes_read]) {
            return Ok(kind.mime_type().to_string());
        }

        // Fallback: use extension if available
        if let Some(ext) = path.extension() {
            match ext.to_string_lossy().to_lowercase().as_str() {
                "txt" => return Ok("text/plain".to_string()),
                "pdf" => return Ok("application/pdf".to_string()),
                "docx" => {
                    return Ok(
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                            .to_string(),
                    )
                }
                "xlsx" => {
                    return Ok(
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                            .to_string(),
                    )
                }
                "rs" => return Ok("text/rust".to_string()),
                "js" => return Ok("application/javascript".to_string()),
                "ts" => return Ok("application/typescript".to_string()),
                "py" => return Ok("text/x-python".to_string()),
                // Add more mappings as needed
                _ => {}
            }
        }

        // Last resort: check if it's likely a text file
        if is_text_file(&buffer[..bytes_read]) {
            return Ok("text/plain".to_string());
        }

        Ok("application/octet-stream".to_string())
    }

    /// Simple heuristic to check if a buffer is likely text
    fn is_text_file(buffer: &[u8]) -> bool {
        if buffer.is_empty() {
            return false;
        }

        // Check if the content is likely ASCII or UTF-8 text
        let text_ratio =
            buffer.iter().filter(|&&b| b.is_ascii()).count() as f32 / buffer.len() as f32;
        text_ratio > 0.8
    }

    /// Normalize text: unify line endings, trim whitespace, etc.
    pub fn normalize_text(text: &str) -> String {
        let mut normalized = text.replace("\r", "\n"); // Normalize Mac line endings

        // Remove BOM if present
        if normalized.starts_with("\u{FEFF}") {
            normalized = normalized[3..].to_string();
        }

        // probably add more here
        normalized
    }
}
