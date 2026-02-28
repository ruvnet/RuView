//! Core type definitions for the cognitive substrate

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Pattern representation in substrate
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pattern {
    /// Vector embedding
    pub embedding: Vec<f32>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Temporal origin (Unix timestamp in microseconds)
    pub timestamp: u64,
    /// Causal antecedents (pattern IDs)
    pub antecedents: Vec<String>,
}

impl Pattern {
    /// Create a new pattern
    pub fn new(embedding: Vec<f32>) -> Self {
        Self {
            embedding,
            metadata: HashMap::new(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            antecedents: Vec::new(),
        }
    }

    /// Create a pattern with metadata
    pub fn with_metadata(
        embedding: Vec<f32>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            embedding,
            metadata,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            antecedents: Vec::new(),
        }
    }

    /// Add causal antecedent
    pub fn with_antecedent(mut self, antecedent_id: String) -> Self {
        self.antecedents.push(antecedent_id);
        self
    }
}

/// Search result from substrate query
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    /// Pattern ID
    pub id: String,
    /// Similarity score (lower is better for distance metrics)
    pub score: f32,
    /// Retrieved pattern
    pub pattern: Option<Pattern>,
}

/// Query specification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Query {
    /// Query embedding
    pub embedding: Vec<f32>,
    /// Number of results to return
    pub k: usize,
    /// Optional metadata filter
    pub filter: Option<HashMap<String, serde_json::Value>>,
}

impl Query {
    /// Create a query from embedding
    pub fn from_embedding(embedding: Vec<f32>, k: usize) -> Self {
        Self {
            embedding,
            k,
            filter: None,
        }
    }

    /// Add metadata filter
    pub fn with_filter(mut self, filter: HashMap<String, serde_json::Value>) -> Self {
        self.filter = Some(filter);
        self
    }
}

/// Topological query specification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TopologicalQuery {
    /// Find persistent homology features
    PersistentHomology {
        dimension: usize,
        epsilon_range: (f32, f32),
    },
    /// Find N-dimensional holes in structure
    BettiNumbers { max_dimension: usize },
    /// Sheaf consistency check
    SheafConsistency { local_sections: Vec<String> },
}

/// Result from hypergraph query
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum HypergraphResult {
    /// Persistence diagram
    PersistenceDiagram { birth_death_pairs: Vec<(f32, f32)> },
    /// Betti numbers by dimension
    BettiNumbers { numbers: Vec<usize> },
    /// Sheaf consistency result
    SheafConsistency {
        is_consistent: bool,
        violations: Vec<String>,
    },
    /// Not supported on current backend
    NotSupported,
}

/// Substrate configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubstrateConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Storage path
    pub storage_path: String,
    /// Enable hypergraph features
    pub enable_hypergraph: bool,
    /// Enable temporal memory
    pub enable_temporal: bool,
}

impl Default for SubstrateConfig {
    fn default() -> Self {
        Self {
            dimensions: 384,
            storage_path: "./substrate.db".to_string(),
            enable_hypergraph: false,
            enable_temporal: false,
        }
    }
}
