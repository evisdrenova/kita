use arrow_array::types::Float32Type;
use arrow_array::FixedSizeListArray;
use arrow_array::RecordBatch;
use arrow_array::RecordBatchIterator;
use arrow_array::StringArray;
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::{Connection, Error, Table};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::AppHandle;
use tauri::Manager;
use thiserror::Error;
use tokio::sync::Mutex;

use crate::chunker::Chunk;
use crate::embedder::Embedder;

pub struct VectorDbManager {
    client: Connection,
}

const TABLE_NAME: &str = "embeddings";

#[derive(Debug, Error)]
pub enum VectorDbError {
    #[error("LanceDB error: {0}")]
    LanceError(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Other: {0}")]
    Other(String),
}

pub type VectorDbResult<T> = Result<T, VectorDbError>;

impl VectorDbManager {
    /// Initialize vectordb
    pub async fn initialize_vectordb(
        app_handle: AppHandle,
    ) -> VectorDbResult<Arc<Mutex<VectorDbManager>>> {
        let app_data_dir: PathBuf = app_handle
            .path()
            .app_data_dir()
            .map_err(|_| VectorDbError::Other("Failed to get app data directory".into()))?;

        let vectordb_path: PathBuf = app_data_dir.join("vector_db");

        let manager = Self::new_vectordb_client(&vectordb_path).await?;

        println!("Vector database initialized. Path: {:?}", vectordb_path);
        Ok(Arc::new(Mutex::new(manager)))
    }

    /// Create new vector db client
    async fn new_vectordb_client(vdb_path: &PathBuf) -> VectorDbResult<Self> {
        let client = match lancedb::connect(&vdb_path.to_string_lossy())
            .execute()
            .await
        {
            Ok(client) => {
                println!("Successfully created LanceDB client");
                client
            }
            Err(e) => {
                println!("Unable to create LanceDB client: {}", e);
                return Err(VectorDbError::LanceError(e.to_string()));
            }
        };

        let instance = Self { client };
        instance.init_table().await?;

        Ok(instance)
    }

    /// Creates embeddings table
    async fn init_table(&self) -> VectorDbResult<()> {
        let client: &Connection = &self.client;

        let table_exists: bool = match client.open_table(TABLE_NAME).execute().await {
            Ok(_) => {
                println!("Table '{}' exists", TABLE_NAME);
                true
            }
            Err(Error::TableNotFound { name }) if name == TABLE_NAME => {
                println!("Table '{}' doesn't exist, will create it", TABLE_NAME);
                false
            }
            Err(e) => {
                println!("some other error'{:?}' exists", e);
                false
            }
        };

        let schema_clone: Arc<Schema> = get_embeddings_schema();

        if !table_exists {
            Self::create_table(&self, TABLE_NAME, &schema_clone).await?;
            // create_index(client, &table, schema).await?;
        }

        Ok(())
    }

    async fn create_table(&self, table_name: &str, schema: &Arc<Schema>) -> VectorDbResult<Table> {
        let client = &self.client;

        match client
            .create_empty_table(table_name, schema.clone())
            .execute()
            .await
        {
            Ok(table) => {
                println!("Successfully created table '{}'", table_name);
                Ok(table)
            }
            Err(e) => {
                return Err(VectorDbError::LanceError(format!(
                    "Failed to create table: {}",
                    e
                )))
            }
        }
    }

    /// Inserts embeddings into vector database
    pub async fn insert_embeddings(
        app_handle: &AppHandle,
        file_id: &str,
        chunk_embeddings: Vec<(Chunk, Vec<f32>)>,
    ) -> VectorDbResult<()> {
        let state = app_handle.state::<Arc<Mutex<VectorDbManager>>>();
        let manager = state.lock().await;
        // open table
        let table = match manager.client.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(e) => {
                return Err(VectorDbError::LanceError(format!(
                    "Failed to open table: {}",
                    e
                )));
            }
        };

        let batches = from_chunks_embeddings_to_data(chunk_embeddings, file_id);

        // insert into table
        if let Err(e) = table.add(Box::new(batches)).execute().await {
            return Err(VectorDbError::LanceError(format!(
                "Failed to add embeddings: {}",
                e
            )));
        }

        Ok(())
    }

    /// Embeds the search query and searches vector database
    pub async fn search_similar(
        app_handle: &AppHandle,
        query_text: &str,
    ) -> VectorDbResult<Vec<RecordBatch>> {
        let embedder = app_handle.state::<Arc<Embedder>>();

        let query_embedding = embedder.embed_single_text(query_text);

        let state = app_handle.state::<Arc<tokio::sync::Mutex<VectorDbManager>>>();
        let manager = state.lock().await;

        // Open the table
        let table = match manager.client.open_table(TABLE_NAME).execute().await {
            Ok(table) => table,
            Err(e) => {
                return Err(VectorDbError::LanceError(format!(
                    "Failed to open table: {}",
                    e
                )));
            }
        };

        // Perform the vector similarity search
        let results = table
            .query()
            .nearest_to(query_embedding)
            .unwrap()
            .execute_hybrid()
            .await
            .unwrap()
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| VectorDbError::LanceError(format!("Search failed: {}", e)))?;

        Ok(results)
    }
}

fn from_chunks_embeddings_to_data(
    chunk_embeddings: Vec<(Chunk, Vec<f32>)>,
    file_id: &str,
) -> RecordBatchIterator<
    std::iter::Map<
        std::vec::IntoIter<RecordBatch>,
        fn(RecordBatch) -> Result<RecordBatch, arrow_schema::ArrowError>,
    >,
> {
    let schema = get_embeddings_schema();

    let mut ids = Vec::with_capacity(chunk_embeddings.len());
    let mut texts = Vec::with_capacity(chunk_embeddings.len());
    let mut embeddings = Vec::with_capacity(chunk_embeddings.len());
    let mut file_ids = Vec::with_capacity(chunk_embeddings.len());

    for (i, (chunk, embedding)) in chunk_embeddings.iter().enumerate() {
        ids.push(format!("{}_chunk_{}", file_id, i));
        texts.push(chunk.content.clone());
        embeddings.push(Some(embedding.iter().map(|&f| Some(f)).collect::<Vec<_>>()));
        file_ids.push(file_id);
    }

    RecordBatchIterator::new(
        vec![RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(StringArray::from(ids)),
                Arc::new(StringArray::from(texts)),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(embeddings, 384),
                ),
                Arc::new(StringArray::from(file_ids)),
            ],
        )
        .unwrap()]
        .into_iter()
        .map(Ok),
        schema.clone(),
    )
}

#[tauri::command]
pub async fn init_vectordb(app_handle: AppHandle) -> VectorDbResult<Arc<Mutex<VectorDbManager>>> {
    VectorDbManager::initialize_vectordb(app_handle).await
}

fn get_embeddings_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                384, // embedding dimension
            ),
            false,
        ),
        Field::new("file_id", DataType::Utf8, false),
    ]))
}

// creates an index
// need at least 256 rows of data
// TODO: reinitialize this later in the flow
// async fn create_index(
//     client: &Connection,
//     table: &Table,
//     schema: Arc<Schema>,
// ) -> VectorDbResult<()> {
//     // open table
//     let table = match client.open_table(table.name()).execute().await {
//         Ok(table) => table,
//         Err(e) => {
//             return Err(VectorDbError::LanceError(format!(
//                 "Failed to open table: {}",
//                 e
//             )));
//         }
//     };

//     println!("open the embeddings table");
//     // We'll insert a single row with 384 zeros as the embedding since we can't create an index on an empty table
//     let batches = RecordBatchIterator::new(
//         vec![RecordBatch::try_new(
//             schema.clone(),
//             vec![
//                 // 1. id (Utf8)
//                 Arc::new(StringArray::from(vec!["dummy_id"])),
//                 // 2. text (Utf8)
//                 Arc::new(StringArray::from(vec!["dummy text"])),
//                 // 3. embedding (FixedSizeList of Float32)
//                 Arc::new(
//                     FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
//                         vec![Some(vec![Some(0.0); 384])], // 384-dimensional vector of zeros
//                         384,
//                     ),
//                 ),
//                 // 4. path (Utf8)
//                 Arc::new(StringArray::from(vec!["dummy/path"])),
//                 // 5. chunk_index (Int32)
//                 Arc::new(Int32Array::from(vec![0])),
//                 // 6. total_chunks (Int32, nullable)
//                 Arc::new(Int32Array::from(vec![Some(1)])),
//                 // 7. mime_type (Utf8, nullable)
//                 Arc::new(StringArray::from(vec![Some("text/plain")])),
//                 // 8. page_number (Int32, nullable)
//                 Arc::new(Int32Array::from(vec![Some(1)])),
//             ],
//         )
//         .unwrap()]
//         .into_iter()
//         .map(Ok),
//         schema.clone(),
//     );

//     println!("created a batch");

//     match table.add(Box::new(batches)).execute().await {
//         Ok(_) => {
//             println!("Successfully added dummy row")
//         }
//         Err(e) => {
//             return Err(VectorDbError::LanceError(format!(
//                 "Failed to add dummy row: {}",
//                 e
//             )))
//         }
//     }

//     println!("Creating vector index on 'embedding' column...");
//     if let Err(e) = table
//         .create_index(&["embedding"], Index::Auto)
//         .execute()
//         .await
//     {
//         return Err(VectorDbError::LanceError(format!(
//             "Failed to create index: {}",
//             e
//         )));
//     }

//     println!("Successfully created index on table '{}'.", table.name());

//     Ok(())
// }
