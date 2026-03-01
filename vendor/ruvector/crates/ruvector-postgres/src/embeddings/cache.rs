//! Thread-safe model caching with lazy loading

use dashmap::DashMap;
use fastembed::{EmbeddingModel as FastEmbedModel, InitOptions, TextEmbedding};
use parking_lot::RwLock;

use super::models::EmbeddingModel;

/// Global model cache for lazy loading and reuse
pub struct ModelCache {
    /// Cached embedding models (using RwLock for interior mutability)
    models: DashMap<EmbeddingModel, RwLock<TextEmbedding>>,
    /// Default model setting
    default_model: RwLock<EmbeddingModel>,
}

impl ModelCache {
    /// Create a new model cache
    pub fn new() -> Self {
        Self {
            models: DashMap::new(),
            default_model: RwLock::new(EmbeddingModel::default()),
        }
    }

    /// Get or load a model and generate embeddings
    pub fn embed(&self, model: EmbeddingModel, texts: Vec<&str>) -> Result<Vec<Vec<f32>>, String> {
        // Check if already cached
        if let Some(cached) = self.models.get(&model) {
            let mut embedding = cached.write();
            return embedding
                .embed(texts, None)
                .map_err(|e| format!("Embedding failed: {}", e));
        }

        // Load the model
        let embedding = self.load_model(model)?;

        // Generate embeddings first
        let mut embedding_model = embedding;
        let result = embedding_model
            .embed(texts, None)
            .map_err(|e| format!("Embedding failed: {}", e));

        // Cache the model
        self.models.insert(model, RwLock::new(embedding_model));

        result
    }

    /// Load a model from fastembed
    fn load_model(&self, model: EmbeddingModel) -> Result<TextEmbedding, String> {
        let fastembed_model = match model {
            EmbeddingModel::AllMiniLmL6V2 => FastEmbedModel::AllMiniLML6V2,
            EmbeddingModel::BgeSmallEnV15 => FastEmbedModel::BGESmallENV15,
            EmbeddingModel::BgeBaseEnV15 => FastEmbedModel::BGEBaseENV15,
            EmbeddingModel::BgeLargeEnV15 => FastEmbedModel::BGELargeENV15,
            EmbeddingModel::AllMpnetBaseV2 => FastEmbedModel::AllMiniLML6V2, // Fallback
            EmbeddingModel::NomicEmbedTextV15 => FastEmbedModel::NomicEmbedTextV15,
        };

        let options = InitOptions::new(fastembed_model).with_show_download_progress(false);

        TextEmbedding::try_new(options)
            .map_err(|e| format!("Failed to load model '{}': {}", model.name(), e))
    }

    /// Pre-load a model into the cache
    pub fn preload(&self, model: EmbeddingModel) -> Result<(), String> {
        if self.models.contains_key(&model) {
            return Ok(());
        }
        let embedding = self.load_model(model)?;
        self.models.insert(model, RwLock::new(embedding));
        Ok(())
    }

    /// Check if a model is loaded
    pub fn is_loaded(&self, model: EmbeddingModel) -> bool {
        self.models.contains_key(&model)
    }

    /// Get list of loaded models
    pub fn loaded_models(&self) -> Vec<EmbeddingModel> {
        self.models.iter().map(|r| *r.key()).collect()
    }

    /// Unload a model from cache
    pub fn unload(&self, model: EmbeddingModel) -> bool {
        self.models.remove(&model).is_some()
    }

    /// Clear all cached models
    pub fn clear(&self) {
        self.models.clear();
    }

    /// Get the default model
    pub fn default_model(&self) -> EmbeddingModel {
        *self.default_model.read()
    }

    /// Set the default model
    pub fn set_default_model(&self, model: EmbeddingModel) {
        *self.default_model.write() = model;
    }

    /// Get memory usage estimate in bytes
    pub fn estimated_memory_usage(&self) -> usize {
        self.models
            .iter()
            .map(|r| r.key().memory_mb() * 1024 * 1024)
            .sum()
    }
}

impl Default for ModelCache {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton cache
lazy_static::lazy_static! {
    pub static ref GLOBAL_CACHE: ModelCache = ModelCache::new();
}

/// Get the global model cache
pub fn global_cache() -> &'static ModelCache {
    &GLOBAL_CACHE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = ModelCache::new();
        assert!(!cache.is_loaded(EmbeddingModel::AllMiniLmL6V2));
        assert!(cache.loaded_models().is_empty());
    }

    #[test]
    fn test_default_model() {
        let cache = ModelCache::new();
        assert_eq!(cache.default_model(), EmbeddingModel::AllMiniLmL6V2);

        cache.set_default_model(EmbeddingModel::BgeSmallEnV15);
        assert_eq!(cache.default_model(), EmbeddingModel::BgeSmallEnV15);
    }
}
