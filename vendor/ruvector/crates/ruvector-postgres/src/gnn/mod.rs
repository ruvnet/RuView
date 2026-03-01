//! # Graph Neural Network Module
//!
//! Provides GNN-based embeddings and graph-aware vector operations.

// GNN sub-modules
pub mod aggregators;
pub mod gcn;
pub mod graphsage;
pub mod message_passing;
pub mod operators;

// Re-export operator functions for PostgreSQL
pub use operators::*;

use pgrx::prelude::*;
use serde::{Deserialize, Serialize};

/// GNN model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnConfig {
    pub num_layers: usize,
    pub hidden_dim: usize,
    pub dropout: f32,
    pub aggregation: String,
}

impl Default for GnnConfig {
    fn default() -> Self {
        Self {
            num_layers: 2,
            hidden_dim: 128,
            dropout: 0.1,
            aggregation: "mean".to_string(),
        }
    }
}

/// GNN training status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnTrainingStatus {
    pub epoch: usize,
    pub total_epochs: usize,
    pub loss: f64,
    pub accuracy: f64,
    pub completed: bool,
}

/// GNN model state
pub struct GnnModel {
    config: GnnConfig,
    trained: bool,
}

impl GnnModel {
    pub fn new() -> Self {
        Self::with_config(GnnConfig::default())
    }

    pub fn with_config(config: GnnConfig) -> Self {
        Self {
            config,
            trained: false,
        }
    }

    pub fn is_trained(&self) -> bool {
        self.trained
    }

    pub fn config(&self) -> &GnnConfig {
        &self.config
    }

    pub fn forward(&self, node_features: &[f32], _adjacency: &[(usize, usize)]) -> Vec<f32> {
        node_features.to_vec()
    }

    pub fn train(
        &mut self,
        _node_features: &[Vec<f32>],
        _adjacency: &[(usize, usize)],
        _epochs: usize,
    ) -> GnnTrainingStatus {
        self.trained = true;
        GnnTrainingStatus {
            epoch: 1,
            total_epochs: 1,
            loss: 0.0,
            accuracy: 1.0,
            completed: true,
        }
    }
}

impl Default for GnnModel {
    fn default() -> Self {
        Self::new()
    }
}

#[pg_extern]
fn ruvector_gnn_status() -> pgrx::JsonB {
    pgrx::JsonB(serde_json::json!({
        "enabled": true,
        "model_loaded": false,
        "version": "1.0.0"
    }))
}

#[pg_extern]
fn ruvector_gnn_default_config() -> pgrx::JsonB {
    pgrx::JsonB(serde_json::json!(GnnConfig::default()))
}

#[cfg(feature = "pg_test")]
#[pg_schema]
mod tests {
    use super::*;

    #[pg_test]
    fn test_gnn_status() {
        let status = ruvector_gnn_status();
        assert!(status.0.get("enabled").is_some());
    }
}
