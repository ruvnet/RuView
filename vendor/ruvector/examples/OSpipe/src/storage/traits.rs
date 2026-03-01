//! Embedding model trait abstraction.
//!
//! Defines the [`EmbeddingModel`] trait that all embedding providers must
//! implement, enabling pluggable embedding backends. Two implementations are
//! provided out of the box:
//!
//! - [`HashEmbeddingModel`] - deterministic hash-based embeddings (no semantic
//!   similarity, suitable for testing).
//! - [`RuvectorEmbeddingModel`] (native only) - wraps ruvector-core's
//!   [`EmbeddingProvider`](ruvector_core::embeddings::EmbeddingProvider) for
//!   real embedding backends (hash, candle, API-based).

/// Trait for generating vector embeddings from text.
///
/// Implementations must be `Send + Sync` so they can be shared across
/// threads.
pub trait EmbeddingModel: Send + Sync {
    /// Generate an embedding vector for the given text.
    fn embed(&self, text: &str) -> Vec<f32>;

    /// Generate embeddings for a batch of texts.
    ///
    /// The default implementation calls [`embed`](Self::embed) for each text
    /// sequentially. Implementations may override this for batched inference.
    fn batch_embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Return the dimensionality of embeddings produced by this model.
    fn dimension(&self) -> usize;
}

// ---------------------------------------------------------------------------
// HashEmbeddingModel (cross-platform, always available)
// ---------------------------------------------------------------------------

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::embedding::normalize;

/// Hash-based embedding model for testing and development.
///
/// Produces deterministic, L2-normalized vectors from text using
/// `DefaultHasher`. The vectors have no semantic meaning -- identical
/// inputs produce identical outputs, but semantically similar inputs
/// are *not* guaranteed to be close in vector space.
pub struct HashEmbeddingModel {
    dimension: usize,
}

impl HashEmbeddingModel {
    /// Create a new hash-based embedding model with the given dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingModel for HashEmbeddingModel {
    fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimension];
        for (i, val) in vector.iter_mut().enumerate() {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            text.hash(&mut hasher);
            let h = hasher.finish();
            *val = ((h as f64 / u64::MAX as f64) * 2.0 - 1.0) as f32;
        }
        normalize(&mut vector);
        vector
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

// ---------------------------------------------------------------------------
// RuvectorEmbeddingModel (native only -- wraps ruvector-core)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::EmbeddingModel;
    use crate::storage::embedding::normalize;
    use ruvector_core::embeddings::EmbeddingProvider;
    use std::sync::Arc;

    /// Embedding model backed by a ruvector-core [`EmbeddingProvider`].
    ///
    /// This wraps any `EmbeddingProvider` (e.g. `HashEmbedding`,
    /// `CandleEmbedding`, `ApiEmbedding`) behind the OSpipe
    /// [`EmbeddingModel`] trait, making the provider swappable at
    /// construction time.
    pub struct RuvectorEmbeddingModel {
        provider: Arc<dyn EmbeddingProvider>,
    }

    impl RuvectorEmbeddingModel {
        /// Create a new model wrapping the given provider.
        pub fn new(provider: Arc<dyn EmbeddingProvider>) -> Self {
            Self { provider }
        }

        /// Create a model using ruvector-core's `HashEmbedding` with the
        /// given dimension. This is the simplest way to get started on
        /// native targets.
        pub fn hash(dimensions: usize) -> Self {
            let provider = Arc::new(ruvector_core::embeddings::HashEmbedding::new(dimensions));
            Self { provider }
        }
    }

    impl EmbeddingModel for RuvectorEmbeddingModel {
        fn embed(&self, text: &str) -> Vec<f32> {
            match self.provider.embed(text) {
                Ok(mut v) => {
                    normalize(&mut v);
                    v
                }
                Err(e) => {
                    tracing::warn!("Embedding provider failed, returning zero vector: {}", e);
                    vec![0.0f32; self.provider.dimensions()]
                }
            }
        }

        fn dimension(&self) -> usize {
            self.provider.dimensions()
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::RuvectorEmbeddingModel;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_embedding_model_determinism() {
        let model = HashEmbeddingModel::new(128);
        let v1 = model.embed("hello world");
        let v2 = model.embed("hello world");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_hash_embedding_model_dimension() {
        let model = HashEmbeddingModel::new(64);
        assert_eq!(model.dimension(), 64);
        let v = model.embed("test");
        assert_eq!(v.len(), 64);
    }

    #[test]
    fn test_hash_embedding_model_normalized() {
        let model = HashEmbeddingModel::new(384);
        let v = model.embed("normalization test");
        let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (mag - 1.0).abs() < 1e-5,
            "Expected unit vector, got magnitude {}",
            mag,
        );
    }

    #[test]
    fn test_batch_embed() {
        let model = HashEmbeddingModel::new(64);
        let texts: Vec<&str> = vec!["one", "two", "three"];
        let embeddings = model.batch_embed(&texts);
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 64);
        }
    }

    #[test]
    fn test_trait_object_dispatch() {
        let model: Box<dyn EmbeddingModel> = Box::new(HashEmbeddingModel::new(32));
        let v = model.embed("dispatch test");
        assert_eq!(v.len(), 32);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn test_ruvector_embedding_model() {
        let model = RuvectorEmbeddingModel::hash(128);
        let v = model.embed("ruvector test");
        assert_eq!(v.len(), 128);
        assert_eq!(model.dimension(), 128);

        // Should be normalized
        let mag: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (mag - 1.0).abs() < 1e-4,
            "Expected unit vector, got magnitude {}",
            mag,
        );
    }
}
