// src/parser/pdf.rs
use lopdf::Document;
use pdf_extract::{self, OutputOptions};
use std::path::{Path, PathBuf};
use tracing::{debug, error, instrument};

use super::common::{ChunkMetadata, ParsedChunk, ParserConfig, ParserError, ParserResult};
use super::util;
use super::Parser;

/// Parser for PDF files with GPU acceleration support via poppler if available
#[derive(Default)]
pub struct PdfParser;

impl Parser for PdfParser {
    fn supported_mime_types(&self) -> Vec<&str> {
        vec!["application/pdf"]
    }

    fn can_parse_file_type(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            if ext.to_string_lossy().to_lowercase() == "pdf" {
                return true;
            }
        }

        // Try to detect by MIME type
        match util::detect_mime_type(path) {
            Ok(mime) => mime == "application/pdf",
            Err(_) => false,
        }
    }

    #[instrument(skip(self, config))]
    async fn parse(&self, path: &Path, config: &ParserConfig) -> ParserResult<Vec<ParsedChunk>> {
        debug!("Parsing PDF file: {}", path.display());

        // Check if GPU acceleration is available and enabled
        if config.use_gpu_acceleration && is_gpu_available() {
            // Use GPU-accelerated parsing (via tokio blocking task)
            self.parse_with_gpu(path, config).await
        } else {
            // Use CPU-based parsing
            self.parse_with_cpu(path, config).await
        }
    }
}

impl PdfParser {
    /// Parse PDF using CPU-based libraries
    async fn parse_with_cpu(
        &self,
        path: &Path,
        config: &ParserConfig,
    ) -> ParserResult<Vec<ParsedChunk>> {
        // Run the CPU-intensive parsing in a blocking task pool
        let path_buf = path.to_path_buf();
        let config_clone = config.clone();

        tokio::task::spawn_blocking(move || {
            let mut chunks = Vec::new();

            // Try pdf-extract first as it's generally more reliable for text
            match self.extract_with_pdf_extract(&path_buf, &config_clone) {
                Ok(extracted_chunks) => {
                    chunks = extracted_chunks;
                }
                Err(e) => {
                    error!(
                        "Error extracting PDF with pdf-extract: {:?}. Falling back to lopdf",
                        e
                    );
                    // Fallback to lopdf
                    match self.extract_with_lopdf(&path_buf, &config_clone) {
                        Ok(lopdf_chunks) => chunks = lopdf_chunks,
                        Err(e) => return Err(e),
                    }
                }
            }

            Ok(chunks)
        })
        .await
        .map_err(|e| ParserError::JoinError(e.to_string()))?
    }

    /// Extract text using pdf-extract
    fn extract_with_pdf_extract(
        &self,
        path: &Path,
        config: &ParserConfig,
    ) -> ParserResult<Vec<ParsedChunk>> {
        // Configure pdf-extract options
        let options = OutputOptions {
            include_text_data: true,
            include_metadata: config.extract_metadata,
            include_fonts: false,
            include_images: false,
            ..Default::default()
        };

        // Extract text from PDF
        let text = pdf_extract::extract_text_with_options(path, &options)
            .map_err(|e| ParserError::PdfError(format!("pdf-extract error: {}", e)))?;

        // Normalize if needed
        let processed_text = if config.normalize_text {
            util::normalize_text(&text)
        } else {
            text
        };

        // Split into chunks
        let text_chunks =
            util::chunk_text(&processed_text, config.chunk_size, config.chunk_overlap);

        // Create ParsedChunk objects
        let result = text_chunks
            .into_iter()
            .enumerate()
            .map(|(idx, content)| {
                ParsedChunk {
                    content,
                    metadata: ChunkMetadata {
                        source_path: path.to_path_buf(),
                        chunk_index: idx,
                        total_chunks: None,
                        page_number: None, // We don't have page info with this method
                        section: None,
                        mime_type: "application/pdf".to_string(),
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

    /// Extract text using lopdf (page by page)
    fn extract_with_lopdf(
        &self,
        path: &Path,
        config: &ParserConfig,
    ) -> ParserResult<Vec<ParsedChunk>> {
        // Open the PDF document
        let doc = Document::load(path)
            .map_err(|e| ParserError::PdfError(format!("lopdf error: {}", e)))?;

        let mut all_chunks = Vec::new();

        // Extract text from each page
        for (page_idx, page_id) in doc.page_iter().enumerate() {
            let page_number = page_idx + 1;

            // Extract page text
            let page_content = match extract_page_text(&doc, page_id) {
                Ok(text) => text,
                Err(e) => {
                    error!("Error extracting text from page {}: {:?}", page_number, e);
                    continue; // Skip this page but continue with others
                }
            };

            // Normalize if needed
            let processed_text = if config.normalize_text {
                util::normalize_text(&page_content)
            } else {
                page_content
            };

            // Skip empty pages
            if processed_text.trim().is_empty() {
                continue;
            }

            // Split into chunks
            let text_chunks =
                util::chunk_text(&processed_text, config.chunk_size, config.chunk_overlap);

            // Create ParsedChunk objects for this page
            let page_chunks = text_chunks
                .into_iter()
                .enumerate()
                .map(|(idx, content)| {
                    ParsedChunk {
                        content,
                        metadata: ChunkMetadata {
                            source_path: path.to_path_buf(),
                            chunk_index: all_chunks.len() + idx,
                            total_chunks: None, // Will update after processing all pages
                            page_number: Some(page_number),
                            section: None,
                            mime_type: "application/pdf".to_string(),
                        },
                    }
                })
                .collect::<Vec<_>>();

            all_chunks.extend(page_chunks);
        }

        // Update total_chunks
        let total = all_chunks.len();
        let result = all_chunks
            .into_iter()
            .map(|mut chunk| {
                chunk.metadata.total_chunks = Some(total);
                chunk
            })
            .collect();

        Ok(result)
    }

    /// Parse PDF using GPU-accelerated libraries (if available)
    async fn parse_with_gpu(
        &self,
        path: &Path,
        config: &ParserConfig,
    ) -> ParserResult<Vec<ParsedChunk>> {
        // We'll use a wrapper around poppler-rs with CUDA support
        // This is mostly a placeholder - actual implementation would depend on
        // available GPU accelerated PDF libraries

        // For now, fall back to CPU parsing
        debug!(
            "GPU acceleration requested but not fully implemented. Falling back to CPU parsing."
        );
        self.parse_with_cpu(path, config).await
    }
}

/// Helper function to check if GPU is available
fn is_gpu_available() -> bool {
    // In a real implementation, you would check for CUDA or other GPU resources
    // For now, we'll just return false
    false
}

/// Helper function to extract text from a page using lopdf
fn extract_page_text(doc: &Document, page_id: lopdf::ObjectId) -> Result<String, String> {
    // This is a simplified version that may not handle all PDF formats correctly
    // A more robust implementation would involve proper parsing of content streams

    let page = doc
        .get_object(page_id)
        .map_err(|e| format!("Failed to get page object: {}", e))?;

    // Extract content streams
    let contents = doc
        .get_page_content(page_id)
        .map_err(|e| format!("Failed to get page content: {}", e))?;

    // This is a very basic extraction that won't handle complex PDFs well
    // In a real implementation, you would use a proper PDF content parser
    let text = String::from_utf8_lossy(&contents).to_string();

    // Very basic content extraction - this would need to be much more sophisticated
    // for real-world PDFs
    Ok(text)
}

// A more sophisticated PDF parser would include:
// 1. Proper handling of content streams and PDF operators
// 2. Text positioning and layout reconstruction
// 3. Table detection and extraction
// 4. Image OCR integration
// 5. Form field extraction
// 6. Better metadata handling

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pdf_parser() {
        // Implement basic tests
    }
}
