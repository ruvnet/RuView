//! # EXO Backend Classical
//!
//! Classical substrate backend consuming ruvector crates.
//! This provides a bridge between the EXO substrate abstractions and the
//! high-performance ruvector vector database and graph database.

#![warn(missing_docs)]

pub mod dither_quantizer;
pub mod domain_bridge;
pub mod graph;
pub mod thermo_layer;
pub mod transfer_orchestrator;
pub mod vector;

use exo_core::{
    Error as ExoError, Filter, ManifoldDelta, Pattern, Result as ExoResult, SearchResult,
    SubstrateBackend,
};
use parking_lot::RwLock;
use std::sync::Arc;
use vector::VectorIndexWrapper;

pub use graph::GraphWrapper;

/// Configuration for the classical backend
#[derive(Debug, Clone)]
pub struct ClassicalConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Distance metric
    pub distance_metric: ruvector_core::DistanceMetric,
}

impl Default for ClassicalConfig {
    fn default() -> Self {
        Self {
            dimensions: 768,
            distance_metric: ruvector_core::DistanceMetric::Cosine,
        }
    }
}

/// Classical substrate backend using ruvector
///
/// This backend wraps ruvector-core for vector operations and ruvector-graph
/// for hypergraph operations, providing a classical (discrete) implementation
/// of the substrate backend trait.
pub struct ClassicalBackend {
    /// Vector index wrapper
    vector_index: Arc<RwLock<VectorIndexWrapper>>,
    /// Graph database wrapper
    graph_db: Arc<RwLock<GraphWrapper>>,
    /// Configuration
    config: ClassicalConfig,
}

impl ClassicalBackend {
    /// Create a new classical backend with the given configuration
    pub fn new(config: ClassicalConfig) -> ExoResult<Self> {
        let vector_index = VectorIndexWrapper::new(config.dimensions, config.distance_metric)
            .map_err(|e| ExoError::Backend(format!("Failed to create vector index: {}", e)))?;

        let graph_db = GraphWrapper::new();

        Ok(Self {
            vector_index: Arc::new(RwLock::new(vector_index)),
            graph_db: Arc::new(RwLock::new(graph_db)),
            config,
        })
    }

    /// Create with default configuration
    pub fn with_dimensions(dimensions: usize) -> ExoResult<Self> {
        let mut config = ClassicalConfig::default();
        config.dimensions = dimensions;
        Self::new(config)
    }

    /// Get access to the underlying graph database (for hyperedge operations)
    pub fn graph_db(&self) -> Arc<RwLock<GraphWrapper>> {
        Arc::clone(&self.graph_db)
    }
}

impl SubstrateBackend for ClassicalBackend {
    fn similarity_search(
        &self,
        query: &[f32],
        k: usize,
        filter: Option<&Filter>,
    ) -> ExoResult<Vec<SearchResult>> {
        // Validate dimensions
        if query.len() != self.config.dimensions {
            return Err(ExoError::InvalidDimension {
                expected: self.config.dimensions,
                got: query.len(),
            });
        }

        // Delegate to vector index wrapper
        let index = self.vector_index.read();
        index.search(query, k, filter)
    }

    fn manifold_deform(&self, pattern: &Pattern, _learning_rate: f32) -> ExoResult<ManifoldDelta> {
        // Validate dimensions
        if pattern.embedding.len() != self.config.dimensions {
            return Err(ExoError::InvalidDimension {
                expected: self.config.dimensions,
                got: pattern.embedding.len(),
            });
        }

        // Classical backend: discrete insert (no continuous deformation)
        let mut index = self.vector_index.write();
        let id = index.insert(pattern)?;

        Ok(ManifoldDelta::DiscreteInsert { id })
    }

    fn dimension(&self) -> usize {
        self.config.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use exo_core::{Metadata, PatternId, SubstrateTime};

    #[test]
    fn test_classical_backend_creation() {
        let backend = ClassicalBackend::with_dimensions(128).unwrap();
        assert_eq!(backend.dimension(), 128);
    }

    #[test]
    fn test_insert_and_search() {
        let backend = ClassicalBackend::with_dimensions(3).unwrap();

        // Create a pattern
        let pattern = Pattern {
            id: PatternId::new(),
            embedding: vec![1.0, 2.0, 3.0],
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: vec![],
            salience: 1.0,
        };

        // Insert pattern
        let result = backend.manifold_deform(&pattern, 0.0);
        assert!(result.is_ok());

        // Search
        let query = vec![1.1, 2.1, 3.1];
        let results = backend.similarity_search(&query, 1, None);
        assert!(results.is_ok());
        let results = results.unwrap();
        assert_eq!(results.len(), 1);
    }
}
