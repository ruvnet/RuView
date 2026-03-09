//! rUv Neural Embed -- Graph embedding generation for brain connectivity states.
//!
//! This crate provides multiple embedding methods to convert brain connectivity
//! graphs (`BrainGraph`) into fixed-dimensional vector representations suitable
//! for downstream classification, clustering, and temporal analysis.
//!
//! # Embedding Methods
//!
//! - **Spectral**: Laplacian eigenvector-based positional encoding
//! - **Topology**: Hand-crafted topological feature vectors
//! - **Node2Vec**: Random-walk co-occurrence embeddings
//! - **Combined**: Weighted concatenation of multiple methods
//! - **Temporal**: Sliding-window context-enriched embeddings
//!
//! # RVF Export
//!
//! Embeddings can be serialized to the RuVector `.rvf` format for interoperability
//! with the broader RuVector ecosystem.

pub mod combined;
pub mod distance;
pub mod node2vec;
pub mod rvf_export;
pub mod spectral_embed;
pub mod temporal;
pub mod topology_embed;

// Re-export core types used throughout this crate.
pub use ruv_neural_core::embedding::{EmbeddingMetadata, EmbeddingTrajectory, NeuralEmbedding};
pub use ruv_neural_core::graph::{BrainGraph, BrainGraphSequence};
pub use ruv_neural_core::traits::EmbeddingGenerator;

/// Helper to build an `EmbeddingMetadata` with just a method name and atlas.
pub fn default_metadata(
    method: &str,
    atlas: ruv_neural_core::brain::Atlas,
) -> EmbeddingMetadata {
    EmbeddingMetadata {
        subject_id: None,
        session_id: None,
        cognitive_state: None,
        source_atlas: atlas,
        embedding_method: method.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::brain::Atlas;

    #[test]
    fn test_neural_embedding_new() {
        let meta = default_metadata("test", Atlas::Custom(3));
        let emb = NeuralEmbedding::new(vec![1.0, 2.0, 3.0], 0.0, meta).unwrap();
        assert_eq!(emb.dimension, 3);
        assert_eq!(emb.vector.len(), 3);
    }

    #[test]
    fn test_neural_embedding_empty_fails() {
        let meta = default_metadata("test", Atlas::Custom(1));
        let result = NeuralEmbedding::new(vec![], 0.0, meta);
        assert!(result.is_err());
    }

    #[test]
    fn test_embedding_norm() {
        let meta = default_metadata("test", Atlas::Custom(2));
        let emb = NeuralEmbedding::new(vec![3.0, 4.0], 0.0, meta).unwrap();
        assert!((emb.norm() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_trajectory() {
        let traj = EmbeddingTrajectory {
            embeddings: vec![
                NeuralEmbedding::new(
                    vec![0.0; 4],
                    0.0,
                    default_metadata("test", Atlas::Custom(4)),
                )
                .unwrap(),
                NeuralEmbedding::new(
                    vec![0.0; 4],
                    0.5,
                    default_metadata("test", Atlas::Custom(4)),
                )
                .unwrap(),
                NeuralEmbedding::new(
                    vec![0.0; 4],
                    1.0,
                    default_metadata("test", Atlas::Custom(4)),
                )
                .unwrap(),
            ],
            timestamps: vec![0.0, 0.5, 1.0],
        };
        assert_eq!(traj.len(), 3);
        assert!((traj.duration_s() - 1.0).abs() < 1e-10);
    }
}
