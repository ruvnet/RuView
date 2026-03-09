//! Vector embedding types for neural state representations.

use serde::{Deserialize, Serialize};

use crate::brain::Atlas;
use crate::error::{Result, RuvNeuralError};
use crate::topology::CognitiveState;

/// Neural state embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralEmbedding {
    /// The embedding vector.
    pub vector: Vec<f64>,
    /// Dimensionality of the embedding.
    pub dimension: usize,
    /// Timestamp (Unix time).
    pub timestamp: f64,
    /// Associated metadata.
    pub metadata: EmbeddingMetadata,
}

impl NeuralEmbedding {
    /// Create a new embedding, validating dimension consistency.
    pub fn new(vector: Vec<f64>, timestamp: f64, metadata: EmbeddingMetadata) -> Result<Self> {
        let dimension = vector.len();
        if dimension == 0 {
            return Err(RuvNeuralError::Embedding(
                "Embedding vector must not be empty".into(),
            ));
        }
        Ok(Self {
            vector,
            dimension,
            timestamp,
            metadata,
        })
    }

    /// L2 norm of the embedding vector.
    pub fn norm(&self) -> f64 {
        self.vector.iter().map(|x| x * x).sum::<f64>().sqrt()
    }

    /// Cosine similarity to another embedding.
    pub fn cosine_similarity(&self, other: &NeuralEmbedding) -> Result<f64> {
        if self.dimension != other.dimension {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: self.dimension,
                got: other.dimension,
            });
        }
        let dot: f64 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        let norm_a = self.norm();
        let norm_b = other.norm();
        if norm_a == 0.0 || norm_b == 0.0 {
            return Ok(0.0);
        }
        Ok(dot / (norm_a * norm_b))
    }

    /// Euclidean distance to another embedding.
    pub fn euclidean_distance(&self, other: &NeuralEmbedding) -> Result<f64> {
        if self.dimension != other.dimension {
            return Err(RuvNeuralError::DimensionMismatch {
                expected: self.dimension,
                got: other.dimension,
            });
        }
        let sum_sq: f64 = self
            .vector
            .iter()
            .zip(other.vector.iter())
            .map(|(a, b)| (a - b) * (a - b))
            .sum();
        Ok(sum_sq.sqrt())
    }
}

/// Metadata associated with a neural embedding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingMetadata {
    /// Subject identifier.
    pub subject_id: Option<String>,
    /// Session identifier.
    pub session_id: Option<String>,
    /// Decoded cognitive state (if available).
    pub cognitive_state: Option<CognitiveState>,
    /// Atlas used for the source graph.
    pub source_atlas: Atlas,
    /// Name of the embedding method (e.g., "spectral", "node2vec").
    pub embedding_method: String,
}

/// Temporal sequence of embeddings (trajectory through embedding space).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingTrajectory {
    /// Ordered sequence of embeddings.
    pub embeddings: Vec<NeuralEmbedding>,
    /// Timestamps for each embedding.
    pub timestamps: Vec<f64>,
}

impl EmbeddingTrajectory {
    /// Number of time points.
    pub fn len(&self) -> usize {
        self.embeddings.len()
    }

    /// Returns true if the trajectory is empty.
    pub fn is_empty(&self) -> bool {
        self.embeddings.is_empty()
    }

    /// Total duration in seconds.
    pub fn duration_s(&self) -> f64 {
        if self.timestamps.len() < 2 {
            return 0.0;
        }
        self.timestamps.last().unwrap() - self.timestamps.first().unwrap()
    }
}
