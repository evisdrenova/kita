use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

/// Holds embedding model
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let init_options: InitOptions =
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

        let model = TextEmbedding::try_new(init_options)?;

        Ok(Self { model: model })
    }

    /// Get embeddings for a single chunk of text
    /// If there is an error this will return back an empty vector
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        self.model
            .embed(vec![text], None)
            .map(|embeddings| embeddings.get(0).cloned().unwrap_or_default())
            .unwrap_or_default()
    }

    // // Get embeddings for a batch of texts
    // pub fn embed_batch(&self, texts: &[String]) -> Vec<Vec<f32>> {
    //     match self.model.embed_batch(texts, None) {
    //         Ok(embeddings) => embeddings,
    //         Err(_) => vec![Vec::new(); texts.len()],
    //     }
    // }
}
