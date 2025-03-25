/// Common module that defines the traits and implementations that every chunker type (txt, pdf, docx, etc.) should implement
/// Also contains some utility functions
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tracing::error;

pub mod docx;
pub mod json;
pub mod pdf;
pub mod txt;

use crate::{embedder::Embedder, file_processor::FileMetadata};

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

        #[error("Unsupported file type: {0}")]
        UnsupportedType(String),

        #[error("PDF File parsing error: {0}")]
        PdFilefError(String),

        #[error("JSON File Parsing error: {0}")]
        JSONFileError(String),

        #[error("Text File Parsing error: {0}")]
        TextFileError(String),

        #[error("Docx File Parsing error: {0}")]
        DocxFileError(String),

        // #[error("XLS parsing error: {0}")]
        // XlsError(String),

        // #[error("Encoding error: {0}")]
        // EncodingError(String),

        // #[error("Channel error")]
        // ChannelError,

        // #[error("Task join error: {0}")]
        // JoinError(String),
        #[error("Other error: {0}")]
        Other(String),
    }
}

// chunker trait that each chunker needs to explicitly implement
#[async_trait]
pub trait Chunker: Send + Sync {
    fn supported_mime_types(&self) -> Vec<&str>;

    fn can_chunk_file_type(&self, path: &Path) -> bool;

    async fn chunk_file(
        &self,
        file: &FileMetadata,
        config: &ChunkerConfig,
        embedder: Arc<Embedder>,
    ) -> ChunkerResult<Vec<(Chunk, Vec<f32>)>>;
}

pub struct ChunkerOrchestrator {
    chunkers: Vec<Box<dyn Chunker>>, //a vector of available chunkers like txt, pdf, etc.
    config: ChunkerConfig,           // defines a chunker orchestrator config
    mime_map: HashMap<String, usize>, // mime type to chunker indices in the chunkers vector
    extension_map: HashMap<String, usize>, // maps extensions to chunker indices
}

impl ChunkerOrchestrator {
    pub fn new(config: ChunkerConfig) -> Self {
        let mut orchestrator = Self {
            chunkers: Vec::new(),
            extension_map: HashMap::new(),
            mime_map: HashMap::new(),
            config,
        };

        orchestrator.register_chunker(Box::new(txt::TxtChunker::default()));
        orchestrator.register_chunker(Box::new(pdf::PdfChunker::default()));
        orchestrator.register_chunker(Box::new(json::JsonChunker::default()));
        orchestrator.register_chunker(Box::new(docx::DocxChunker::default()));

        orchestrator
    }

    pub fn register_chunker(&mut self, chunker: Box<dyn Chunker>) {
        let chunker_index = self.chunkers.len();

        // Register all supported MIME types
        for mime_type in chunker.supported_mime_types() {
            self.mime_map.insert(mime_type.to_string(), chunker_index);
        }

        // Register extensions based on the chunker's supported MIME types
        for mime_type in chunker.supported_mime_types() {
            match mime_type {
                "text/plain" => {
                    self.extension_map.insert("txt".to_string(), chunker_index);
                    self.extension_map.insert("text".to_string(), chunker_index);
                }
                "application/pdf" => {
                    self.extension_map.insert("pdf".to_string(), chunker_index);
                }
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                    self.extension_map.insert("docx".to_string(), chunker_index);
                }
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                    self.extension_map.insert("xlsx".to_string(), chunker_index);
                }
                "text/rust" => {
                    self.extension_map.insert("rs".to_string(), chunker_index);
                }
                "application/javascript" => {
                    self.extension_map.insert("js".to_string(), chunker_index);
                }
                "application/typescript" => {
                    self.extension_map.insert("ts".to_string(), chunker_index);
                }
                "text/x-python" => {
                    self.extension_map.insert("py".to_string(), chunker_index);
                }
                "application/json" => {
                    self.extension_map.insert("json".to_string(), chunker_index);
                }
                "text/markdown" => {
                    self.extension_map.insert("md".to_string(), chunker_index);
                }
                "text/html" => {
                    self.extension_map.insert("html".to_string(), chunker_index);
                    self.extension_map.insert("htm".to_string(), chunker_index);
                }
                "text/css" => {
                    self.extension_map.insert("css".to_string(), chunker_index);
                }
                "text/csv" => {
                    self.extension_map.insert("csv".to_string(), chunker_index);
                }
                _ => {} // Ignore any other MIME types
            }
        }

        self.chunkers.push(chunker);
    }

    fn find_chunker_for_file(&self, path: &Path) -> Option<&dyn Chunker> {
        // First try a quick lookup by extension
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if let Some(&chunker_idx) = self.extension_map.get(&ext_str) {
                println!("Found chunker by extension mapping for file {:?}", path);
                return Some(self.chunkers[chunker_idx].as_ref());
            }
        }

        // If that fails, try MIME type detection
        match util::detect_mime_type(path) {
            Ok(mime) => {
                if let Some(&chunker_idx) = self.mime_map.get(&mime) {
                    println!("Found chunker by MIME type for file {:?}", path);
                    return Some(self.chunkers[chunker_idx].as_ref());
                }
            }
            Err(_) => {}
        }

        // Fallback: try each chunker directly (slower but more thorough)
        for (i, chunker) in self.chunkers.iter().enumerate() {
            println!("Trying chunker {} directly for file {:?}", i, path);
            if chunker.can_chunk_file_type(path) {
                println!("Chunker {} accepted file {:?}", i, path);
                return Some(chunker.as_ref());
            }
        }

        println!("No chunker found for file: {:?}", path);
        None
    }

    /// Find the right chunker for the file and chunk a single file
    pub async fn chunk_file(
        &self,
        file: &FileMetadata,
        embedder: Arc<Embedder>,
    ) -> ChunkerResult<Vec<(Chunk, Vec<f32>)>> {
        let chunker: &dyn Chunker = self
            .find_chunker_for_file(Path::new(&file.base.path))
            .ok_or_else(|| ChunkerError::UnsupportedType(file.extension.clone()))?;

        chunker.chunk_file(file, &self.config, embedder).await
    }
}

impl Clone for ChunkerOrchestrator {
    fn clone(&self) -> Self {
        // We need to re-register parsers when cloning
        let mut new_instance = Self {
            chunkers: Vec::new(),
            extension_map: HashMap::new(),
            mime_map: HashMap::new(),
            config: self.config.clone(),
        };

        // Re-register the default parsers
        new_instance.register_chunker(Box::new(txt::TxtChunker::default()));
        new_instance.register_chunker(Box::new(pdf::PdfChunker::default()));

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

        // Try to detect by magic bytes first
        let infer = Infer::new();
        if let Some(kind) = infer.get(&buffer[..bytes_read]) {
            return Ok(kind.mime_type().to_string());
        }

        // Fallback: use extension if available
        if let Some(ext) = path.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            match ext_str.as_str() {
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
                "json" => return Ok("application/json".to_string()),
                "md" => return Ok("text/markdown".to_string()),
                "html" | "htm" => return Ok("text/html".to_string()),
                "css" => return Ok("text/css".to_string()),
                "csv" => return Ok("text/csv".to_string()),
                _ => {
                    return Err(ChunkerError::UnsupportedType(format!(
                        "Unsupported file extension: {}",
                        ext_str
                    )))
                }
            }
        }

        Err(ChunkerError::UnsupportedType(
            "File has no extension and couldn't be identified by content".to_string(),
        ))
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

    /// Chunks texts based on a configured chunk_size and overlap
    pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
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
}
