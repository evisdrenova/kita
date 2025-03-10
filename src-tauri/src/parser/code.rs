// // src/parser/code.rs
// use std::collections::HashMap;
// use std::path::{Path, PathBuf};
// use tokio::fs::File;
// use tokio::io::{self, AsyncBufReadExt, BufReader};
// use tracing::{debug, instrument};

// use super::common::{ChunkMetadata, ParsedChunk, ParserConfig, ParserError, ParserResult};
// use super::util;
// use super::Parser;

// /// Parser for code files (Rust, JavaScript, TypeScript, Python, etc.)
// #[derive(Default)]
// pub struct CodeParser {
//     language_extensions: HashMap<String, String>,
// }

// impl CodeParser {
//     pub fn new() -> Self {
//         let mut parser = Self {
//             language_extensions: HashMap::new(),
//         };

//         // Initialize known language mappings
//         parser
//             .language_extensions
//             .insert("rs".to_string(), "Rust".to_string());
//         parser
//             .language_extensions
//             .insert("js".to_string(), "JavaScript".to_string());
//         parser
//             .language_extensions
//             .insert("ts".to_string(), "TypeScript".to_string());
//         parser
//             .language_extensions
//             .insert("tsx".to_string(), "TypeScript React".to_string());
//         parser
//             .language_extensions
//             .insert("jsx".to_string(), "JavaScript React".to_string());
//         parser
//             .language_extensions
//             .insert("py".to_string(), "Python".to_string());
//         parser
//             .language_extensions
//             .insert("java".to_string(), "Java".to_string());
//         parser
//             .language_extensions
//             .insert("c".to_string(), "C".to_string());
//         parser
//             .language_extensions
//             .insert("cpp".to_string(), "C++".to_string());
//         parser
//             .language_extensions
//             .insert("h".to_string(), "C Header".to_string());
//         parser
//             .language_extensions
//             .insert("hpp".to_string(), "C++ Header".to_string());
//         parser
//             .language_extensions
//             .insert("cs".to_string(), "C#".to_string());
//         parser
//             .language_extensions
//             .insert("go".to_string(), "Go".to_string());
//         parser
//             .language_extensions
//             .insert("rb".to_string(), "Ruby".to_string());
//         parser
//             .language_extensions
//             .insert("php".to_string(), "PHP".to_string());
//         parser
//             .language_extensions
//             .insert("swift".to_string(), "Swift".to_string());
//         parser
//             .language_extensions
//             .insert("kt".to_string(), "Kotlin".to_string());
//         parser
//             .language_extensions
//             .insert("scala".to_string(), "Scala".to_string());
//         parser
//             .language_extensions
//             .insert("sh".to_string(), "Shell".to_string());
//         parser
//             .language_extensions
//             .insert("bash".to_string(), "Bash".to_string());
//         parser
//             .language_extensions
//             .insert("html".to_string(), "HTML".to_string());
//         parser
//             .language_extensions
//             .insert("css".to_string(), "CSS".to_string());
//         parser
//             .language_extensions
//             .insert("scss".to_string(), "SCSS".to_string());
//         parser
//             .language_extensions
//             .insert("sql".to_string(), "SQL".to_string());

//         parser
//     }

//     /// Detect programming language from file extension
//     fn detect_language(&self, path: &Path) -> Option<String> {
//         path.extension()
//             .and_then(|ext| ext.to_str())
//             .and_then(|ext| self.language_extensions.get(ext).cloned())
//     }

//     /// Get MIME type for language
//     fn get_mime_type(&self, language: &str) -> String {
//         match language.to_lowercase().as_str() {
//             "javascript" | "javascript react" => "application/javascript".to_string(),
//             "typescript" | "typescript react" => "application/typescript".to_string(),
//             "python" => "text/x-python".to_string(),
//             "rust" => "text/rust".to_string(),
//             "java" => "text/x-java".to_string(),
//             "c" | "c header" => "text/x-c".to_string(),
//             "c++" | "c++ header" => "text/x-c++".to_string(),
//             "c#" => "text/x-csharp".to_string(),
//             "go" => "text/x-go".to_string(),
//             "ruby" => "text/x-ruby".to_string(),
//             "php" => "text/x-php".to_string(),
//             "swift" => "text/x-swift".to_string(),
//             "kotlin" => "text/x-kotlin".to_string(),
//             "scala" => "text/x-scala".to_string(),
//             "shell" | "bash" => "text/x-shellscript".to_string(),
//             "html" => "text/html".to_string(),
//             "css" => "text/css".to_string(),
//             "scss" => "text/x-scss".to_string(),
//             "sql" => "text/x-sql".to_string(),
//             _ => format!("text/x-{}", language.to_lowercase()),
//         }
//     }

//     /// Advanced: extract documentation comments from code
//     fn extract_doc_comments(&self, content: &str, language: &str) -> Vec<String> {
//         let mut doc_comments = Vec::new();

//         // Different languages have different doc comment styles
//         match language.to_lowercase().as_str() {
//             "rust" => {
//                 // Extract Rust doc comments (///, /** */)
//                 for line in content.lines() {
//                     let trimmed = line.trim();
//                     if trimmed.starts_with("///") {
//                         doc_comments.push(trimmed[3..].trim().to_string());
//                     }
//                 }

//                 // TODO: Handle block doc comments /** */
//             }
//             "javascript" | "typescript" | "javascript react" | "typescript react" => {
//                 // Extract JS/TS doc comments (///, /** */)
//                 for line in content.lines() {
//                     let trimmed = line.trim();
//                     if trimmed.starts_with("///") || trimmed.starts_with("//!") {
//                         doc_comments.push(trimmed[3..].trim().to_string());
//                     }
//                 }

//                 // TODO: Handle JSDoc block comments /** */
//             }
//             "python" => {
//                 // Extract Python docstrings
//                 // This is a simplistic approach; a more robust solution would use a proper parser
//                 let mut in_triple_quotes = false;
//                 let mut current_docstring = String::new();

//                 for line in content.lines() {
//                     let trimmed = line.trim();

//                     if !in_triple_quotes
//                         && (trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''"))
//                     {
//                         in_triple_quotes = true;
//                         current_docstring = trimmed[3..].to_string();

//                         // Check if docstring ends on the same line
//                         if (trimmed.starts_with("\"\"\"")
//                             && trimmed.ends_with("\"\"\"")
//                             && trimmed.len() > 6)
//                             || (trimmed.starts_with("'''")
//                                 && trimmed.ends_with("'''")
//                                 && trimmed.len() > 6)
//                         {
//                             in_triple_quotes = false;
//                             doc_comments.push(
//                                 current_docstring[..current_docstring.len() - 3]
//                                     .trim()
//                                     .to_string(),
//                             );
//                             current_docstring.clear();
//                         }
//                     } else if in_triple_quotes {
//                         if (trimmed.ends_with("\"\"\"") || trimmed.ends_with("'''")) {
//                             in_triple_quotes = false;
//                             current_docstring.push_str("\n");
//                             current_docstring.push_str(&trimmed[..trimmed.len() - 3]);
//                             doc_comments.push(current_docstring.trim().to_string());
//                             current_docstring.clear();
//                         } else {
//                             current_docstring.push_str("\n");
//                             current_docstring.push_str(trimmed);
//                         }
//                     }
//                 }
//             }
//             _ => {
//                 // Default: try to detect common comment patterns
//                 for line in content.lines() {
//                     let trimmed = line.trim();
//                     if trimmed.starts_with("///")
//                         || trimmed.starts_with("//!")
//                         || trimmed.starts_with("/**")
//                         || trimmed.starts_with("/*!")
//                     {
//                         doc_comments.push(trimmed.to_string());
//                     }
//                 }
//             }
//         }

//         doc_comments
//     }
// }

// impl Parser for CodeParser {
//     fn supported_mime_types(&self) -> Vec<&str> {
//         vec![
//             "text/rust",
//             "application/javascript",
//             "application/typescript",
//             "text/x-python",
//             "text/x-java",
//             "text/x-c",
//             "text/x-c++",
//             "text/x-csharp",
//             "text/x-go",
//             "text/html",
//             "text/css",
//         ]
//     }

//     fn can_parse_file_type(&self, path: &Path) -> bool {
//         if let Some(ext) = path.extension() {
//             if let Some(ext_str) = ext.to_str() {
//                 return self.language_extensions.contains_key(ext_str);
//             }
//         }

//         false
//     }

//     #[instrument(skip(self, config))]
//     async fn parse(&self, path: &Path, config: &ParserConfig) -> ParserResult<Vec<ParsedChunk>> {
//         debug!("Parsing code file: {}", path.display());

//         // Detect language
//         let language = self
//             .detect_language(path)
//             .unwrap_or_else(|| "Unknown".to_string());

//         debug!("Detected language: {}", language);

//         // Open the file
//         let file = File::open(path).await?;
//         let reader = BufReader::new(file);

//         // Read the entire file content
//         let mut lines = reader.lines();
//         let mut content = String::new();

//         while let Some(line) = lines.next_line().await? {
//             content.push_str(&line);
//             content.push('\n');
//         }

//         // Normalize text if configured
//         let processed_content = if config.normalize_text {
//             util::normalize_text(&content)
//         } else {
//             content
//         };

//         // Special handling for code files - we might want to preserve structure
//         // or extract docstrings/comments differently based on language
//         let chunks = self.process_code_file(&processed_content, &language, config)?;

//         // Create ParsedChunk objects
//         let mime_type = self.get_mime_type(&language);

//         let result = chunks
//             .into_iter()
//             .enumerate()
//             .map(|(idx, chunk_content)| ParsedChunk {
//                 content: chunk_content,
//                 metadata: ChunkMetadata {
//                     source_path: path.to_path_buf(),
//                     chunk_index: idx,
//                     total_chunks: None,
//                     page_number: None,
//                     section: Some(language.clone()),
//                     mime_type: mime_type.clone(),
//                 },
//             })
//             .collect::<Vec<_>>();

//         // Update total_chunks
//         let total = result.len();
//         let result = result
//             .into_iter()
//             .map(|mut chunk| {
//                 chunk.metadata.total_chunks = Some(total);
//                 chunk
//             })
//             .collect();

//         Ok(result)
//     }
// }

// impl CodeParser {
//     /// Process a code file with language-specific handling
//     fn process_code_file(
//         &self,
//         content: &str,
//         language: &str,
//         config: &ParserConfig,
//     ) -> ParserResult<Vec<String>> {
//         // For many RAG use cases, preserving code structure is important
//         // We might want to chunk by:
//         // 1. Function/method boundaries
//         // 2. Class/module boundaries
//         // 3. Logical sections

//         // For this example, we'll use a simpler approach that's still code-aware

//         if config.extract_metadata {
//             // If we want to focus on documentation/comments
//             let doc_comments = self.extract_doc_comments(content, language);

//             if !doc_comments.is_empty() {
//                 // Join comments with newlines and then chunk
//                 let doc_text = doc_comments.join("\n\n");
//                 return Ok(util::chunk_text(
//                     &doc_text,
//                     config.chunk_size,
//                     config.chunk_overlap,
//                 ));
//             }
//         }

//         // Otherwise, try to be smart about chunking code
//         match language.to_lowercase().as_str() {
//             "rust" => self.chunk_rust_code(content, config),
//             "python" => self.chunk_python_code(content, config),
//             "javascript" | "typescript" => self.chunk_js_ts_code(content, config),
//             _ => {
//                 // Default chunking strategy - try to respect function boundaries
//                 self.chunk_generic_code(content, config)
//             }
//         }
//     }

//     /// Chunk Rust code with awareness of functions, modules, etc.
//     fn chunk_rust_code(&self, content: &str, config: &ParserConfig) -> ParserResult<Vec<String>> {
//         // This is a simplified approach - a real implementation might use a proper Rust parser

//         // Split by obvious boundaries like function/struct/impl definitions
//         let mut chunks = Vec::new();
//         let mut current_chunk = String::new();
//         let mut current_chunk_size = 0;

//         for line in content.lines() {
//             // Check if this line starts a new definition
//             let starts_new_block = line.starts_with("fn ")
//                 || line.starts_with("struct ")
//                 || line.starts_with("enum ")
//                 || line.starts_with("impl ")
//                 || line.starts_with("mod ")
//                 || line.starts_with("trait ");

//             if starts_new_block
//                 && !current_chunk.is_empty()
//                 && current_chunk_size >= config.chunk_size
//             {
//                 // Start a new chunk
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }

//             current_chunk.push_str(line);
//             current_chunk.push('\n');
//             current_chunk_size += 1;

//             // If we've exceeded chunk size significantly, save this chunk
//             if current_chunk_size >= config.chunk_size * 2 {
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }
//         }

//         // Don't forget the last chunk
//         if !current_chunk.is_empty() {
//             chunks.push(current_chunk);
//         }

//         Ok(chunks)
//     }

//     /// Chunk Python code with awareness of functions, classes, etc.
//     fn chunk_python_code(&self, content: &str, config: &ParserConfig) -> ParserResult<Vec<String>> {
//         // Similar to Rust but with Python syntax
//         let mut chunks = Vec::new();
//         let mut current_chunk = String::new();
//         let mut current_chunk_size = 0;

//         for line in content.lines() {
//             let trimmed = line.trim();
//             // Check if this line starts a new definition
//             let starts_new_block = trimmed.starts_with("def ")
//                 || trimmed.starts_with("class ")
//                 || trimmed.starts_with("async def ");

//             if starts_new_block
//                 && !current_chunk.is_empty()
//                 && current_chunk_size >= config.chunk_size
//             {
//                 // Start a new chunk
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }

//             current_chunk.push_str(line);
//             current_chunk.push('\n');
//             current_chunk_size += 1;

//             // If we've exceeded chunk size significantly, save this chunk
//             if current_chunk_size >= config.chunk_size * 2 {
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }
//         }

//         // Don't forget the last chunk
//         if !current_chunk.is_empty() {
//             chunks.push(current_chunk);
//         }

//         Ok(chunks)
//     }

//     /// Chunk JavaScript/TypeScript code
//     fn chunk_js_ts_code(&self, content: &str, config: &ParserConfig) -> ParserResult<Vec<String>> {
//         // Similar approach for JS/TS
//         let mut chunks = Vec::new();
//         let mut current_chunk = String::new();
//         let mut current_chunk_size = 0;

//         for line in content.lines() {
//             let trimmed = line.trim();
//             // Check if this line starts a new definition (function, class, etc.)
//             let starts_new_block = trimmed.starts_with("function ")
//                 || trimmed.starts_with("class ")
//                 || trimmed.starts_with("const ") && trimmed.contains(" = function")
//                 || trimmed.starts_with("const ") && trimmed.contains(" = (")
//                 || trimmed.starts_with("export ")
//                     && (trimmed.contains(" function ")
//                         || trimmed.contains(" class ")
//                         || trimmed.contains(" const ")
//                         || trimmed.contains(" interface "));

//             if starts_new_block
//                 && !current_chunk.is_empty()
//                 && current_chunk_size >= config.chunk_size
//             {
//                 // Start a new chunk
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }

//             current_chunk.push_str(line);
//             current_chunk.push('\n');
//             current_chunk_size += 1;

//             // If we've exceeded chunk size significantly, save this chunk
//             if current_chunk_size >= config.chunk_size * 2 {
//                 chunks.push(current_chunk);
//                 current_chunk = String::new();
//                 current_chunk_size = 0;
//             }
//         }

//         // Don't forget the last chunk
//         if !current_chunk.is_empty() {
//             chunks.push(current_chunk);
//         }

//         Ok(chunks)
//     }

//     /// Generic code chunking strategy that works for most languages
//     fn chunk_generic_code(
//         &self,
//         content: &str,
//         config: &ParserConfig,
//     ) -> ParserResult<Vec<String>> {
//         // Fallback to standard text chunking
//         let chunks = util::chunk_text(content, config.chunk_size, config.chunk_overlap);
//         Ok(chunks)
//     }
// }

// // More advanced code parsing would include:
// // 1. Language-specific AST (Abstract Syntax Tree) parsing
// // 2. Better extraction of documentation, function signatures, classes, etc.
// // 3. Symbol extraction (function names, class names, etc.)
// // 4. Dependency analysis
// // 5. Semantic code understanding

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[tokio::test]
//     async fn test_code_parser() {
//         // Implement basic tests
//     }
// }
