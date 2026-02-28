//! Learned Manifold Engine for EXO-AI Cognitive Substrate
//!
//! This crate implements a simplified manifold storage system.
//! The burn dependency has been removed to avoid bincode version conflicts.
//!
//! # Key Concepts
//!
//! - **Retrieval**: Vector similarity search
//! - **Storage**: Pattern storage with embeddings
//! - **Forgetting**: Strategic pattern pruning
//!
//! # Architecture
//!
//! ```text
//! Query → Vector Search → Nearest Patterns
//!            ↓
//!      Pattern Storage
//!      (Vec-based)
//!            ↓
//!      Similarity Scores
//! ```

use exo_core::{Error, ManifoldConfig, ManifoldDelta, Pattern, Result, SearchResult};
use parking_lot::RwLock;
use std::sync::Arc;

mod deformation;
mod forgetting;
mod network;
mod retrieval;
pub mod simd_ops;
pub mod transfer_store;

pub use deformation::ManifoldDeformer;
pub use forgetting::StrategicForgetting;
pub use network::LearnedManifold;
pub use retrieval::GradientDescentRetriever;
pub use simd_ops::{batch_distances, cosine_similarity_simd, euclidean_distance_simd};

/// Simplified manifold storage using vector similarity
pub struct ManifoldEngine {
    /// Simple pattern storage
    network: Arc<RwLock<LearnedManifold>>,
    /// Configuration
    config: ManifoldConfig,
    /// Stored patterns (for extraction)
    patterns: Arc<RwLock<Vec<Pattern>>>,
}

impl ManifoldEngine {
    /// Create a new manifold engine
    pub fn new(config: ManifoldConfig) -> Self {
        let network =
            LearnedManifold::new(config.dimension, config.hidden_dim, config.hidden_layers);

        Self {
            network: Arc::new(RwLock::new(network)),
            config,
            patterns: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Query manifold via vector similarity
    pub fn retrieve(&self, query: &[f32], k: usize) -> Result<Vec<SearchResult>> {
        if query.len() != self.config.dimension {
            return Err(Error::InvalidDimension {
                expected: self.config.dimension,
                got: query.len(),
            });
        }

        let retriever = GradientDescentRetriever::new(self.network.clone(), self.config.clone());

        retriever.retrieve(query, k, &self.patterns)
    }

    /// Store pattern (simplified deformation)
    pub fn deform(&mut self, pattern: Pattern, salience: f32) -> Result<ManifoldDelta> {
        if pattern.embedding.len() != self.config.dimension {
            return Err(Error::InvalidDimension {
                expected: self.config.dimension,
                got: pattern.embedding.len(),
            });
        }

        // Store pattern for later extraction
        self.patterns.write().push(pattern.clone());

        let mut deformer = ManifoldDeformer::new(self.network.clone(), self.config.learning_rate);

        deformer.deform(&pattern, salience)
    }

    /// Strategic forgetting via pattern pruning
    pub fn forget(&mut self, salience_threshold: f32, decay_rate: f32) -> Result<usize> {
        let forgetter = StrategicForgetting::new(self.network.clone());

        forgetter.forget(&self.patterns, salience_threshold, decay_rate)
    }

    /// Get number of stored patterns
    pub fn len(&self) -> usize {
        self.patterns.read().len()
    }

    /// Check if engine is empty
    pub fn is_empty(&self) -> bool {
        self.patterns.read().is_empty()
    }

    /// Get configuration
    pub fn config(&self) -> &ManifoldConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::{Metadata, PatternId, SubstrateTime};

    fn create_test_pattern(embedding: Vec<f32>, salience: f32) -> Pattern {
        Pattern {
            id: PatternId::new(),
            embedding,
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: vec![],
            salience,
        }
    }

    #[test]
    fn test_manifold_engine_creation() {
        let config = ManifoldConfig {
            dimension: 128,
            ..Default::default()
        };
        let engine = ManifoldEngine::new(config);

        assert_eq!(engine.len(), 0);
        assert!(engine.is_empty());
        assert_eq!(engine.config().dimension, 128);
    }

    #[test]
    fn test_deform_and_retrieve() {
        let config = ManifoldConfig {
            dimension: 64,
            max_descent_steps: 10,
            learning_rate: 0.01,
            ..Default::default()
        };
        let mut engine = ManifoldEngine::new(config);

        // Create and deform with a pattern
        let embedding = vec![1.0; 64];
        let pattern = create_test_pattern(embedding.clone(), 0.9);

        let result = engine.deform(pattern, 0.9);
        assert!(result.is_ok());
        assert_eq!(engine.len(), 1);

        // Retrieve similar patterns
        let results = engine.retrieve(&embedding, 1);
        assert!(results.is_ok());
    }

    #[test]
    fn test_invalid_dimension() {
        let config = ManifoldConfig {
            dimension: 128,
            ..Default::default()
        };
        let mut engine = ManifoldEngine::new(config);

        // Wrong dimension
        let embedding = vec![1.0; 64];
        let pattern = create_test_pattern(embedding.clone(), 0.9);

        let result = engine.deform(pattern, 0.9);
        assert!(result.is_err());

        let retrieve_result = engine.retrieve(&embedding, 1);
        assert!(retrieve_result.is_err());
    }
}
