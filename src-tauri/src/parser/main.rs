use clap::{Parser, Subcommand};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn, Level};
use tracing_subscriber::{fmt, EnvFilter};

mod parser;
use parser::{
    common::{ParsedChunk, ParserConfig, ParserError, ParserResult},
    ParsingOrchestrator,
};

/// CLI application for parsing various file formats for RAG pipelines
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse files and output to a directory or stdout
    Parse {
        /// Input files or directories to parse
        #[arg(required = true)]
        inputs: Vec<PathBuf>,

        /// Output directory for parsed chunks
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Chunk size (in words)
        #[arg(short, long, default_value = "1000")]
        chunk_size: usize,

        /// Chunk overlap (in words)
        #[arg(short = 'v', long, default_value = "100")]
        chunk_overlap: usize,

        /// Normalize text (unify line endings, remove BOM, etc.)
        #[arg(short, long)]
        normalize: bool,

        /// Extract metadata (if available)
        #[arg(short, long)]
        metadata: bool,

        /// Maximum concurrent files to process
        #[arg(short, long, default_value = "8")]
        concurrent: usize,

        /// Use GPU acceleration if available
        #[arg(short, long)]
        gpu: bool,

        /// Use Rayon for CPU parallelization instead of Tokio tasks
        #[arg(short, long)]
        rayon: bool,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Parse {
            inputs,
            output,
            chunk_size,
            chunk_overlap,
            normalize,
            metadata,
            concurrent,
            gpu,
            rayon,
        } => {
            // Create parser config
            let config = ParserConfig {
                chunk_size,
                chunk_overlap,
                normalize_text: normalize,
                extract_metadata: metadata,
                max_concurrent_files: concurrent,
                use_gpu_acceleration: gpu,
            };

            // Create parsing orchestrator
            let orchestrator = ParsingOrchestrator::new(config);

            // Collect all files to parse
            let files = collect_files(inputs)?;
            info!("Found {} files to parse", files.len());

            // Parse files
            let chunks = if rayon {
                // Use Rayon for CPU parallelization
                parse_with_rayon(&orchestrator, files).await?
            } else {
                // Use Tokio for async parallelization
                parse_with_tokio(&orchestrator, files).await?
            };

            info!("Parsed {} chunks from {} files", chunks.len(), files.len());

            // Output chunks
            if let Some(output_dir) = output {
                save_chunks_to_directory(&chunks, &output_dir).await?;
            } else {
                // Print summary to stdout
                println!("Parsed {} chunks from {} files", chunks.len(), files.len());
                for (i, chunk) in chunks.iter().enumerate().take(5) {
                    println!(
                        "Chunk {}: {} bytes, from {}",
                        i,
                        chunk.content.len(),
                        chunk.metadata.source_path.display()
                    );
                }
                if chunks.len() > 5 {
                    println!("... and {} more chunks", chunks.len() - 5);
                }
            }
        }
    }

    Ok(())
}

/// Collect all files from input paths (expanding directories)
fn collect_files(inputs: Vec<PathBuf>) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for input in inputs {
        if input.is_dir() {
            // Walk directory and collect files
            for entry in walkdir::WalkDir::new(&input)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.is_file() {
                    files.push(path.to_path_buf());
                }
            }
        } else if input.is_file() {
            files.push(input);
        } else {
            warn!("Input path does not exist: {}", input.display());
        }
    }

    Ok(files)
}

/// Parse files using Tokio for async parallelization
async fn parse_with_tokio(
    orchestrator: &ParsingOrchestrator,
    files: Vec<PathBuf>,
) -> ParserResult<Vec<ParsedChunk>> {
    info!("Parsing with Tokio tasks");

    // Create channel for collecting chunks
    let (tx, mut rx) = mpsc::channel::<ParserResult<ParsedChunk>>(100);

    // Spawn a task to parse files in parallel
    let parse_task =
        tokio::spawn(async move { orchestrator.parse_files_parallel(files, tx).await });

    // Collect chunks from channel
    let mut chunks = Vec::new();
    while let Some(result) = rx.recv().await {
        match result {
            Ok(chunk) => chunks.push(chunk),
            Err(e) => {
                error!("Error parsing chunk: {:?}", e);
                // Continue processing other chunks
            }
        }
    }

    // Wait for parse task to complete
    match parse_task.await {
        Ok(result) => {
            if let Err(e) = result {
                error!("Error in parse task: {:?}", e);
            }
        }
        Err(e) => {
            error!("Parse task panicked: {:?}", e);
        }
    }

    Ok(chunks)
}

/// Parse files using Rayon for CPU parallelization
async fn parse_with_rayon(
    orchestrator: &ParsingOrchestrator,
    files: Vec<PathBuf>,
) -> ParserResult<Vec<ParsedChunk>> {
    info!("Parsing with Rayon CPU parallelization");
    orchestrator.parse_files_rayon(files)
}

/// Save parsed chunks to a directory
async fn save_chunks_to_directory(
    chunks: &[ParsedChunk],
    output_dir: &Path,
) -> std::io::Result<()> {
    // Create output directory if it doesn't exist
    tokio::fs::create_dir_all(output_dir).await?;

    // Group chunks by source file
    let mut chunks_by_file: std::collections::HashMap<PathBuf, Vec<&ParsedChunk>> =
        std::collections::HashMap::new();

    for chunk in chunks {
        chunks_by_file
            .entry(chunk.metadata.source_path.clone())
            .or_default()
            .push(chunk);
    }

    // Save each file's chunks
    for (source_path, file_chunks) in chunks_by_file {
        let file_name = source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        let file_stem = source_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();

        // Create a JSON file with all chunks for this source file
        let json_path = output_dir.join(format!("{}.json", file_stem));
        let json_content = serde_json::to_string_pretty(&file_chunks)?;
        tokio::fs::write(json_path, json_content).await?;

        // Also create individual text files for each chunk
        let file_dir = output_dir.join(file_stem.to_string());
        tokio::fs::create_dir_all(&file_dir).await?;

        for (i, chunk) in file_chunks.iter().enumerate() {
            let chunk_path = file_dir.join(format!("chunk_{:04}.txt", i));
            tokio::fs::write(chunk_path, &chunk.content).await?;
        }
    }

    Ok(())
}
