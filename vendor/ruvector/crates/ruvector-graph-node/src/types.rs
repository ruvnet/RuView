//! Type conversions between Rust and JavaScript

use napi::bindgen_prelude::*;
use napi_derive::napi;
use ruvector_core::DistanceMetric;
use std::collections::HashMap;

/// Distance metric for similarity calculation
#[napi(string_enum)]
#[derive(Debug)]
pub enum JsDistanceMetric {
    Euclidean,
    Cosine,
    DotProduct,
    Manhattan,
}

impl From<JsDistanceMetric> for DistanceMetric {
    fn from(metric: JsDistanceMetric) -> Self {
        match metric {
            JsDistanceMetric::Euclidean => DistanceMetric::Euclidean,
            JsDistanceMetric::Cosine => DistanceMetric::Cosine,
            JsDistanceMetric::DotProduct => DistanceMetric::DotProduct,
            JsDistanceMetric::Manhattan => DistanceMetric::Manhattan,
        }
    }
}

/// Graph database configuration options
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsGraphOptions {
    /// Distance metric for embeddings
    pub distance_metric: Option<JsDistanceMetric>,
    /// Vector dimensions
    pub dimensions: Option<u32>,
    /// Storage path
    pub storage_path: Option<String>,
}

impl Default for JsGraphOptions {
    fn default() -> Self {
        Self {
            distance_metric: Some(JsDistanceMetric::Cosine),
            dimensions: Some(384),
            storage_path: None,
        }
    }
}

/// Node in the graph
#[napi(object)]
#[derive(Clone)]
pub struct JsNode {
    /// Node ID
    pub id: String,
    /// Node embedding
    pub embedding: Float32Array,
    /// Node labels (e.g., ["Person", "Employee"])
    pub labels: Option<Vec<String>>,
    /// Optional properties
    pub properties: Option<HashMap<String, String>>,
}

/// Edge between two nodes
#[napi(object)]
#[derive(Clone)]
pub struct JsEdge {
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Edge description/label
    pub description: String,
    /// Edge embedding
    pub embedding: Float32Array,
    /// Confidence score (0.0-1.0)
    pub confidence: Option<f64>,
    /// Optional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Hyperedge connecting multiple nodes
#[napi(object)]
#[derive(Clone)]
pub struct JsHyperedge {
    /// Node IDs connected by this hyperedge
    pub nodes: Vec<String>,
    /// Natural language description of the relationship
    pub description: String,
    /// Embedding of the hyperedge description
    pub embedding: Float32Array,
    /// Confidence weight (0.0-1.0)
    pub confidence: Option<f64>,
    /// Optional metadata
    pub metadata: Option<HashMap<String, String>>,
}

/// Query for searching hyperedges
#[napi(object)]
#[derive(Clone)]
pub struct JsHyperedgeQuery {
    /// Query embedding
    pub embedding: Float32Array,
    /// Number of results to return
    pub k: u32,
}

/// Hyperedge search result
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsHyperedgeResult {
    /// Hyperedge ID
    pub id: String,
    /// Similarity score
    pub score: f64,
}

/// Node result from query (without embedding)
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsNodeResult {
    /// Node ID
    pub id: String,
    /// Node labels
    pub labels: Vec<String>,
    /// Node properties
    pub properties: HashMap<String, String>,
}

/// Edge result from query
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsEdgeResult {
    /// Edge ID
    pub id: String,
    /// Source node ID
    pub from: String,
    /// Target node ID
    pub to: String,
    /// Edge type/label
    pub edge_type: String,
    /// Edge properties
    pub properties: HashMap<String, String>,
}

/// Query result
#[napi(object)]
#[derive(Clone)]
pub struct JsQueryResult {
    /// Nodes returned by the query
    pub nodes: Vec<JsNodeResult>,
    /// Edges returned by the query
    pub edges: Vec<JsEdgeResult>,
    /// Optional statistics
    pub stats: Option<JsGraphStats>,
}

/// Graph statistics
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsGraphStats {
    /// Total number of nodes
    pub total_nodes: u32,
    /// Total number of edges
    pub total_edges: u32,
    /// Average node degree
    pub avg_degree: f64,
}

/// Batch insert data
#[napi(object)]
#[derive(Clone)]
pub struct JsBatchInsert {
    /// Nodes to insert
    pub nodes: Vec<JsNode>,
    /// Edges to insert
    pub edges: Vec<JsEdge>,
}

/// Batch insert result
#[napi(object)]
#[derive(Debug, Clone)]
pub struct JsBatchResult {
    /// IDs of inserted nodes
    pub node_ids: Vec<String>,
    /// IDs of inserted edges
    pub edge_ids: Vec<String>,
}

/// Temporal granularity
#[napi(string_enum)]
#[derive(Debug)]
pub enum JsTemporalGranularity {
    Hourly,
    Daily,
    Monthly,
    Yearly,
}

/// Temporal hyperedge
#[napi(object)]
#[derive(Clone)]
pub struct JsTemporalHyperedge {
    /// Base hyperedge
    pub hyperedge: JsHyperedge,
    /// Creation timestamp (Unix epoch seconds)
    pub timestamp: i64,
    /// Optional expiration timestamp
    pub expires_at: Option<i64>,
    /// Temporal context
    pub granularity: JsTemporalGranularity,
}
