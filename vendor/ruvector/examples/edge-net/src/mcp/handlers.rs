//! Additional MCP Handlers
//!
//! Extended handler implementations for specialized edge-net capabilities.

use super::protocol::*;
use serde_json::{json, Value};

/// Vector search handler parameters
pub struct VectorSearchParams {
    pub query: Vec<f32>,
    pub k: usize,
    pub filter: Option<Value>,
}

/// Embedding generation parameters
pub struct EmbeddingParams {
    pub text: String,
    pub model: Option<String>,
}

/// Semantic match parameters
pub struct SemanticMatchParams {
    pub text: String,
    pub categories: Vec<String>,
}

/// Task execution result
#[derive(Debug, Clone)]
pub struct TaskResult {
    pub task_id: String,
    pub status: TaskStatus,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub cost: u64,
}

/// Task status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Queued => "queued",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// Handler for vector operations
pub struct VectorHandler;

impl VectorHandler {
    /// Create vector search response
    pub fn search_response(id: Option<Value>, results: Vec<(String, f32)>) -> McpResponse {
        let result_list: Vec<Value> = results
            .into_iter()
            .map(|(id, score)| json!({ "id": id, "score": score }))
            .collect();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Found {} results", result_list.len())
            }],
            "results": result_list
        }))
    }

    /// Create embedding response
    pub fn embedding_response(id: Option<Value>, embedding: Vec<f32>) -> McpResponse {
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Generated {}-dimensional embedding", embedding.len())
            }],
            "embedding": embedding,
            "dimensions": embedding.len()
        }))
    }
}

/// Handler for RAC coherence operations
pub struct CoherenceHandler;

impl CoherenceHandler {
    /// Create conflict detection response
    pub fn conflict_response(
        id: Option<Value>,
        conflicts: Vec<(String, String, f32)>,
    ) -> McpResponse {
        let conflict_list: Vec<Value> = conflicts
            .into_iter()
            .map(|(id1, id2, severity)| {
                json!({
                    "claim1": id1,
                    "claim2": id2,
                    "severity": severity
                })
            })
            .collect();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Detected {} conflicts", conflict_list.len())
            }],
            "conflicts": conflict_list
        }))
    }

    /// Create resolution response
    pub fn resolution_response(
        id: Option<Value>,
        resolution_id: &str,
        accepted: Vec<String>,
        deprecated: Vec<String>,
    ) -> McpResponse {
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Resolution {}: accepted {}, deprecated {}",
                    resolution_id,
                    accepted.len(),
                    deprecated.len()
                )
            }],
            "resolutionId": resolution_id,
            "accepted": accepted,
            "deprecated": deprecated
        }))
    }
}

/// Handler for economic operations
pub struct EconomicsHandler;

impl EconomicsHandler {
    /// Create stake response
    pub fn stake_response(
        id: Option<Value>,
        staked: u64,
        locked_until: u64,
        multiplier: f32,
    ) -> McpResponse {
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Staked {} rUv ({}x multiplier, locked until {})",
                    staked, multiplier, locked_until
                )
            }],
            "staked": staked,
            "lockedUntil": locked_until,
            "multiplier": multiplier
        }))
    }

    /// Create reward distribution response
    pub fn reward_response(
        id: Option<Value>,
        recipients: Vec<(String, u64)>,
        total: u64,
    ) -> McpResponse {
        let recipient_list: Vec<Value> = recipients
            .into_iter()
            .map(|(node, amount)| json!({ "nodeId": node, "amount": amount }))
            .collect();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Distributed {} rUv to {} recipients", total, recipient_list.len())
            }],
            "recipients": recipient_list,
            "totalDistributed": total
        }))
    }
}

/// Handler for network operations
pub struct NetworkHandler;

impl NetworkHandler {
    /// Create peer list response
    pub fn peers_response(id: Option<Value>, peers: Vec<PeerInfo>) -> McpResponse {
        let peer_list: Vec<Value> = peers
            .into_iter()
            .map(|p| {
                json!({
                    "nodeId": p.node_id,
                    "publicKey": p.public_key,
                    "reputation": p.reputation,
                    "latency": p.latency_ms,
                    "connected": p.connected
                })
            })
            .collect();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("{} peers connected", peer_list.len())
            }],
            "peers": peer_list,
            "count": peer_list.len()
        }))
    }

    /// Create network health response
    pub fn health_response(id: Option<Value>, health: NetworkHealth) -> McpResponse {
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Network Health: {}% (peers: {}, avg latency: {}ms)",
                    (health.score * 100.0) as u32,
                    health.peer_count,
                    health.avg_latency_ms
                )
            }],
            "score": health.score,
            "peerCount": health.peer_count,
            "avgLatency": health.avg_latency_ms,
            "messageRate": health.message_rate,
            "bandwidth": health.bandwidth_kbps
        }))
    }
}

/// Peer information
pub struct PeerInfo {
    pub node_id: String,
    pub public_key: String,
    pub reputation: f32,
    pub latency_ms: u32,
    pub connected: bool,
}

/// Network health metrics
pub struct NetworkHealth {
    pub score: f32,
    pub peer_count: usize,
    pub avg_latency_ms: u32,
    pub message_rate: f32,
    pub bandwidth_kbps: u32,
}

/// Helper for creating error responses
pub fn error_response(id: Option<Value>, code: i32, message: &str) -> McpResponse {
    McpResponse::error(id, McpError::new(code, message))
}

/// Helper for creating not implemented responses
pub fn not_implemented(id: Option<Value>, feature: &str) -> McpResponse {
    McpResponse::success(id, json!({
        "content": [{
            "type": "text",
            "text": format!("{} is not yet implemented", feature)
        }],
        "status": "not_implemented",
        "feature": feature
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_search_response() {
        let results = vec![
            ("doc1".to_string(), 0.95),
            ("doc2".to_string(), 0.87),
        ];
        let response = VectorHandler::search_response(Some(json!(1)), results);
        assert!(response.result.is_some());
    }

    #[test]
    fn test_task_status() {
        assert_eq!(TaskStatus::Completed.as_str(), "completed");
        assert_eq!(TaskStatus::Running.as_str(), "running");
    }

    #[test]
    fn test_network_health_response() {
        let health = NetworkHealth {
            score: 0.85,
            peer_count: 10,
            avg_latency_ms: 50,
            message_rate: 100.0,
            bandwidth_kbps: 1000,
        };
        let response = NetworkHandler::health_response(Some(json!(1)), health);
        assert!(response.result.is_some());
    }
}
