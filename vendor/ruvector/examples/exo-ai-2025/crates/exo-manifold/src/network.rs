//! Simplified network module (burn removed)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedManifold {
    dimension: usize,
    hidden_dim: usize,
    hidden_layers: usize,
}

impl LearnedManifold {
    pub fn new(dimension: usize, hidden_dim: usize, hidden_layers: usize) -> Self {
        Self {
            dimension,
            hidden_dim,
            hidden_layers,
        }
    }

    pub fn dimension(&self) -> usize {
        self.dimension
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SirenLayer;
