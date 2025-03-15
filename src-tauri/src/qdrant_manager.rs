use qdrant_client::qdrant::{
    CreateCollection, Distance, OptimizersConfigDiff, VectorParams, VectorsConfig,
};

use qdrant_client::qdrant::vectors_config::Config;
use qdrant_client::Qdrant;
use std::path::Path;

struct QdrantManager {
    client: Qdrant,
    collection_name: String,
}

impl QdrantManager {
    /// Initialize new Qdrant client
    async fn new(data_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        // ensure the data directory exists
        std::fs::create_dir_all(data_path)?;

        // Initialize client
        let client: Qdrant = Qdrant::from_url("http://localhost:6334").build()?;
        let collection_name: &str = "documents";

        // Create collection if it doesn't exist
        Self::upsert_collection(&client, collection_name).await?;

        Ok(Self {
            client,
            collection_name: collection_name.to_string(),
        })
    }

    async fn upsert_collection(
        client: &Qdrant,
        collection_name: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
