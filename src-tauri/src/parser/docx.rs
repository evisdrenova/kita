// use std::path::{Path, PathBuf};
// use std::io::Read;
// use zip::ZipArchive;
// use quick_xml::Reader;
// use quick_xml::events::Event;
// use tracing::{debug, error, instrument};

// use super::common::{ParsedChunk, ChunkMetadata, ParserConfig, ParserResult, ParserError};
// use super::Parser;
// use super::util;

// /// Parser for DOCX files
// #[derive(Default)]
// pub struct DocxParser;

// impl Parser for DocxParser {
//     fn supported_mime_types(&self) -> Vec<&str> {
//         vec![
//             "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
//             "application/docx"
//         ]
//     }

//     fn can_parse_file_type(&self, path: &Path) -> bool {
//         if let Some(ext) = path.extension() {
//             if ext.to_string_lossy().to_lowercase() == "docx" {
//                 return true;
//             }
//         }

//         // Try to detect by MIME type
//         match util::detect_mime_type(path) {
//             Ok(mime) => {
//                 mime == "application/vnd.openxmlformats-officedocument.wordprocessingml.document" ||
//                 mime == "application/docx"
//             },
//             Err(_) => false,
//         }
//     }

//     #[instrument(skip(self, config))]
//     async fn parse(
//         &self,
//         path: &Path,
//         config: &ParserConfig
//     ) -> ParserResult<Vec<ParsedChunk>> {
//         debug!("Parsing DOCX file: {}", path.display());

//         // DOCX parsing is CPU-bound, so run it in a blocking task
//         let path_buf = path.to_path_buf();
//         let config_clone = config.clone();

//         tokio::task::spawn_blocking(move || {
//             let file = std::fs::File::open(&path_buf)?;

//             // Parse the document
//             let text = self.extract_docx_text(file)?;

//             // Normalize if needed
//             let processed_text = if config_clone.normalize_text {
//                 util::normalize_text(&text)
//             } else {
//                 text
//             };

//             // Split into chunks
//             let text_chunks = util::chunk_text(
//                 &processed_text,
//                 config_clone.chunk_size,
//                 config_clone.chunk_overlap
//             );

//             // Create ParsedChunk objects
//             let result = text_chunks
//                 .into_iter()
//                 .enumerate()
//                 .map(|(idx, content)| {
//                     ParsedChunk {
//                         content,
//                         metadata: ChunkMetadata {
//                             source_path: path_buf.clone(),
//                             chunk_index: idx,
//                             total_chunks: None,
//                             page_number: None,
//                             section: None,
//                             mime_type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document".to_string(),
//                         },
//                     }
//                 })
//                 .collect::<Vec<_>>();

//             // Update total_chunks
//             let total = result.len();
//             let result = result
//                 .into_iter()
//                 .map(|mut chunk| {
//                     chunk.metadata.total_chunks = Some(total);
//                     chunk
//                 })
//                 .collect();

//             Ok(result)
//         })
//         .await
//         .map_err(|e| ParserError::JoinError(e.to_string()))?
//     }
// }

// impl DocxParser {
//     /// Extract text from DOCX document
//     fn extract_docx_text(&self, file: std::fs::File) -> ParserResult<String> {
//         // DOCX is a ZIP file containing XML files
//         let mut archive = ZipArchive::new(file)
//             .map_err(|e| ParserError::DocxError(format!("Failed to open DOCX as ZIP: {}", e)))?;

//         // Check if document.xml exists
//         if !archive.file_names().any(|name| name == "word/document.xml") {
//             return Err(ParserError::DocxError("Invalid DOCX: Missing word/document.xml".to_string()));
//         }

//         // Read document.xml
//         let mut document_xml = archive.by_name("word/document.xml")
//             .map_err(|e| ParserError::DocxError(format!("Failed to read document.xml: {}", e)))?;

//         let mut xml_content = String::new();
//         document_xml.read_to_string(&mut xml_content)
//             .map_err(|e| ParserError::DocxError(format!("Failed to read XML content: {}", e)))?;

//         // Extract text from XML
//         let text = self.extract_text_from_document_xml(&xml_content)?;

//         // Also check for headers, footers, and footnotes if needed
//         // This would involve looking for header*.xml, footer*.xml, footnotes.xml, etc.

//         Ok(text)
//     }

//     /// Extract text from document.xml
//     fn extract_text_from_document_xml(&self, xml_content: &str) -> ParserResult<String> {
//         let mut reader = Reader::from_str(xml_content);
//         reader.trim_text(true);

//         let mut text = String::new();
//         let mut buf = Vec::new();
//         let mut in_paragraph = false;
//         let mut in_text_run = false;

//         loop {
//             match reader.read_event_into(&mut buf) {
//                 Ok(Event::Start(ref e)) => {
//                     match e.name().as_ref() {
//                         b"p" => {
//                             in_paragraph = true;
//                         },
//                         b"r" => {
//                             in_text_run = true;
//                         },
//                         _ => {}
//                     }
//                 },
//                 Ok(Event::End(ref e)) => {
//                     match e.name().as_ref() {
//                         b"p" => {
//                             in_paragraph = false;
//                             text.push('\n'); // End paragraph with newline
//                         },
//                         b"r" => {
//                             in_text_run = false;
//                         },
//                         _ => {}
//                     }
//                 },
//                 Ok(Event::Text(e)) => {
//                     if in_paragraph && in_text_run {
//                         text.push_str(&e.unescape().unwrap_or_default().to_string());
//                     }
//                 },
//                 Ok(Event::Eof) => break,
//                 Err(e) => {
//                     return Err(ParserError::DocxError(format!("XML parsing error: {}", e)));
//                 },
//                 _ => {}
//             }
//             buf.clear();
//         }

//         Ok(text)
//     }
// }

// // More advanced DOCX processing would include:
// // 1. Handling of styles, formatting, and structure
// // 2. Tables and lists
// // 3. Headers, footers, footnotes, and comments
// // 4. Images and charts (with captions)
// // 5. Embedded documents

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[tokio::test]
//     async fn test_docx_parser() {
//         // Implement basic tests
//     }
// }
