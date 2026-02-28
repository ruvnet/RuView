//! DagAttention trait definition for pluggable attention mechanisms

use crate::dag::QueryDag;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Attention scores for each node in the DAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionScores {
    /// Attention score for each node (0.0 to 1.0)
    pub scores: Vec<f32>,
    /// Optional attention weights between nodes (adjacency-like)
    pub edge_weights: Option<Vec<Vec<f32>>>,
    /// Metadata for debugging
    pub metadata: HashMap<String, String>,
}

impl AttentionScores {
    pub fn new(scores: Vec<f32>) -> Self {
        Self {
            scores,
            edge_weights: None,
            metadata: HashMap::new(),
        }
    }

    pub fn with_edge_weights(mut self, weights: Vec<Vec<f32>>) -> Self {
        self.edge_weights = Some(weights);
        self
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Errors that can occur during attention computation
#[derive(Debug, Error)]
pub enum AttentionError {
    #[error("Invalid DAG structure: {0}")]
    InvalidDag(String),

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    #[error("Computation failed: {0}")]
    ComputationFailed(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),
}

/// Trait for DAG attention mechanisms
pub trait DagAttentionMechanism: Send + Sync {
    /// Compute attention scores for the given DAG
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError>;

    /// Get the mechanism name
    fn name(&self) -> &'static str;

    /// Get computational complexity as a string
    fn complexity(&self) -> &'static str;

    /// Optional: Update internal state based on execution feedback
    fn update(&mut self, _dag: &QueryDag, _execution_times: &HashMap<usize, f64>) {
        // Default: no-op
    }

    /// Optional: Reset internal state
    fn reset(&mut self) {
        // Default: no-op
    }
}
