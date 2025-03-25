use async_trait::async_trait;
use docx_rs::{
    read_docx, DocumentChild, ParagraphChild, TableCellChild, TableChild, TableRowChild,
};
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::embedder::Embedder;
use crate::file_processor::FileMetadata;

use super::common::{Chunk, ChunkMetadata, ChunkerConfig, ChunkerResult};
use super::Chunker;
use super::{util, ChunkerError};

/// Parser for DOCX files
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

        // Read the DOCX file
        let mut file = File::open(path).await?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        // Extract text from DOCX
        let chunks = tokio::task::spawn_blocking(move || {
            // Parse the DOCX document
            let doc = match read_docx(&buffer) {
                Ok(doc) => doc,
                Err(e) => {
                    return Err(ChunkerError::DocxFileError(format!(
                        "Failed to parse DOCX: {:?}",
                        e
                    )));
                }
            };

            // Extract text from the document
            let extracted_text = extract_text_from_docx(doc);

            // Create chunks based on section headers and paragraphs
            chunk_docx_content(&extracted_text, path, config)
        })
        .await
        .map_err(|e| ChunkerError::DocxFileError(format!("Thread error: {:?}", e)))??;

        if chunks.is_empty() {
            return Ok(Vec::new());
        }

        // Process embeddings in a single batch (similar to other chunkers)
        tokio::task::spawn_blocking(move || {
            // Extract just the text content for embedding
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
                Err(_) => Err(ChunkerError::DocxFileError(
                    "Failed to generate embeddings".to_string(),
                )),
            }
        })
        .await
        .map_err(|e| ChunkerError::DocxFileError(format!("Thread error: {:?}", e)))?
    }
}

/// Represents a section or block of content in a DOCX file
struct DocxSection {
    heading: Option<String>,
    content: String,
    level: usize,
}

/// Extract text from a DOCX document
fn extract_text_from_docx(doc: docx_rs::Document) -> Vec<DocxSection> {
    let mut sections = Vec::new();
    let mut current_section = DocxSection {
        heading: None,
        content: String::new(),
        level: 0,
    };

    // Process each document child
    for child in doc.children {
        match child {
            DocumentChild::Paragraph(para) => {
                // Check if this is a heading paragraph
                let is_heading = para
                    .property
                    .and_then(|prop| prop.style)
                    .map_or(false, |style| {
                        style.starts_with("Heading") || style.starts_with("heading")
                    });

                // Extract text from the paragraph
                let text = extract_text_from_paragraph(&para);

                if is_heading {
                    // Get heading level (if we can)
                    let level = para
                        .property
                        .and_then(|prop| prop.style)
                        .and_then(|style| {
                            if style.starts_with("Heading") || style.starts_with("heading") {
                                style.chars().last().and_then(|c| c.to_digit(10))
                            } else {
                                None
                            }
                        })
                        .unwrap_or(1) as usize;

                    // If we have content in the current section, save it
                    if !current_section.content.trim().is_empty() {
                        sections.push(current_section);
                    }

                    // Start a new section with this heading
                    current_section = DocxSection {
                        heading: Some(text.clone()),
                        content: String::new(),
                        level,
                    };
                } else {
                    // Add to current section's content
                    if !text.trim().is_empty() {
                        if !current_section.content.is_empty() {
                            current_section.content.push('\n');
                        }
                        current_section.content.push_str(&text);
                    }
                }
            }
            DocumentChild::Table(table) => {
                // Process table content
                let table_text = extract_text_from_table(&table);
                if !table_text.trim().is_empty() {
                    if !current_section.content.is_empty() {
                        current_section.content.push('\n');
                    }
                    current_section.content.push_str(&table_text);
                }
            }
            _ => {
                // Ignore other document elements
            }
        }
    }

    // Add the final section if it has content
    if !current_section.content.trim().is_empty() {
        sections.push(current_section);
    }

    sections
}

/// Extract text from a paragraph
fn extract_text_from_paragraph(para: &docx_rs::Paragraph) -> String {
    let mut text = String::new();

    for child in &para.children {
        match child {
            ParagraphChild::Run(run) => {
                for text_child in &run.children {
                    match text_child {
                        docx_rs::RunChild::Text(t) => {
                            text.push_str(&t.text);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    text
}

/// Extract text from a table
fn extract_text_from_table(table: &docx_rs::Table) -> String {
    let mut text = String::new();

    for child in &table.children {
        match child {
            TableChild::TableRow(row) => {
                let row_text = extract_text_from_table_row(row);
                if !row_text.trim().is_empty() {
                    if !text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&row_text);
                }
            }
            _ => {}
        }
    }

    text
}

/// Extract text from a table row
fn extract_text_from_table_row(row: &docx_rs::TableRow) -> String {
    let mut text = String::new();

    for child in &row.children {
        match child {
            TableRowChild::TableCell(cell) => {
                let cell_text = extract_text_from_table_cell(cell);
                if !cell_text.trim().is_empty() {
                    if !text.is_empty() {
                        text.push('\t');
                    }
                    text.push_str(&cell_text);
                }
            }
            _ => {}
        }
    }

    text
}

/// Extract text from a table cell
fn extract_text_from_table_cell(cell: &docx_rs::TableCell) -> String {
    let mut text = String::new();

    for child in &cell.children {
        match child {
            TableCellChild::Paragraph(para) => {
                let para_text = extract_text_from_paragraph(para);
                if !para_text.trim().is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&para_text);
                }
            }
            TableCellChild::Table(table) => {
                let table_text = extract_text_from_table(table);
                if !table_text.trim().is_empty() {
                    if !text.is_empty() {
                        text.push(' ');
                    }
                    text.push_str(&table_text);
                }
            }
            _ => {}
        }
    }

    text
}

/// Create chunks from extracted DOCX content
fn chunk_docx_content(
    sections: &[DocxSection],
    path: &Path,
    config: &ChunkerConfig,
) -> ChunkerResult<Vec<Chunk>> {
    let mut chunks = Vec::new();

    if sections.is_empty() {
        return Ok(chunks);
    }

    // Strategy 1: If we have sections, use them as natural chunk boundaries
    if sections.len() > 1 {
        for (idx, section) in sections.iter().enumerate() {
            let section_title = section.heading.as_deref().unwrap_or("Untitled Section");
            let mut content = String::new();

            // Add heading if available
            if let Some(heading) = &section.heading {
                content.push_str(heading);
                content.push_str("\n\n");
            }

            // Add content
            content.push_str(&section.content);

            // Create chunk
            chunks.push(Chunk {
                content,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: idx,
                    total_chunks: Some(sections.len()),
                    page_number: None,
                    section: Some(section_title.to_string()),
                    mime_type:
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                            .to_string(),
                },
            });
        }
    } else {
        // Strategy 2: Single section or no sections - chunk by size
        let full_text = if sections.len() == 1 {
            // Single section - use its content
            let section = &sections[0];
            let mut text = String::new();

            if let Some(heading) = &section.heading {
                text.push_str(heading);
                text.push_str("\n\n");
            }

            text.push_str(&section.content);
            text
        } else {
            // No sections (shouldn't happen) - empty string
            String::new()
        };

        if full_text.is_empty() {
            return Ok(chunks);
        }

        // Use the text chunking utility from common
        let text_chunks = util::chunk_text(&full_text, config.chunk_size, config.chunk_overlap);

        // Create chunks
        let total_chunks = text_chunks.len();
        chunks = text_chunks
            .into_iter()
            .enumerate()
            .map(|(idx, content)| Chunk {
                content,
                metadata: ChunkMetadata {
                    source_path: path.to_path_buf(),
                    chunk_index: idx,
                    total_chunks: Some(total_chunks),
                    page_number: None,
                    section: sections.get(0).and_then(|s| s.heading.clone()),
                    mime_type:
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                            .to_string(),
                },
            })
            .collect();
    }

    Ok(chunks)
}
