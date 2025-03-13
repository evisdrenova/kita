use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::Arc;

// struct to hold your embedding model
pub struct Embedder {
    model: Arc<TextEmbedding>,
}

impl Embedder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let init_options =
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

        let model = TextEmbedding::try_new(init_options)?;

        Ok(Self {
            model: Arc::new(model),
        })
    }

    // Get embeddings for a single text
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        match self.model.embed(vec![text], None) {
            Ok(embeddings) => {
                if !embeddings.is_empty() {
                    embeddings[0].clone()
                } else {
                    Vec::new() // Empty embedding if something went wrong
                }
            }
            Err(_) => Vec::new(),
        }
    }

    // // Get embeddings for a batch of texts
    // pub fn embed_batch(&self, texts: &[String]) -> Vec<Vec<f32>> {
    //     match self.model.embed_batch(texts, None) {
    //         Ok(embeddings) => embeddings,
    //         Err(_) => vec![Vec::new(); texts.len()],
    //     }
    // }
}
