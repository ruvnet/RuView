//! MCP tools for the coherence gate
//!
//! Provides three main tools:
//! - permit_action: Request permission for an action
//! - get_receipt: Get a witness receipt by sequence number
//! - replay_decision: Deterministically replay a decision for audit

use crate::types::*;
use cognitum_gate_tilezero::{GateDecision, TileZero, WitnessReceipt};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Error type for MCP tool operations
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("Receipt not found: sequence {0}")]
    ReceiptNotFound(u64),
    #[error("Chain verification failed: {0}")]
    ChainVerifyFailed(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl McpError {
    /// Convert to JSON-RPC error code
    pub fn code(&self) -> i32 {
        match self {
            McpError::ReceiptNotFound(_) => -32001,
            McpError::ChainVerifyFailed(_) => -32002,
            McpError::InvalidRequest(_) => -32602,
            McpError::Internal(_) => -32603,
        }
    }
}

/// MCP Gate tools handler
pub struct McpGateTools {
    /// TileZero instance
    tilezero: Arc<RwLock<TileZero>>,
}

impl McpGateTools {
    /// Create a new tools handler
    pub fn new(tilezero: Arc<RwLock<TileZero>>) -> Self {
        Self { tilezero }
    }

    /// Get the list of available tools
    pub fn list_tools() -> Vec<McpTool> {
        vec![
            McpTool {
                name: "permit_action".to_string(),
                description: "Request permission for an action from the coherence gate. Returns a PermitToken for permitted actions, escalation info for deferred actions, or denial details.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "action_id": {
                            "type": "string",
                            "description": "Unique identifier for this action"
                        },
                        "action_type": {
                            "type": "string",
                            "description": "Type of action (e.g., config_change, api_call)"
                        },
                        "target": {
                            "type": "object",
                            "properties": {
                                "device": { "type": "string" },
                                "path": { "type": "string" }
                            }
                        },
                        "context": {
                            "type": "object",
                            "properties": {
                                "agent_id": { "type": "string" },
                                "session_id": { "type": "string" },
                                "prior_actions": {
                                    "type": "array",
                                    "items": { "type": "string" }
                                },
                                "urgency": { "type": "string" }
                            }
                        }
                    },
                    "required": ["action_id", "action_type"]
                }),
            },
            McpTool {
                name: "get_receipt".to_string(),
                description: "Retrieve a witness receipt by sequence number for audit purposes.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sequence": {
                            "type": "integer",
                            "description": "Sequence number of the receipt to retrieve"
                        }
                    },
                    "required": ["sequence"]
                }),
            },
            McpTool {
                name: "replay_decision".to_string(),
                description: "Deterministically replay a past decision for audit and verification.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sequence": {
                            "type": "integer",
                            "description": "Sequence number of the decision to replay"
                        },
                        "verify_chain": {
                            "type": "boolean",
                            "description": "Whether to verify the hash chain up to this decision"
                        }
                    },
                    "required": ["sequence"]
                }),
            },
        ]
    }

    /// Handle a tool call
    pub async fn call_tool(&self, call: McpToolCall) -> Result<McpToolResult, McpError> {
        match call.name.as_str() {
            "permit_action" => {
                let request: PermitActionRequest = serde_json::from_value(call.arguments)
                    .map_err(|e| McpError::InvalidRequest(e.to_string()))?;
                let response = self.permit_action(request).await?;
                Ok(McpToolResult::Success {
                    content: serde_json::to_value(response)
                        .map_err(|e| McpError::Internal(e.to_string()))?,
                })
            }
            "get_receipt" => {
                let request: GetReceiptRequest = serde_json::from_value(call.arguments)
                    .map_err(|e| McpError::InvalidRequest(e.to_string()))?;
                let response = self.get_receipt(request).await?;
                Ok(McpToolResult::Success {
                    content: serde_json::to_value(response)
                        .map_err(|e| McpError::Internal(e.to_string()))?,
                })
            }
            "replay_decision" => {
                let request: ReplayDecisionRequest = serde_json::from_value(call.arguments)
                    .map_err(|e| McpError::InvalidRequest(e.to_string()))?;
                let response = self.replay_decision(request).await?;
                Ok(McpToolResult::Success {
                    content: serde_json::to_value(response)
                        .map_err(|e| McpError::Internal(e.to_string()))?,
                })
            }
            _ => Err(McpError::InvalidRequest(format!(
                "Unknown tool: {}",
                call.name
            ))),
        }
    }

    /// Request permission for an action
    pub async fn permit_action(
        &self,
        request: PermitActionRequest,
    ) -> Result<PermitActionResponse, McpError> {
        let ctx = request.to_action_context();
        let tilezero = self.tilezero.read().await;
        let token = tilezero.decide(&ctx).await;

        // Get the receipt for witness info
        let receipt = tilezero
            .get_receipt(token.sequence)
            .await
            .ok_or_else(|| McpError::Internal("Failed to get receipt".to_string()))?;

        let witness = self.build_witness_info(&receipt);

        match token.decision {
            GateDecision::Permit => Ok(PermitActionResponse::Permit(PermitResponse {
                token: token.encode_base64(),
                valid_until_ns: token.timestamp + token.ttl_ns,
                witness,
                receipt_sequence: token.sequence,
            })),
            GateDecision::Defer => {
                let reason = self.determine_defer_reason(&receipt);
                Ok(PermitActionResponse::Defer(DeferResponse {
                    reason: reason.0,
                    detail: reason.1,
                    escalation: EscalationInfo {
                        to: "human_operator".to_string(),
                        context_url: format!("/receipts/{}/context", token.sequence),
                        timeout_ns: 300_000_000_000, // 5 minutes
                        default_on_timeout: "deny".to_string(),
                    },
                    witness,
                    receipt_sequence: token.sequence,
                }))
            }
            GateDecision::Deny => {
                let reason = self.determine_deny_reason(&receipt);
                Ok(PermitActionResponse::Deny(DenyResponse {
                    reason: reason.0,
                    detail: reason.1,
                    witness,
                    receipt_sequence: token.sequence,
                }))
            }
        }
    }

    /// Get a witness receipt
    pub async fn get_receipt(
        &self,
        request: GetReceiptRequest,
    ) -> Result<GetReceiptResponse, McpError> {
        let tilezero = self.tilezero.read().await;
        let receipt = tilezero
            .get_receipt(request.sequence)
            .await
            .ok_or(McpError::ReceiptNotFound(request.sequence))?;

        Ok(GetReceiptResponse {
            sequence: receipt.sequence,
            decision: receipt.token.decision.to_string(),
            timestamp: receipt.token.timestamp,
            witness_summary: receipt.witness_summary.to_json(),
            previous_hash: hex::encode(receipt.previous_hash),
            receipt_hash: hex::encode(receipt.hash()),
        })
    }

    /// Replay a decision for audit
    pub async fn replay_decision(
        &self,
        request: ReplayDecisionRequest,
    ) -> Result<ReplayDecisionResponse, McpError> {
        let tilezero = self.tilezero.read().await;

        // Optionally verify hash chain
        if request.verify_chain {
            tilezero
                .verify_chain_to(request.sequence)
                .await
                .map_err(|e| McpError::ChainVerifyFailed(e.to_string()))?;
        }

        // Get the original receipt
        let receipt = tilezero
            .get_receipt(request.sequence)
            .await
            .ok_or(McpError::ReceiptNotFound(request.sequence))?;

        // Replay the decision
        let replayed = tilezero.replay(&receipt).await;

        Ok(ReplayDecisionResponse {
            original_decision: receipt.token.decision.to_string(),
            replayed_decision: replayed.decision.to_string(),
            match_confirmed: receipt.token.decision == replayed.decision,
            state_snapshot: replayed.state_snapshot.to_json(),
        })
    }

    /// Build witness info from a receipt
    fn build_witness_info(&self, receipt: &WitnessReceipt) -> WitnessInfo {
        let summary = &receipt.witness_summary;
        WitnessInfo {
            structural: StructuralInfo {
                cut_value: summary.structural.cut_value,
                partition: summary.structural.partition.clone(),
                critical_edges: Some(summary.structural.critical_edges),
                boundary: if summary.structural.boundary.is_empty() {
                    None
                } else {
                    Some(summary.structural.boundary.clone())
                },
            },
            predictive: PredictiveInfo {
                set_size: summary.predictive.set_size,
                coverage: summary.predictive.coverage,
            },
            evidential: EvidentialInfo {
                e_value: summary.evidential.e_value,
                verdict: summary.evidential.verdict.clone(),
            },
        }
    }

    /// Determine the reason for a DEFER decision
    fn determine_defer_reason(&self, receipt: &WitnessReceipt) -> (String, String) {
        let summary = &receipt.witness_summary;

        // Check predictive uncertainty
        if summary.predictive.set_size > 10 {
            return (
                "prediction_uncertainty".to_string(),
                format!(
                    "Prediction set size {} indicates high uncertainty",
                    summary.predictive.set_size
                ),
            );
        }

        // Check evidential indeterminate
        if summary.evidential.verdict == "continue" {
            return (
                "insufficient_evidence".to_string(),
                format!(
                    "E-value {} is in indeterminate range",
                    summary.evidential.e_value
                ),
            );
        }

        // Default
        (
            "shift_detected".to_string(),
            "Distribution shift detected, escalating for human review".to_string(),
        )
    }

    /// Determine the reason for a DENY decision
    fn determine_deny_reason(&self, receipt: &WitnessReceipt) -> (String, String) {
        let summary = &receipt.witness_summary;

        // Check structural violation
        if summary.structural.partition == "fragile" {
            return (
                "boundary_violation".to_string(),
                format!(
                    "Action crosses fragile partition (cut={:.1} is below minimum)",
                    summary.structural.cut_value
                ),
            );
        }

        // Check evidential rejection
        if summary.evidential.verdict == "reject" {
            return (
                "evidence_rejection".to_string(),
                format!(
                    "E-value {:.4} indicates strong evidence of incoherence",
                    summary.evidential.e_value
                ),
            );
        }

        // Default
        (
            "policy_violation".to_string(),
            "Action violates gate policy".to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cognitum_gate_tilezero::GateThresholds;

    #[tokio::test]
    async fn test_permit_action() {
        let tilezero = Arc::new(RwLock::new(TileZero::new(GateThresholds::default())));
        let tools = McpGateTools::new(tilezero);

        let request = PermitActionRequest {
            action_id: "test-action-1".to_string(),
            action_type: "config_change".to_string(),
            target: TargetInfo {
                device: Some("router-1".to_string()),
                path: Some("/config".to_string()),
                extra: Default::default(),
            },
            context: ContextInfo {
                agent_id: "agent-1".to_string(),
                session_id: Some("session-1".to_string()),
                prior_actions: vec![],
                urgency: "normal".to_string(),
            },
        };

        let response = tools.permit_action(request).await.unwrap();
        match response {
            PermitActionResponse::Permit(p) => {
                assert!(!p.token.is_empty());
                assert!(p.receipt_sequence == 0);
            }
            PermitActionResponse::Defer(d) => {
                assert!(!d.reason.is_empty());
            }
            PermitActionResponse::Deny(d) => {
                assert!(!d.reason.is_empty());
            }
        }
    }

    #[tokio::test]
    async fn test_get_receipt() {
        let tilezero = Arc::new(RwLock::new(TileZero::new(GateThresholds::default())));
        let tools = McpGateTools::new(tilezero);

        // First create a decision
        let request = PermitActionRequest {
            action_id: "test-action-1".to_string(),
            action_type: "config_change".to_string(),
            target: Default::default(),
            context: Default::default(),
        };
        let _ = tools.permit_action(request).await.unwrap();

        // Now get the receipt
        let receipt_response = tools
            .get_receipt(GetReceiptRequest { sequence: 0 })
            .await
            .unwrap();

        assert_eq!(receipt_response.sequence, 0);
        assert!(!receipt_response.receipt_hash.is_empty());
    }

    #[tokio::test]
    async fn test_replay_decision() {
        let tilezero = Arc::new(RwLock::new(TileZero::new(GateThresholds::default())));
        let tools = McpGateTools::new(tilezero);

        // First create a decision
        let request = PermitActionRequest {
            action_id: "test-action-1".to_string(),
            action_type: "config_change".to_string(),
            target: Default::default(),
            context: Default::default(),
        };
        let _ = tools.permit_action(request).await.unwrap();

        // Replay the decision
        let replay_response = tools
            .replay_decision(ReplayDecisionRequest {
                sequence: 0,
                verify_chain: true,
            })
            .await
            .unwrap();

        assert!(replay_response.match_confirmed);
    }

    #[test]
    fn test_list_tools() {
        let tools = McpGateTools::list_tools();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "permit_action");
        assert_eq!(tools[1].name, "get_receipt");
        assert_eq!(tools[2].name, "replay_decision");
    }
}
