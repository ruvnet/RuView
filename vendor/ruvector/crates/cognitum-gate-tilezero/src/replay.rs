//! Deterministic replay for auditing and debugging
//!
//! This module provides the ability to replay gate decisions for audit purposes,
//! ensuring that the same inputs produce the same outputs deterministically.

use crate::{GateDecision, WitnessReceipt, WitnessSummary};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Result of replaying a decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayResult {
    /// The replayed decision
    pub decision: GateDecision,
    /// Whether the replay matched the original
    pub matched: bool,
    /// Original decision from receipt
    pub original_decision: GateDecision,
    /// State snapshot at decision time
    pub state_snapshot: WitnessSummary,
    /// Differences if any
    pub differences: Vec<ReplayDifference>,
}

/// A difference found during replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDifference {
    /// Field that differs
    pub field: String,
    /// Original value
    pub original: String,
    /// Replayed value
    pub replayed: String,
}

/// Snapshot of state for replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Sequence number
    pub sequence: u64,
    /// Timestamp
    pub timestamp: u64,
    /// Global min-cut value
    pub global_min_cut: f64,
    /// Aggregate e-value
    pub aggregate_e_value: f64,
    /// Minimum coherence
    pub min_coherence: i16,
    /// Tile states
    pub tile_states: HashMap<u8, TileSnapshot>,
}

/// Snapshot of a single tile's state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileSnapshot {
    /// Tile ID
    pub tile_id: u8,
    /// Coherence
    pub coherence: i16,
    /// E-value
    pub e_value: f32,
    /// Boundary edge count
    pub boundary_edges: usize,
}

/// Engine for replaying decisions
pub struct ReplayEngine {
    /// Checkpoints for state restoration
    checkpoints: HashMap<u64, StateSnapshot>,
    /// Checkpoint interval
    checkpoint_interval: u64,
}

impl ReplayEngine {
    /// Create a new replay engine
    pub fn new(checkpoint_interval: u64) -> Self {
        Self {
            checkpoints: HashMap::new(),
            checkpoint_interval,
        }
    }

    /// Save a checkpoint
    pub fn save_checkpoint(&mut self, sequence: u64, snapshot: StateSnapshot) {
        if sequence % self.checkpoint_interval == 0 {
            self.checkpoints.insert(sequence, snapshot);
        }
    }

    /// Find the nearest checkpoint before a sequence
    pub fn find_nearest_checkpoint(&self, sequence: u64) -> Option<(u64, &StateSnapshot)> {
        self.checkpoints
            .iter()
            .filter(|(seq, _)| **seq <= sequence)
            .max_by_key(|(seq, _)| *seq)
            .map(|(seq, snap)| (*seq, snap))
    }

    /// Replay a decision from a receipt
    pub fn replay(&self, receipt: &WitnessReceipt) -> ReplayResult {
        // Get the witness summary from the receipt
        let summary = &receipt.witness_summary;

        // Reconstruct the decision based on the witness data
        let replayed_decision = self.reconstruct_decision(summary);

        // Compare with original
        let original_decision = receipt.token.decision;
        let matched = replayed_decision == original_decision;

        let mut differences = Vec::new();
        if !matched {
            differences.push(ReplayDifference {
                field: "decision".to_string(),
                original: format!("{:?}", original_decision),
                replayed: format!("{:?}", replayed_decision),
            });
        }

        ReplayResult {
            decision: replayed_decision,
            matched,
            original_decision,
            state_snapshot: summary.clone(),
            differences,
        }
    }

    /// Reconstruct decision from witness summary
    fn reconstruct_decision(&self, summary: &WitnessSummary) -> GateDecision {
        // Apply the same three-filter logic as in TileZero

        // 1. Structural filter
        if summary.structural.partition == "fragile" {
            return GateDecision::Deny;
        }

        // 2. Evidence filter
        if summary.evidential.verdict == "reject" {
            return GateDecision::Deny;
        }

        if summary.evidential.verdict == "continue" {
            return GateDecision::Defer;
        }

        // 3. Prediction filter
        if summary.predictive.set_size > 20 {
            return GateDecision::Defer;
        }

        GateDecision::Permit
    }

    /// Verify a sequence of receipts for consistency
    pub fn verify_sequence(&self, receipts: &[WitnessReceipt]) -> SequenceVerification {
        let mut results = Vec::new();
        let mut all_matched = true;

        for receipt in receipts {
            let result = self.replay(receipt);
            if !result.matched {
                all_matched = false;
            }
            results.push((receipt.sequence, result));
        }

        SequenceVerification {
            total_receipts: receipts.len(),
            all_matched,
            results,
        }
    }

    /// Export checkpoint for external storage
    pub fn export_checkpoint(&self, sequence: u64) -> Option<Vec<u8>> {
        self.checkpoints
            .get(&sequence)
            .and_then(|snap| serde_json::to_vec(snap).ok())
    }

    /// Import checkpoint from external storage
    pub fn import_checkpoint(&mut self, sequence: u64, data: &[u8]) -> Result<(), ReplayError> {
        let snapshot: StateSnapshot =
            serde_json::from_slice(data).map_err(|_| ReplayError::InvalidCheckpoint)?;
        self.checkpoints.insert(sequence, snapshot);
        Ok(())
    }

    /// Clear old checkpoints to manage memory
    pub fn prune_before(&mut self, sequence: u64) {
        self.checkpoints.retain(|seq, _| *seq >= sequence);
    }

    /// Get checkpoint count
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }
}

impl Default for ReplayEngine {
    fn default() -> Self {
        Self::new(100)
    }
}

/// Result of verifying a sequence of receipts
#[derive(Debug)]
pub struct SequenceVerification {
    /// Total number of receipts verified
    pub total_receipts: usize,
    /// Whether all replays matched
    pub all_matched: bool,
    /// Individual results
    pub results: Vec<(u64, ReplayResult)>,
}

impl SequenceVerification {
    /// Get the mismatches
    pub fn mismatches(&self) -> impl Iterator<Item = &(u64, ReplayResult)> {
        self.results.iter().filter(|(_, r)| !r.matched)
    }

    /// Get mismatch count
    pub fn mismatch_count(&self) -> usize {
        self.results.iter().filter(|(_, r)| !r.matched).count()
    }
}

/// Error during replay
#[derive(Debug, thiserror::Error)]
pub enum ReplayError {
    #[error("Receipt not found for sequence {sequence}")]
    ReceiptNotFound { sequence: u64 },
    #[error("Checkpoint not found for sequence {sequence}")]
    CheckpointNotFound { sequence: u64 },
    #[error("Invalid checkpoint data")]
    InvalidCheckpoint,
    #[error("State reconstruction failed: {reason}")]
    ReconstructionFailed { reason: String },
    #[error("Hash chain verification failed at sequence {sequence}")]
    ChainVerificationFailed { sequence: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvidentialWitness, PermitToken, PredictiveWitness, StructuralWitness, TimestampProof,
    };

    fn create_test_receipt(sequence: u64, decision: GateDecision) -> WitnessReceipt {
        WitnessReceipt {
            sequence,
            token: PermitToken {
                decision,
                action_id: format!("action-{}", sequence),
                timestamp: 1000 + sequence,
                ttl_ns: 60000,
                witness_hash: [0u8; 32],
                sequence,
                signature: [0u8; 64],
            },
            previous_hash: [0u8; 32],
            witness_summary: WitnessSummary {
                structural: StructuralWitness {
                    cut_value: 10.0,
                    partition: "stable".to_string(),
                    critical_edges: 0,
                    boundary: vec![],
                },
                predictive: PredictiveWitness {
                    set_size: 5,
                    coverage: 0.9,
                },
                evidential: EvidentialWitness {
                    e_value: 100.0,
                    verdict: "accept".to_string(),
                },
            },
            timestamp_proof: TimestampProof {
                timestamp: 1000 + sequence,
                previous_receipt_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
        }
    }

    #[test]
    fn test_replay_matching() {
        let engine = ReplayEngine::new(100);
        let receipt = create_test_receipt(0, GateDecision::Permit);

        let result = engine.replay(&receipt);
        assert!(result.matched);
        assert_eq!(result.decision, GateDecision::Permit);
    }

    #[test]
    fn test_replay_mismatch() {
        let engine = ReplayEngine::new(100);
        let mut receipt = create_test_receipt(0, GateDecision::Permit);

        // Modify the witness to indicate a deny condition
        receipt.witness_summary.structural.partition = "fragile".to_string();

        let result = engine.replay(&receipt);
        assert!(!result.matched);
        assert_eq!(result.decision, GateDecision::Deny);
        assert!(!result.differences.is_empty());
    }

    #[test]
    fn test_checkpoint_save_load() {
        let mut engine = ReplayEngine::new(10);

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 10.0,
            aggregate_e_value: 100.0,
            min_coherence: 256,
            tile_states: HashMap::new(),
        };

        engine.save_checkpoint(0, snapshot.clone());
        assert_eq!(engine.checkpoint_count(), 1);

        let (seq, found) = engine.find_nearest_checkpoint(5).unwrap();
        assert_eq!(seq, 0);
        assert_eq!(found.global_min_cut, 10.0);
    }

    #[test]
    fn test_sequence_verification() {
        let engine = ReplayEngine::new(100);

        let receipts = vec![
            create_test_receipt(0, GateDecision::Permit),
            create_test_receipt(1, GateDecision::Permit),
            create_test_receipt(2, GateDecision::Permit),
        ];

        let verification = engine.verify_sequence(&receipts);
        assert_eq!(verification.total_receipts, 3);
        assert!(verification.all_matched);
        assert_eq!(verification.mismatch_count(), 0);
    }

    #[test]
    fn test_prune_checkpoints() {
        let mut engine = ReplayEngine::new(10);

        for i in (0..100).step_by(10) {
            let snapshot = StateSnapshot {
                sequence: i as u64,
                timestamp: 1000 + i as u64,
                global_min_cut: 10.0,
                aggregate_e_value: 100.0,
                min_coherence: 256,
                tile_states: HashMap::new(),
            };
            engine.save_checkpoint(i as u64, snapshot);
        }

        assert_eq!(engine.checkpoint_count(), 10);

        engine.prune_before(50);
        assert_eq!(engine.checkpoint_count(), 5);
    }

    #[test]
    fn test_checkpoint_export_import() {
        let mut engine = ReplayEngine::new(10);

        let snapshot = StateSnapshot {
            sequence: 0,
            timestamp: 1000,
            global_min_cut: 10.0,
            aggregate_e_value: 100.0,
            min_coherence: 256,
            tile_states: HashMap::new(),
        };

        engine.save_checkpoint(0, snapshot);
        let exported = engine.export_checkpoint(0).unwrap();

        let mut engine2 = ReplayEngine::new(10);
        engine2.import_checkpoint(0, &exported).unwrap();
        assert_eq!(engine2.checkpoint_count(), 1);
    }
}
