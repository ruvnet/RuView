//! cognitum-gate-tilezero: TileZero arbiter for the Anytime-Valid Coherence Gate
//!
//! TileZero acts as the central arbiter in the 256-tile WASM fabric, responsible for:
//! - Merging worker tile reports into a supergraph
//! - Making global gate decisions (Permit/Defer/Deny)
//! - Issuing cryptographically signed permit tokens
//! - Maintaining a hash-chained witness receipt log

pub mod decision;
pub mod evidence;
pub mod merge;
pub mod permit;
pub mod receipt;
pub mod supergraph;

pub use decision::{
    DecisionFilter, DecisionOutcome, EvidenceDecision, GateDecision, GateThresholds,
    ThreeFilterDecision,
};
pub use evidence::{AggregatedEvidence, EvidenceFilter};
pub use merge::{MergeStrategy, MergedReport, ReportMerger, WorkerReport};
pub use permit::{PermitState, PermitToken, TokenDecodeError, Verifier, VerifyError};
pub use receipt::{ReceiptLog, TimestampProof, WitnessReceipt, WitnessSummary};
pub use supergraph::{ReducedGraph, ShiftPressure, StructuralFilter};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::RwLock;

/// Action identifier
pub type ActionId = String;

/// Vertex identifier in the coherence graph
pub type VertexId = u64;

/// Edge identifier in the coherence graph
pub type EdgeId = u64;

/// Worker tile identifier (1-255, with 0 reserved for TileZero)
pub type TileId = u8;

/// Context for an action being evaluated by the gate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionContext {
    /// Unique identifier for this action
    pub action_id: ActionId,
    /// Type of action (e.g., "config_change", "api_call")
    pub action_type: String,
    /// Target of the action
    pub target: ActionTarget,
    /// Additional context
    pub context: ActionMetadata,
}

/// Target of an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionTarget {
    /// Target device/resource
    pub device: Option<String>,
    /// Target path
    pub path: Option<String>,
    /// Additional target properties
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Metadata about the action context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionMetadata {
    /// Agent requesting the action
    pub agent_id: String,
    /// Session identifier
    pub session_id: Option<String>,
    /// Prior related actions
    #[serde(default)]
    pub prior_actions: Vec<ActionId>,
    /// Urgency level
    #[serde(default = "default_urgency")]
    pub urgency: String,
}

fn default_urgency() -> String {
    "normal".to_string()
}

/// Report from a worker tile
#[repr(C, align(64))]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileReport {
    /// Tile identifier (1-255)
    pub tile_id: TileId,
    /// Local coherence score
    pub coherence: f32,
    /// Whether boundary has moved since last report
    pub boundary_moved: bool,
    /// Top suspicious edges
    pub suspicious_edges: Vec<EdgeId>,
    /// Local e-value accumulator
    pub e_value: f32,
    /// Witness fragment for boundary changes
    pub witness_fragment: Option<WitnessFragment>,
}

/// Fragment of witness data from a worker tile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessFragment {
    /// Tile that generated this fragment
    pub tile_id: TileId,
    /// Boundary edges in this shard
    pub boundary_edges: Vec<EdgeId>,
    /// Local cut value
    pub cut_value: f32,
}

/// Escalation information for DEFER decisions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationInfo {
    /// Who to escalate to
    pub to: String,
    /// URL for context
    pub context_url: String,
    /// Timeout in nanoseconds
    pub timeout_ns: u64,
    /// Default action on timeout
    #[serde(default = "default_timeout_action")]
    pub default_on_timeout: String,
}

fn default_timeout_action() -> String {
    "deny".to_string()
}

/// TileZero: The central arbiter of the coherence gate
pub struct TileZero {
    /// Reduced supergraph from worker summaries
    supergraph: RwLock<ReducedGraph>,
    /// Canonical permit token state
    permit_state: PermitState,
    /// Hash-chained witness receipt log
    receipt_log: RwLock<ReceiptLog>,
    /// Threshold configuration
    thresholds: GateThresholds,
    /// Sequence counter
    sequence: AtomicU64,
}

impl TileZero {
    /// Create a new TileZero arbiter
    pub fn new(thresholds: GateThresholds) -> Self {
        Self {
            supergraph: RwLock::new(ReducedGraph::new()),
            permit_state: PermitState::new(),
            receipt_log: RwLock::new(ReceiptLog::new()),
            thresholds,
            sequence: AtomicU64::new(0),
        }
    }

    /// Collect reports from all worker tiles
    pub async fn collect_reports(&self, reports: &[TileReport]) {
        let mut graph = self.supergraph.write().await;
        for report in reports {
            if report.boundary_moved {
                if let Some(ref fragment) = report.witness_fragment {
                    graph.update_from_fragment(fragment);
                }
            }
            graph.update_coherence(report.tile_id, report.coherence);
        }
    }

    /// Make a gate decision for an action
    pub async fn decide(&self, action_ctx: &ActionContext) -> PermitToken {
        let seq = self.sequence.fetch_add(1, Ordering::SeqCst);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let graph = self.supergraph.read().await;

        // Three stacked filters:
        // 1. Structural filter (global cut on reduced graph)
        let structural_ok = graph.global_cut() >= self.thresholds.min_cut;

        // 2. Shift filter (aggregated shift pressure)
        let shift_pressure = graph.aggregate_shift_pressure();
        let shift_ok = shift_pressure < self.thresholds.max_shift;

        // 3. Evidence filter
        let e_aggregate = graph.aggregate_evidence();
        let evidence_decision = self.evidence_decision(e_aggregate);

        // Combined decision
        let decision = match (structural_ok, shift_ok, evidence_decision) {
            (false, _, _) => GateDecision::Deny,
            (_, false, _) => GateDecision::Defer,
            (_, _, EvidenceDecision::Reject) => GateDecision::Deny,
            (_, _, EvidenceDecision::Continue) => GateDecision::Defer,
            (true, true, EvidenceDecision::Accept) => GateDecision::Permit,
        };

        // Compute witness hash
        let witness_summary = graph.witness_summary();
        let witness_hash = witness_summary.hash();

        drop(graph);

        // Create token
        let token = PermitToken {
            decision,
            action_id: action_ctx.action_id.clone(),
            timestamp: now,
            ttl_ns: self.thresholds.permit_ttl_ns,
            witness_hash,
            sequence: seq,
            signature: [0u8; 64], // Will be filled by sign
        };

        // Sign the token
        let signed_token = self.permit_state.sign_token(token);

        // Emit receipt
        self.emit_receipt(&signed_token, &witness_summary).await;

        signed_token
    }

    /// Get evidence decision based on accumulated e-value
    fn evidence_decision(&self, e_aggregate: f64) -> EvidenceDecision {
        if e_aggregate < self.thresholds.tau_deny {
            EvidenceDecision::Reject
        } else if e_aggregate >= self.thresholds.tau_permit {
            EvidenceDecision::Accept
        } else {
            EvidenceDecision::Continue
        }
    }

    /// Emit a witness receipt
    async fn emit_receipt(&self, token: &PermitToken, summary: &WitnessSummary) {
        let mut log = self.receipt_log.write().await;
        let previous_hash = log.last_hash();

        let receipt = WitnessReceipt {
            sequence: token.sequence,
            token: token.clone(),
            previous_hash,
            witness_summary: summary.clone(),
            timestamp_proof: TimestampProof {
                timestamp: token.timestamp,
                previous_receipt_hash: previous_hash,
                merkle_root: [0u8; 32], // Simplified for v0
            },
        };

        log.append(receipt);
    }

    /// Get a receipt by sequence number
    pub async fn get_receipt(&self, sequence: u64) -> Option<WitnessReceipt> {
        let log = self.receipt_log.read().await;
        log.get(sequence).cloned()
    }

    /// Verify the hash chain up to a sequence number
    pub async fn verify_chain_to(&self, sequence: u64) -> Result<(), ChainVerifyError> {
        let log = self.receipt_log.read().await;
        log.verify_chain_to(sequence)
    }

    /// Replay a decision for audit purposes
    pub async fn replay(&self, receipt: &WitnessReceipt) -> ReplayResult {
        // In a full implementation, this would reconstruct state from checkpoints
        // For now, return the original decision
        ReplayResult {
            decision: receipt.token.decision,
            state_snapshot: receipt.witness_summary.clone(),
        }
    }

    /// Get the verifier for token validation
    pub fn verifier(&self) -> Verifier {
        self.permit_state.verifier()
    }

    /// Get the thresholds configuration
    pub fn thresholds(&self) -> &GateThresholds {
        &self.thresholds
    }

    /// Verify the entire receipt chain
    pub async fn verify_receipt_chain(&self) -> Result<(), ChainVerifyError> {
        let log = self.receipt_log.read().await;
        let len = log.len();
        if len == 0 {
            return Ok(());
        }
        log.verify_chain_to(len as u64 - 1)
    }

    /// Export all receipts as JSON
    pub async fn export_receipts_json(&self) -> Result<String, serde_json::Error> {
        let log = self.receipt_log.read().await;
        let receipts: Vec<&WitnessReceipt> = log.iter().collect();
        serde_json::to_string_pretty(&receipts)
    }
}

/// Result of replaying a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// The replayed decision
    pub decision: GateDecision,
    /// State snapshot at decision time
    pub state_snapshot: WitnessSummary,
}

/// Error during chain verification
#[derive(Debug, thiserror::Error)]
pub enum ChainVerifyError {
    #[error("Receipt {sequence} not found")]
    ReceiptNotFound { sequence: u64 },
    #[error("Hash mismatch at sequence {sequence}")]
    HashMismatch { sequence: u64 },
    #[error("Signature verification failed at sequence {sequence}")]
    SignatureInvalid { sequence: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tilezero_basic_permit() {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);

        let ctx = ActionContext {
            action_id: "test-action-1".to_string(),
            action_type: "config_change".to_string(),
            target: ActionTarget {
                device: Some("router-1".to_string()),
                path: Some("/config".to_string()),
                extra: HashMap::new(),
            },
            context: ActionMetadata {
                agent_id: "agent-1".to_string(),
                session_id: Some("session-1".to_string()),
                prior_actions: vec![],
                urgency: "normal".to_string(),
            },
        };

        let token = tilezero.decide(&ctx).await;
        assert_eq!(token.sequence, 0);
        assert!(!token.action_id.is_empty());
    }

    #[tokio::test]
    async fn test_receipt_chain() {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);

        let ctx = ActionContext {
            action_id: "test-action-1".to_string(),
            action_type: "config_change".to_string(),
            target: ActionTarget {
                device: None,
                path: None,
                extra: HashMap::new(),
            },
            context: ActionMetadata {
                agent_id: "agent-1".to_string(),
                session_id: None,
                prior_actions: vec![],
                urgency: "normal".to_string(),
            },
        };

        // Generate multiple decisions
        let _token1 = tilezero.decide(&ctx).await;
        let _token2 = tilezero.decide(&ctx).await;

        // Verify receipts exist
        let receipt0 = tilezero.get_receipt(0).await;
        assert!(receipt0.is_some());

        let receipt1 = tilezero.get_receipt(1).await;
        assert!(receipt1.is_some());
    }
}
