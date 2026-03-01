//! Core traits and types for DAG attention mechanisms

use crate::dag::QueryDag;
use std::collections::HashMap;

/// Attention scores for DAG nodes
pub type AttentionScores = HashMap<usize, f32>;

/// Configuration for attention mechanisms
#[derive(Debug, Clone)]
pub struct AttentionConfig {
    pub normalize: bool,
    pub temperature: f32,
    pub dropout: f32,
}

impl Default for AttentionConfig {
    fn default() -> Self {
        Self {
            normalize: true,
            temperature: 1.0,
            dropout: 0.0,
        }
    }
}

/// Errors from attention computation
#[derive(Debug, thiserror::Error)]
pub enum AttentionError {
    #[error("Empty DAG")]
    EmptyDag,
    #[error("Cycle detected in DAG")]
    CycleDetected,
    #[error("Node {0} not found")]
    NodeNotFound(usize),
    #[error("Computation failed: {0}")]
    ComputationFailed(String),
}

/// Trait for DAG attention mechanisms
pub trait DagAttention: Send + Sync {
    /// Compute attention scores for all nodes
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScores, AttentionError>;

    /// Update internal state after execution feedback
    fn update(&mut self, dag: &QueryDag, execution_times: &HashMap<usize, f64>);

    /// Get mechanism name
    fn name(&self) -> &'static str;

    /// Get computational complexity description
    fn complexity(&self) -> &'static str;
}
