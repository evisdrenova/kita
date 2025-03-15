use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::qdrant::{
    CreateCollection, Distance, OptimizersConfigDiff, VectorParams, VectorsConfig,
};
use qdrant_client::Qdrant;
use qdrant_client::QdrantError;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::Manager;
use thiserror::Error;

struct QdrantManager {
    client: Qdrant,
    collection_name: String,
}

pub struct QdrantState {
    pub manager: Arc<Mutex<QdrantManager>>,
}

#[derive(Debug, Error)]
pub enum QdrantManagerError {
    #[error("Qdrant error: {0}")]
    QdrantErr(#[from] QdrantError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other: {0}")]
    Other(String),
}
pub type QdrantManagerResult<T> = Result<T, QdrantManagerError>;

impl QdrantManager {
    /// Initialize qdrant
    pub async fn initialize_qdrant(
        app_handle: AppHandle,
    ) -> QdrantManagerResult<Arc<Mutex<QdrantManager>>> {
        let app_data_dir: PathBuf = app_handle
            .path()
            .app_data_dir()
            .map_err(|_| QdrantManagerError::Other("Failed to get app data directory".into()))?;

        let qdrant_path: PathBuf = app_data_dir.join("vector_db");

        // Ensure storage directory exists
        std::fs::create_dir_all(&qdrant_path).map_err(QdrantManagerError::Io)?;

        // Create a new client
        let manager = Self::new_client().await?;

        println!(
            "Qdrant vector database initialized. Path: {:?}",
            qdrant_path
        );
        Ok(Arc::new(Mutex::new(manager)))
    }
    /// Create new qdrant client
    async fn new_client() -> QdrantManagerResult<Self> {
        // Connect to an already-running Qdrant server on 6334
        let client = Qdrant::from_url("http://localhost:6334").build()?;
        let collection_name = "documents";

        // Ensure the collection exists
        Self::upsert_collection(&client, collection_name).await?;

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
        })
    }

    async fn upsert_collection(client: &Qdrant, collection_name: &str) -> QdrantManagerResult<()> {
        let collections: bool = client.collection_exists(collection_name).await?;

        if collections {
            return Ok(());
        }

        let vectors_config: VectorsConfig = VectorsConfig {
            config: Some(Config::Params(VectorParams {
                size: 384 as u64,
                distance: Distance::Cosine.into(),
                on_disk: Some(true), // Store vectors on disk
                ..Default::default()
            })),
        };

        let optimizers_config: OptimizersConfigDiff = OptimizersConfigDiff {
            indexing_threshold: Some(20000 as u64), // Less frequent indexing for better write performance
            flush_interval_sec: Some(430 as u64),   // Flush to disk every 30 seconds
            max_segment_size: Some(50000 as u64),   // Larger segments for better storage efficiency
            ..Default::default()
        };

        let collection_config: CreateCollection = CreateCollection {
            collection_name: collection_name.to_string(),
            vectors_config: Some(vectors_config),
            optimizers_config: Some(optimizers_config),
            ..Default::default()
        };

        client.create_collection(collection_config).await?;

        Ok(())
    }
}
