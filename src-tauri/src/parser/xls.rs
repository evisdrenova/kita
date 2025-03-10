// // src/parser/xls.rs
// use calamine::{open_workbook, DataType, Range, RangeDeserializerBuilder, Reader, Xlsx};
// use std::path::{Path, PathBuf};
// use tracing::{debug, error, instrument};

// use super::common::{ChunkMetadata, ParsedChunk, ParserConfig, ParserError, ParserResult};
// use super::util;
// use super::Parser;

// /// Parser for Excel files (.xls, .xlsx)
// #[derive(Default)]
// pub struct XlsParser;

// impl Parser for XlsParser {
//     fn supported_mime_types(&self) -> Vec<&str> {
//         vec![
//             "application/vnd.ms-excel",
//             "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
//             "application/xls",
//             "application/xlsx",
//         ]
//     }

//     fn can_parse_file_type(&self, path: &Path) -> bool {
//         if let Some(ext) = path.extension() {
//             let ext_str = ext.to_string_lossy().to_lowercase();
//             if ext_str == "xls" || ext_str == "xlsx" {
//                 return true;
//             }
//         }

//         // Try to detect by MIME type
//         match util::detect_mime_type(path) {
//             Ok(mime) => {
//                 mime == "application/vnd.ms-excel"
//                     || mime == "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
//                     || mime == "application/xls"
//                     || mime == "application/xlsx"
//             }
//             Err(_) => false,
//         }
//     }

//     #[instrument(skip(self, config))]
//     async fn parse(&self, path: &Path, config: &ParserConfig) -> ParserResult<Vec<ParsedChunk>> {
//         debug!("Parsing Excel file: {}", path.display());

//         // Spreadsheet parsing is CPU-bound, so run it in a blocking task
//         let path_buf = path.to_path_buf();
//         let config_clone = config.clone();

//         tokio::task::spawn_blocking(move || {
//             // Detect Excel file type based on extension
//             let is_xlsx = path_buf
//                 .extension()
//                 .map(|ext| ext.to_string_lossy().to_lowercase() == "xlsx")
//                 .unwrap_or(false);

//             let mut workbook = open_workbook(&path_buf)
//                 .map_err(|e| ParserError::XlsError(format!("Failed to open Excel file: {}", e)))?;

//             let mut all_chunks = Vec::new();

//             // Process each worksheet
//             for sheet_name in workbook.sheet_names().to_owned() {
//                 let sheet_chunks = self.process_worksheet(
//                     &mut workbook,
//                     &sheet_name,
//                     &path_buf,
//                     &config_clone,
//                     all_chunks.len(),
//                 )?;

//                 all_chunks.extend(sheet_chunks);
//             }

//             // Update total_chunks
//             let total = all_chunks.len();
//             let result = all_chunks
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

// impl XlsParser {
//     /// Process a single worksheet and convert to text chunks
//     fn process_worksheet(
//         &self,
//         workbook: &mut calamine::Xlsx<std::io::BufReader<std::fs::File>>,
//         sheet_name: &str,
//         path: &Path,
//         config: &ParserConfig,
//         start_index: usize,
//     ) -> ParserResult<Vec<ParsedChunk>> {
//         // Get the worksheet
//         let range = workbook.worksheet_range(sheet_name).map_err(|e| {
//             ParserError::XlsError(format!("Failed to read worksheet '{}': {}", sheet_name, e))
//         })?;

//         // Convert the worksheet to a textual representation
//         let text = self.range_to_text(&range, sheet_name)?;

//         // Normalize if needed
//         let processed_text = if config.normalize_text {
//             util::normalize_text(&text)
//         } else {
//             text
//         };

//         // Split into chunks
//         let text_chunks =
//             util::chunk_text(&processed_text, config.chunk_size, config.chunk_overlap);

//         // Create ParsedChunk objects
//         let mime_type = if path.extension().map(|e| e.to_string_lossy().to_lowercase())
//             == Some("xlsx".into())
//         {
//             "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
//         } else {
//             "application/vnd.ms-excel"
//         };

//         let result = text_chunks
//             .into_iter()
//             .enumerate()
//             .map(|(idx, content)| {
//                 ParsedChunk {
//                     content,
//                     metadata: ChunkMetadata {
//                         source_path: path.to_path_buf(),
//                         chunk_index: start_index + idx,
//                         total_chunks: None, // Will update after processing all sheets
//                         page_number: None,
//                         section: Some(sheet_name.to_string()),
//                         mime_type: mime_type.to_string(),
//                     },
//                 }
//             })
//             .collect();

//         Ok(result)
//     }

//     /// Convert a worksheet range to a textual representation
//     fn range_to_text(&self, range: &Range<DataType>, sheet_name: &str) -> ParserResult<String> {
//         let mut text = format!("Sheet: {}\n\n", sheet_name);

//         // Check if the range has any data
//         if range.is_empty() {
//             return Ok(text);
//         }

//         // Get header row if available
//         let has_headers = !range.rows().next().map(|r| r.is_empty()).unwrap_or(true);

//         if has_headers {
//             // Extract headers
//             let headers: Vec<String> = range
//                 .rows()
//                 .next()
//                 .unwrap()
//                 .iter()
//                 .map(|cell| cell.to_string())
//                 .collect();

//             // Process data rows
//             for row in range.rows().skip(1) {
//                 for (i, cell) in row.iter().enumerate() {
//                     if i < headers.len() && !cell.is_empty() {
//                         text.push_str(&format!("{}: {}\n", headers[i], cell));
//                     }
//                 }
//                 text.push('\n');
//             }
//         } else {
//             // No headers, just output cells
//             for row in range.rows() {
//                 for cell in row.iter() {
//                     if !cell.is_empty() {
//                         text.push_str(&format!("{}\t", cell));
//                     }
//                 }
//                 text.push('\n');
//             }
//         }

//         Ok(text)
//     }

//     /// For more complex Excel files, we might want a structured approach
//     fn range_to_csv(&self, range: &Range<DataType>) -> String {
//         let mut csv = String::new();

//         for row in range.rows() {
//             let row_str = row
//                 .iter()
//                 .map(|cell| {
//                     let cell_str = cell.to_string();
//                     // Escape CSV properly
//                     if cell_str.contains(',') || cell_str.contains('"') || cell_str.contains('\n') {
//                         format!("\"{}\"", cell_str.replace('"', "\"\""))
//                     } else {
//                         cell_str
//                     }
//                 })
//                 .collect::<Vec<_>>()
//                 .join(",");

//             csv.push_str(&row_str);
//             csv.push('\n');
//         }

//         csv
//     }
// }

// // More advanced Excel processing would include:
// // 1. Better handling of formulas and calculated values
// // 2. Handling of merged cells
// // 3. Processing of charts and graphs
// // 4. Handling of Excel tables and pivots
// // 5. Support for macros and VBA content (if relevant)

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[tokio::test]
//     async fn test_xls_parser() {
//         // Implement basic tests
//     }
// }
