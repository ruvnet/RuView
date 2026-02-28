//! Core types for the vector database

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Distance metric for vector similarity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DistanceMetric {
    /// Euclidean distance (L2)
    Euclidean,
    /// Cosine similarity
    Cosine,
    /// Dot product
    DotProduct,
    /// Manhattan distance (L1)
    Manhattan,
}

/// Vector entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    /// Unique identifier
    pub id: String,
    /// Vector data
    pub vector: Vec<f32>,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp
    pub timestamp: i64,
}

/// Search query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Query vector
    pub vector: Vec<f32>,
    /// Number of results to return
    pub k: usize,
    /// Metadata filters (optional)
    pub filters: Option<HashMap<String, serde_json::Value>>,
    /// Distance threshold (optional)
    pub threshold: Option<f32>,
    /// Search parameter (efSearch for HNSW)
    pub ef_search: Option<usize>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Vector ID
    pub id: String,
    /// Distance/similarity score
    pub score: f32,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Vector data (optional, only if requested)
    pub vector: Option<Vec<f32>>,
}

/// Configuration for vector database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDbConfig {
    /// Vector dimensions
    pub dimensions: usize,
    /// Maximum number of elements
    pub max_elements: usize,
    /// Distance metric
    pub distance_metric: DistanceMetric,
    /// HNSW M parameter (connections per node)
    pub hnsw_m: usize,
    /// HNSW ef_construction parameter
    pub hnsw_ef_construction: usize,
    /// Default ef_search parameter
    pub hnsw_ef_search: usize,
    /// Quantization type
    pub quantization: QuantizationType,
    /// Storage path
    pub storage_path: String,
    /// Enable memory mapping
    pub mmap_vectors: bool,
}

impl Default for VectorDbConfig {
    fn default() -> Self {
        Self {
            dimensions: 384,
            max_elements: 1_000_000,
            distance_metric: DistanceMetric::Cosine,
            hnsw_m: 32,
            hnsw_ef_construction: 200,
            hnsw_ef_search: 100,
            quantization: QuantizationType::None,
            storage_path: "./vectors.db".to_string(),
            mmap_vectors: true,
        }
    }
}

/// Quantization type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuantizationType {
    /// No quantization
    None,
    /// Scalar quantization (int8)
    Scalar,
    /// Product quantization
    Product {
        /// Number of subspaces
        subspaces: usize,
        /// Number of centroids per subspace
        k: usize,
    },
    /// Binary quantization
    Binary,
}

/// Statistics about the vector database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorDbStats {
    /// Total number of vectors
    pub total_vectors: usize,
    /// Index size in bytes
    pub index_size_bytes: usize,
    /// Storage size in bytes
    pub storage_size_bytes: usize,
    /// Average query latency in microseconds
    pub avg_query_latency_us: f64,
    /// Queries per second
    pub qps: f64,
}
