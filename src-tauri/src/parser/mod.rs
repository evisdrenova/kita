use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tracing::{error, info, instrument, warn};

// pub mod code;
// pub mod docx;
// pub mod pdf;
pub mod txt;
// pub mod xls;

pub use self::common::{ParsedChunk, ParserConfig, ParserError, ParserResult};

pub mod common {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ParsedChunk {
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
    pub struct ParserConfig {
        pub chunk_size: usize,
        pub chunk_overlap: usize,
        pub normalize_text: bool,
        pub extract_metadata: bool,
        pub max_concurrent_files: usize,
        pub use_gpu_acceleration: bool,
    }

    pub type ParserResult<T> = Result<T, ParserError>;

    #[derive(Error, Debug)]
    pub enum ParserError {
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

// Trait that all file parsers must implement
#[async_trait]
pub trait Parser: Send + Sync {
    // returns a vector of MIME types this parser can handle
    fn supported_mime_types(&self) -> Vec<&str>;

    // check if this parser can handle a given file type
    fn can_parse_file_type(&self, path: &Path) -> bool;

    #[instrument(skip(self, config))]
    async fn parse(&self, path: &Path, config: &ParserConfig) -> ParserResult<Vec<ParsedChunk>>;
}

pub struct ParsingOrchestrator {
    parsers: Vec<Box<dyn Parser>>,
    config: ParserConfig,
}

impl ParsingOrchestrator {
    // cerate a new parsing orchestrator
    pub fn new(config: ParserConfig) -> Self {
        let mut orchestrator = Self {
            parsers: Vec::new(),
            config,
        };

        orchestrator.register_parser(Box::new(txt::TxtParser::default()));
        // orchestrator.register_parser(Box::new(pdf::PdfParser::default()));
        // orchestrator.register_parser(Box::new(docx::DocxParser::default()));
        // orchestrator.register_parser(Box::new(xls::XlsParser::default()));
        // orchestrator.register_parser(Box::new(code::CodeParser::default()));

        orchestrator
    }

    pub fn register_parser(&mut self, parser: Box<dyn Parser>) {
        self.parsers.push(parser);
    }

    // find the right parser given a file type
    fn find_parser_for_file(&self, path: &Path) -> Option<&dyn Parser> {
        for parser in &self.parsers {
            if parser.can_handle(path) {
                return Some(parser.as_ref());
            }
        }
        None
    }

    /// Parse a single file and return chunks
    #[instrument(skip(self))]
    pub async fn parse_file(&self, path: &Path) -> ParserResult<Vec<ParsedChunk>> {
        let parser = self.find_parser_for_file(path).ok_or_else(|| {
            ParserError::UnsupportedType(
                path.extension()
                    .map(|ext| ext.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string()),
            )
        })?;

        info!(
            "Parsing file {} using {:?}",
            path.display(),
            std::any::type_name::<&dyn Parser>()
        );
        parser.parse(path, &self.config).await
    }

    // parse multipel chunks in parallel and stream results through a channel
    #[instrument(skip(self, paths, chunk_sender))]
    pub async fn parse_files_parallel(
        &self,
        paths: Vec<PathBuf>,
        chunk_sender: mpsc::Sender<ParserResult<ParsedChunk>>,
    ) -> ParserResult<()> {
        info!("Starting parallel parsing of {} files", paths.len());

        let mut tasks = JoinSet::new();
        let config = Arc::new(self.config.clone());
        let max_concurrent = self.config.max_concurrent_files;

        // Use semaphore to limit concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

        for path in paths {
            let path_clone = path.clone();
            let sender_clone = chunk_sender.clone();
            let config_clone = config.clone();
            let semaphore_clone = semaphore.clone();
            let orchestrator = Arc::new(self.clone());

            // Spawn a task for each file
            tasks.spawn(async move {
                // Acquire semaphore permit
                let _permit = semaphore_clone.acquire().await.unwrap();

                match orchestrator.parse_file(&path_clone).await {
                    Ok(chunks) => {
                        for chunk in chunks {
                            if sender_clone.send(Ok(chunk)).await.is_err() {
                                error!("Failed to send chunk through channel");
                                return Err(ParserError::ChannelError);
                            }
                        }
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to parse file {}: {:?}", path_clone.display(), e);
                        // Send the error through the channel too
                        if sender_clone.send(Err(e.clone())).await.is_err() {
                            return Err(ParserError::ChannelError);
                        }
                        Err(e)
                    }
                }
            });
        }

        // Wait for all tasks to complete
        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => { /* Task completed successfully */ }
                Ok(Err(e)) => {
                    warn!("A parsing task failed: {:?}", e);
                    // We continue processing other files even if one fails
                }
                Err(e) => {
                    error!("Task join error: {:?}", e);
                    return Err(ParserError::JoinError(e.to_string()));
                }
            }
        }

        info!("Completed parsing all files");
        Ok(())
    }

    // Implementation with Rayon for CPU-bound parsing operations
    pub fn parse_files_rayon(&self, paths: Vec<PathBuf>) -> ParserResult<Vec<ParsedChunk>> {
        use rayon::prelude::*;

        info!("Starting CPU-parallel parsing with Rayon");

        let results: Vec<ParserResult<Vec<ParsedChunk>>> = paths
            .par_iter()
            .map(|path| {
                // This is a blocking call using tokio::runtime::Handle
                let rt = tokio::runtime::Handle::current();
                rt.block_on(self.parse_file(path))
            })
            .collect();

        // Flatten results while propagating errors
        let mut all_chunks = Vec::new();
        for result in results {
            match result {
                Ok(chunks) => all_chunks.extend(chunks),
                Err(e) => {
                    error!("Error in parallel parsing: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(all_chunks)
    }
}

impl Clone for ParsingOrchestrator {
    fn clone(&self) -> Self {
        // We need to re-register parsers when cloning
        let mut new_instance = Self {
            parsers: Vec::new(),
            config: self.config.clone(),
        };

        // Re-register the default parsers
        new_instance.register_parser(Box::new(txt::TxtParser::default()));
        // new_instance.register_parser(Box::new(pdf::PdfParser::default()));
        // new_instance.register_parser(Box::new(docx::DocxParser::default()));
        // new_instance.register_parser(Box::new(xls::XlsParser::default()));
        // new_instance.register_parser(Box::new(code::CodeParser::default()));

        new_instance
    }
}

// Utility functions for file type detection
pub mod util {
    use super::*;
    use infer::Infer;
    use std::io::Read;

    /// Detect MIME type by reading magic bytes
    pub fn detect_mime_type(path: &Path) -> ParserResult<String> {
        let mut file = std::fs::File::open(path)?;
        let mut buffer = [0u8; 8192]; // Read 8KB for signature detection
        let bytes_read = file.read(&mut buffer)?;

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
        let mut normalized = text
            .replace("\r\n", "\n") // Normalize Windows line endings
            .replace("\r", "\n"); // Normalize Mac line endings

        // Remove BOM if present
        if normalized.starts_with("\u{FEFF}") {
            normalized = normalized[3..].to_string();
        }

        // Optional: collapse multiple blank lines, trim excessive whitespace, etc.
        normalized
    }

    /// Split text into chunks with optional overlap
    pub fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
        if text.is_empty() {
            return vec![];
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

            // Calculate the next starting position with overlap
            start = if end == words.len() {
                // We've reached the end
                end
            } else {
                // Move forward by (chunk_size - overlap)
                std::cmp::min(start + chunk_size - overlap, words.len() - 1)
            };
        }

        chunks
    }
}
