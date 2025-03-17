use arrow_array::types::Float32Type;
use arrow_array::{FixedSizeListArray, Int32Array, RecordBatch, RecordBatchIterator};
use arrow_schema::{DataType, Field, Schema};
use lancedb::index::Index;
use lancedb::{Connection, Error, Table};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::AppHandle;
use tauri::Manager;
use thiserror::Error;

pub struct VectorDbManager {
    client: Connection,
}

pub struct VectorDbState {
    pub manager: Arc<Mutex<VectorDbManager>>,
}

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

        Self::init_table(&client).await?;

        Ok(Self { client })
    }

    // checks to see if vector table exists if not, sets it up
    async fn init_table(client: &Connection) -> VectorDbResult<()> {
        const TABLE_NAME: &str = "embeddings";

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
                // Any other error is unexpected, so return it
                println!("some other error'{:?}' exists", e);
                false
            }
        };

        let schema = Arc::new(Schema::new(vec![
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
            Field::new("path", DataType::Utf8, false),
            Field::new("chunk_index", DataType::Int32, false),
            Field::new("total_chunks", DataType::Int32, true),
            Field::new("mime_type", DataType::Utf8, true),
            Field::new("page_number", DataType::Int32, true),
        ]));

        let schema_clone: Arc<Schema> = schema.clone();

        if !table_exists {
            let table = create_table(client, TABLE_NAME, &schema_clone).await?;
            create_index(client, &table, schema).await?;
        }

        Ok(())
    }
}

#[tauri::command]
pub async fn init_vectordb(app_handle: AppHandle) -> VectorDbResult<Arc<Mutex<VectorDbManager>>> {
    VectorDbManager::initialize_vectordb(app_handle).await
}

async fn create_table(
    client: &Connection,
    table_name: &str,
    schema: &Arc<Schema>,
) -> VectorDbResult<Table> {
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

async fn create_index(
    client: &Connection,
    table: &Table,
    schema: Arc<Schema>,
) -> VectorDbResult<()> {
    // open table
    let table = match client.open_table(table.name()).execute().await {
        Ok(table) => table,
        Err(e) => {
            return Err(VectorDbError::LanceError(format!(
                "Failed to open table: {}",
                e
            )));
        }
    };
    // We'll insert a single row with 384 zeros as the embedding since we can't create an index on an empty table
    let batches = RecordBatchIterator::new(
        vec![RecordBatch::try_new(
            schema.clone(),
            vec![
                Arc::new(Int32Array::from_iter_values(0..384)),
                Arc::new(
                    FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                        (0..384).map(|_| Some(vec![Some(1.0); 128])),
                        128,
                    ),
                ),
            ],
        )
        .unwrap()]
        .into_iter()
        .map(Ok),
        schema.clone(),
    );

    match table.add(Box::new(batches)).execute().await {
        Ok(_) => {
            println!("Successfully added dummy row")
        }
        Err(e) => {
            return Err(VectorDbError::LanceError(format!(
                "Failed to add dummy row: {}",
                e
            )))
        }
    }

    println!("Creating vector index on 'embedding' column...");
    if let Err(e) = table
        .create_index(&["embedding"], Index::Auto)
        .execute()
        .await
    {
        return Err(VectorDbError::LanceError(format!(
            "Failed to create index: {}",
            e
        )));
    }

    println!("Successfully created index on table '{}'.", table.name());

    Ok(())
}
