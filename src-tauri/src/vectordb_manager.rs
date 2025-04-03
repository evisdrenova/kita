use arrow_array::types::Float32Type;
use arrow_array::FixedSizeListArray;
use arrow_array::RecordBatch;
use arrow_array::RecordBatchIterator;
use arrow_array::StringArray;
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::ExecutableQuery;
use lancedb::query::QueryExecutionOptions;
use lancedb::{Connection, Error};
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
    pub async fn initialize_vectordb(
        app_handle: AppHandle,
    ) -> VectorDbResult<Arc<Mutex<VectorDbManager>>> {
        let app_data_dir: PathBuf = app_handle
            .path()
            .app_data_dir()
            .map_err(|_| VectorDbError::Other("Failed to get app data directory".into()))?;

        let vectordb_path: PathBuf = app_data_dir.join("vector_db");

        let manager: VectorDbManager = Self::new_vectordb_client(&vectordb_path).await?;

        Ok(Arc::new(Mutex::new(manager)))
    }

    async fn new_vectordb_client(vdb_path: &PathBuf) -> VectorDbResult<Self> {
        let client = lancedb::connect(&vdb_path.to_string_lossy())
            .execute()
            .await
            .map_err(|e| {
                println!("Unable to create LanceDB client: {}", e);
                VectorDbError::LanceError(e.to_string())
            })?;

        let instance: VectorDbManager = Self { client };

        instance.ensure_embedding_table_exists().await?;

        Ok(instance)
    }

    async fn ensure_embedding_table_exists(&self) -> VectorDbResult<()> {
        let table_exists = match self.client.open_table(TABLE_NAME).execute().await {
            Ok(_) => true,
            Err(Error::TableNotFound { name }) if name == TABLE_NAME => false,
            Err(e) => {
                return Err(VectorDbError::LanceError(format!(
                    "Error checking table: {}",
                    e
                )));
            }
        };

        if !table_exists {
            let schema = get_embeddings_schema();
            self.client
                .create_empty_table(TABLE_NAME, schema)
                .execute()
                .await
                .map_err(|e| VectorDbError::LanceError(format!("Failed to create table: {}", e)))?;
        }

        Ok(())
    }

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

    /// given a query, this function performs similarity search and returns the chunks that matched
    pub async fn search_similar(
        app_handle: &AppHandle,
        query_text: &str,
    ) -> VectorDbResult<Vec<RecordBatch>> {
        println!("the query in search similar: {:?}", query_text);

        let state = app_handle.state::<Arc<Mutex<VectorDbManager>>>();
        let manager = state.lock().await;

        if let Err(e) = manager.ensure_embedding_table_exists().await {
            println!("Error ensuring table exists: {}", e);
            return Ok(Vec::new());
        }

        let embedder = app_handle.state::<Arc<Embedder>>();
        let query_embedding: Vec<f32> = embedder.embed_single_text(query_text);

        let table = manager
            .client
            .open_table(TABLE_NAME)
            .execute()
            .await
            .map_err(|e| VectorDbError::LanceError(format!("Failed to open table: {}", e)))?;

        let query_options: QueryExecutionOptions = QueryExecutionOptions::default();

        let vector_query = table.query().nearest_to(query_embedding).map_err(|e| {
            VectorDbError::LanceError(format!("Failed to create vector query: {}", e))
        })?;

        let nev_vec = vector_query
            .distance_type(lancedb::DistanceType::Cosine)
            .clone();

        let results: Vec<RecordBatch> = nev_vec
            .execute_with_options(query_options)
            .await
            .map_err(|e| VectorDbError::LanceError(format!("Vector search failed: {}", e)))?
            .try_collect::<Vec<_>>()
            .await
            .map_err(|e| {
                VectorDbError::LanceError(format!("Vector search collection failed: {}", e))
            })?;

        println!("the similarity search results: {:?}", results);

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

pub fn get_text_chunks_from_similarity_search(results: Vec<RecordBatch>) -> Result<String, String> {
    let top_n = 5; // Limit to top 5 most relevant chunks

    // Extract and format the chunks
    let mut context_chunks = Vec::new();
    for batch in &results {
        // Access the columns
        let ids = batch
            .column_by_name("id")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .expect("Expected 'id' column to be a StringArray");

        let texts = batch
            .column_by_name("text")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .expect("Expected 'text' column to be a StringArray");

        let file_ids = batch
            .column_by_name("file_id")
            .unwrap()
            .as_any()
            .downcast_ref::<arrow_array::StringArray>()
            .expect("Expected 'file_id' column to be a StringArray");

        // Build formatted context chunks
        for i in 0..std::cmp::min(batch.num_rows(), top_n) {
            let id = ids.value(i);
            let text = texts.value(i);
            let file_id = file_ids.value(i);

            context_chunks.push(format!(
                "[{}] <source>{}</source>\n{}",
                i + 1,
                file_id,
                text
            ));
        }
    }

    // Join the chunks with newlines between them
    Ok(context_chunks.join("\n\n"))
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
