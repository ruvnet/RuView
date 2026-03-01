//! Core types for Tiny Dancer routing system

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A candidate for routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Candidate ID
    pub id: String,
    /// Embedding vector (384-768 dimensions)
    pub embedding: Vec<f32>,
    /// Metadata associated with the candidate
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timestamp of creation (Unix timestamp)
    pub created_at: i64,
    /// Access count
    pub access_count: u64,
    /// Historical success rate (0.0 to 1.0)
    pub success_rate: f32,
}

/// Request for routing decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRequest {
    /// Query embedding
    pub query_embedding: Vec<f32>,
    /// List of candidates to score
    pub candidates: Vec<Candidate>,
    /// Optional metadata for context
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Routing decision result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingDecision {
    /// Selected candidate ID
    pub candidate_id: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
    /// Whether to route to lightweight or powerful model
    pub use_lightweight: bool,
    /// Uncertainty estimate
    pub uncertainty: f32,
}

/// Complete routing response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingResponse {
    /// Routing decisions (top-k)
    pub decisions: Vec<RoutingDecision>,
    /// Total inference time in microseconds
    pub inference_time_us: u64,
    /// Number of candidates processed
    pub candidates_processed: usize,
    /// Feature engineering time in microseconds
    pub feature_time_us: u64,
}

/// Model type for routing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelType {
    /// Lightweight model (fast, lower quality)
    Lightweight,
    /// Powerful model (slower, higher quality)
    Powerful,
}

/// Routing metrics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingMetrics {
    /// Total requests processed
    pub total_requests: u64,
    /// Requests routed to lightweight model
    pub lightweight_routes: u64,
    /// Requests routed to powerful model
    pub powerful_routes: u64,
    /// Average inference time (microseconds)
    pub avg_inference_time_us: f64,
    /// P50 latency (microseconds)
    pub p50_latency_us: u64,
    /// P95 latency (microseconds)
    pub p95_latency_us: u64,
    /// P99 latency (microseconds)
    pub p99_latency_us: u64,
    /// Error count
    pub error_count: u64,
    /// Circuit breaker trips
    pub circuit_breaker_trips: u64,
}

impl Default for RoutingMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            lightweight_routes: 0,
            powerful_routes: 0,
            avg_inference_time_us: 0.0,
            p50_latency_us: 0,
            p95_latency_us: 0,
            p99_latency_us: 0,
            error_count: 0,
            circuit_breaker_trips: 0,
        }
    }
}

/// Configuration for the router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Model path or identifier
    pub model_path: String,
    /// Confidence threshold for lightweight routing (0.0 to 1.0)
    pub confidence_threshold: f32,
    /// Maximum uncertainty allowed (0.0 to 1.0)
    pub max_uncertainty: f32,
    /// Enable circuit breaker
    pub enable_circuit_breaker: bool,
    /// Circuit breaker error threshold
    pub circuit_breaker_threshold: u32,
    /// Enable quantization
    pub enable_quantization: bool,
    /// Database path for AgentDB
    pub database_path: Option<String>,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            model_path: "./models/fastgrnn.safetensors".to_string(),
            confidence_threshold: 0.85,
            max_uncertainty: 0.15,
            enable_circuit_breaker: true,
            circuit_breaker_threshold: 5,
            enable_quantization: true,
            database_path: None,
        }
    }
}
