//! Embedding generation engine.
//!
//! This module provides a deterministic hash-based embedding engine for
//! development and testing. In production, this would be replaced with
//! a real model (ONNX, Candle, or an API-based provider via ruvector-core's
//! EmbeddingProvider trait).
//!
//! `EmbeddingEngine` also implements [`EmbeddingModel`]
//! so it can be used anywhere a trait-based embedding source is required.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use super::traits::EmbeddingModel;

/// Engine that generates vector embeddings from text.
///
/// The current implementation uses a deterministic hash-based approach
/// that produces consistent embeddings for the same input text. This is
/// suitable for testing deduplication and search mechanics, but does NOT
/// provide semantic similarity. For semantic search, integrate a real
/// embedding model.
pub struct EmbeddingEngine {
    dimension: usize,
}

impl EmbeddingEngine {
    /// Create a new embedding engine with the given vector dimension.
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Generate an embedding vector for the given text.
    ///
    /// The resulting vector is L2-normalized so that cosine similarity
    /// can be computed as a simple dot product.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dimension];

        // Generate deterministic pseudo-random values from text hash
        // We use multiple hash passes with different seeds to fill the vector.
        for (i, val) in vector.iter_mut().enumerate() {
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            text.hash(&mut hasher);
            let h = hasher.finish();
            // Map to [-1, 1] range
            *val = ((h as f64 / u64::MAX as f64) * 2.0 - 1.0) as f32;
        }

        // L2-normalize the vector
        normalize(&mut vector);
        vector
    }

    /// Generate embeddings for a batch of texts.
    pub fn batch_embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Return the dimensionality of embeddings produced by this engine.
    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

/// `EmbeddingEngine` satisfies [`EmbeddingModel`] so existing code can
/// pass an `&EmbeddingEngine` wherever a `&dyn EmbeddingModel` is needed.
impl EmbeddingModel for EmbeddingEngine {
    fn embed(&self, text: &str) -> Vec<f32> {
        EmbeddingEngine::embed(self, text)
    }

    fn batch_embed(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        EmbeddingEngine::batch_embed(self, texts)
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

/// L2-normalize a vector in place. If the vector has zero magnitude,
/// it is left unchanged.
pub fn normalize(vector: &mut [f32]) {
    let magnitude: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > f32::EPSILON {
        for val in vector.iter_mut() {
            *val /= magnitude;
        }
    }
}

/// Compute cosine similarity between two L2-normalized vectors.
///
/// For normalized vectors, cosine similarity equals the dot product.
/// Returns a value in [-1.0, 1.0].
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "Vectors must have equal dimensions");
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_determinism() {
        let engine = EmbeddingEngine::new(384);
        let v1 = engine.embed("hello world");
        let v2 = engine.embed("hello world");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_embedding_dimension() {
        let engine = EmbeddingEngine::new(128);
        let v = engine.embed("test");
        assert_eq!(v.len(), 128);
    }

    #[test]
    fn test_embedding_normalized() {
        let engine = EmbeddingEngine::new(384);
        let v = engine.embed("test normalization");
        let magnitude: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (magnitude - 1.0).abs() < 1e-5,
            "Expected unit vector, got magnitude {}",
            magnitude
        );
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let engine = EmbeddingEngine::new(384);
        let v = engine.embed("same text");
        let sim = cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_cosine_similarity_different() {
        let engine = EmbeddingEngine::new(384);
        let v1 = engine.embed("hello world");
        let v2 = engine.embed("completely different text about cats");
        let sim = cosine_similarity(&v1, &v2);
        // Hash-based embeddings won't give semantic similarity,
        // but different texts should generally not be identical.
        assert!(sim < 1.0);
    }

    #[test]
    fn test_batch_embed() {
        let engine = EmbeddingEngine::new(64);
        let texts = vec!["one", "two", "three"];
        let embeddings = engine.batch_embed(&texts);
        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.len(), 64);
        }
    }
}
