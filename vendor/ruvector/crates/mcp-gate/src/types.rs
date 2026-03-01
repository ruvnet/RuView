//! Request/response types for the MCP Gate server
//!
//! These types match the API contract defined in ADR-001.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Re-export types from cognitum-gate-tilezero
pub use cognitum_gate_tilezero::{
    ActionContext, ActionMetadata, ActionTarget, EscalationInfo, GateDecision, GateThresholds,
    PermitToken, WitnessReceipt,
};

/// Request to permit an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitActionRequest {
    /// Unique identifier for this action
    pub action_id: String,
    /// Type of action (e.g., "config_change", "api_call")
    pub action_type: String,
    /// Target of the action
    #[serde(default)]
    pub target: TargetInfo,
    /// Additional context
    #[serde(default)]
    pub context: ContextInfo,
}

/// Target information for an action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetInfo {
    /// Target device/resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Target path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Additional target properties
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Context information for an action
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextInfo {
    /// Agent requesting the action
    #[serde(default)]
    pub agent_id: String,
    /// Session identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// Prior related actions
    #[serde(default)]
    pub prior_actions: Vec<String>,
    /// Urgency level
    #[serde(default = "default_urgency")]
    pub urgency: String,
}

fn default_urgency() -> String {
    "normal".to_string()
}

impl PermitActionRequest {
    /// Convert to ActionContext for the gate
    pub fn to_action_context(&self) -> ActionContext {
        ActionContext {
            action_id: self.action_id.clone(),
            action_type: self.action_type.clone(),
            target: ActionTarget {
                device: self.target.device.clone(),
                path: self.target.path.clone(),
                extra: self.target.extra.clone(),
            },
            context: ActionMetadata {
                agent_id: self.context.agent_id.clone(),
                session_id: self.context.session_id.clone(),
                prior_actions: self.context.prior_actions.clone(),
                urgency: self.context.urgency.clone(),
            },
        }
    }
}

/// Response to a permit action request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "lowercase")]
pub enum PermitActionResponse {
    /// Action is permitted
    Permit(PermitResponse),
    /// Action is deferred for escalation
    Defer(DeferResponse),
    /// Action is denied
    Deny(DenyResponse),
}

/// Permit response details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermitResponse {
    /// Base64-encoded permit token
    pub token: String,
    /// Token valid until (nanoseconds since epoch)
    pub valid_until_ns: u64,
    /// Witness summary
    pub witness: WitnessInfo,
    /// Receipt sequence number
    pub receipt_sequence: u64,
}

/// Defer response details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeferResponse {
    /// Reason for deferral
    pub reason: String,
    /// Detailed explanation
    pub detail: String,
    /// Escalation information
    pub escalation: EscalationInfo,
    /// Witness summary
    pub witness: WitnessInfo,
    /// Receipt sequence number
    pub receipt_sequence: u64,
}

/// Deny response details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenyResponse {
    /// Reason for denial
    pub reason: String,
    /// Detailed explanation
    pub detail: String,
    /// Witness summary
    pub witness: WitnessInfo,
    /// Receipt sequence number
    pub receipt_sequence: u64,
}

/// Witness information in responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessInfo {
    /// Structural witness
    pub structural: StructuralInfo,
    /// Predictive witness
    pub predictive: PredictiveInfo,
    /// Evidential witness
    pub evidential: EvidentialInfo,
}

/// Structural witness details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralInfo {
    /// Cut value
    pub cut_value: f64,
    /// Partition status
    pub partition: String,
    /// Number of critical edges
    #[serde(skip_serializing_if = "Option::is_none")]
    pub critical_edges: Option<usize>,
    /// Boundary edge IDs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boundary: Option<Vec<String>>,
}

/// Predictive witness details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveInfo {
    /// Prediction set size
    pub set_size: usize,
    /// Coverage target
    pub coverage: f64,
}

/// Evidential witness details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidentialInfo {
    /// Accumulated e-value
    pub e_value: f64,
    /// Verdict (accept/continue/reject)
    pub verdict: String,
}

/// Request to get a receipt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReceiptRequest {
    /// Sequence number of the receipt
    pub sequence: u64,
}

/// Response with receipt details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetReceiptResponse {
    /// Sequence number
    pub sequence: u64,
    /// Decision that was made
    pub decision: String,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// Witness summary as JSON
    pub witness_summary: serde_json::Value,
    /// Hash of previous receipt
    pub previous_hash: String,
    /// Hash of this receipt
    pub receipt_hash: String,
}

/// Request to replay a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDecisionRequest {
    /// Sequence number of the decision to replay
    pub sequence: u64,
    /// Whether to verify the hash chain
    #[serde(default)]
    pub verify_chain: bool,
}

/// Response from replaying a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDecisionResponse {
    /// Original decision
    pub original_decision: String,
    /// Replayed decision
    pub replayed_decision: String,
    /// Whether the decisions match
    pub match_confirmed: bool,
    /// State snapshot as JSON
    pub state_snapshot: serde_json::Value,
}

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema
    pub input_schema: serde_json::Value,
}

/// MCP Tool call request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolCall {
    /// Tool name
    pub name: String,
    /// Tool arguments
    pub arguments: serde_json::Value,
}

/// MCP Tool result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpToolResult {
    /// Successful result
    Success { content: serde_json::Value },
    /// Error result
    Error { error: String },
}

/// MCP JSON-RPC request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: serde_json::Value,
    /// Method name
    pub method: String,
    /// Parameters
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version
    pub jsonrpc: String,
    /// Request ID
    pub id: serde_json::Value,
    /// Result (if success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error (if failure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permit_request_deserialize() {
        let json = r#"{
            "action_id": "cfg-push-7a3f",
            "action_type": "config_change",
            "target": {
                "device": "router-west-03",
                "path": "/network/interfaces/eth0"
            },
            "context": {
                "agent_id": "ops-agent-12",
                "session_id": "sess-abc123",
                "prior_actions": ["cfg-push-7a3e"],
                "urgency": "normal"
            }
        }"#;

        let req: PermitActionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.action_id, "cfg-push-7a3f");
        assert_eq!(req.target.device, Some("router-west-03".to_string()));
    }

    #[test]
    fn test_permit_response_serialize() {
        let resp = PermitActionResponse::Permit(PermitResponse {
            token: "eyJ0eXAi...".to_string(),
            valid_until_ns: 1737158400000000000,
            witness: WitnessInfo {
                structural: StructuralInfo {
                    cut_value: 12.7,
                    partition: "stable".to_string(),
                    critical_edges: Some(0),
                    boundary: None,
                },
                predictive: PredictiveInfo {
                    set_size: 3,
                    coverage: 0.92,
                },
                evidential: EvidentialInfo {
                    e_value: 847.3,
                    verdict: "accept".to_string(),
                },
            },
            receipt_sequence: 1847392,
        });

        let json = serde_json::to_string_pretty(&resp).unwrap();
        assert!(json.contains("permit"));
        assert!(json.contains("1847392"));
    }

    #[test]
    fn test_jsonrpc_response() {
        let resp =
            JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"status": "ok"}));
        assert_eq!(resp.jsonrpc, "2.0");
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
    }
}
