//! Witness receipt and hash-chained log

use crate::{ChainVerifyError, PermitToken};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Witness receipt: cryptographic proof of a gate decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessReceipt {
    /// Sequence number
    pub sequence: u64,
    /// The permit token issued
    pub token: PermitToken,
    /// Hash of the previous receipt
    #[serde(with = "hex::serde")]
    pub previous_hash: [u8; 32],
    /// Summary of witness data
    pub witness_summary: WitnessSummary,
    /// Timestamp proof
    pub timestamp_proof: TimestampProof,
}

impl WitnessReceipt {
    /// Compute the hash of this receipt
    pub fn hash(&self) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.sequence.to_le_bytes());
        hasher.update(&self.token.signable_content());
        hasher.update(&self.previous_hash);
        hasher.update(&self.witness_summary.hash());
        *hasher.finalize().as_bytes()
    }
}

/// Timestamp proof for receipts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimestampProof {
    /// Timestamp
    pub timestamp: u64,
    /// Hash of previous receipt
    #[serde(with = "hex::serde")]
    pub previous_receipt_hash: [u8; 32],
    /// Merkle root (for batch anchoring)
    #[serde(with = "hex::serde")]
    pub merkle_root: [u8; 32],
}

/// Summary of witness data from the three filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessSummary {
    /// Structural witness
    pub structural: StructuralWitness,
    /// Predictive witness
    pub predictive: PredictiveWitness,
    /// Evidential witness
    pub evidential: EvidentialWitness,
}

impl WitnessSummary {
    /// Create an empty witness summary
    pub fn empty() -> Self {
        Self {
            structural: StructuralWitness {
                cut_value: 0.0,
                partition: "unknown".to_string(),
                critical_edges: 0,
                boundary: vec![],
            },
            predictive: PredictiveWitness {
                set_size: 0,
                coverage: 0.0,
            },
            evidential: EvidentialWitness {
                e_value: 1.0,
                verdict: "unknown".to_string(),
            },
        }
    }

    /// Compute hash of the summary
    pub fn hash(&self) -> [u8; 32] {
        let json = serde_json::to_vec(self).unwrap_or_default();
        *blake3::hash(&json).as_bytes()
    }

    /// Convert to JSON
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }
}

/// Structural witness from min-cut analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralWitness {
    /// Cut value
    pub cut_value: f64,
    /// Partition status
    pub partition: String,
    /// Number of critical edges
    pub critical_edges: usize,
    /// Boundary edge IDs
    #[serde(default)]
    pub boundary: Vec<String>,
}

/// Predictive witness from conformal prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictiveWitness {
    /// Prediction set size
    pub set_size: usize,
    /// Coverage target
    pub coverage: f64,
}

/// Evidential witness from e-process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidentialWitness {
    /// Accumulated e-value
    pub e_value: f64,
    /// Verdict (accept/continue/reject)
    pub verdict: String,
}

/// Hash-chained receipt log
pub struct ReceiptLog {
    /// Receipts by sequence number
    receipts: HashMap<u64, WitnessReceipt>,
    /// Latest sequence number
    latest_sequence: Option<u64>,
    /// Hash of the latest receipt
    latest_hash: [u8; 32],
}

impl ReceiptLog {
    /// Create a new receipt log
    pub fn new() -> Self {
        Self {
            receipts: HashMap::new(),
            latest_sequence: None,
            latest_hash: [0u8; 32], // Genesis hash
        }
    }

    /// Get the last hash in the chain
    pub fn last_hash(&self) -> [u8; 32] {
        self.latest_hash
    }

    /// Append a receipt to the log
    pub fn append(&mut self, receipt: WitnessReceipt) {
        let hash = receipt.hash();
        let seq = receipt.sequence;
        self.receipts.insert(seq, receipt);
        self.latest_sequence = Some(seq);
        self.latest_hash = hash;
    }

    /// Get a receipt by sequence number
    pub fn get(&self, sequence: u64) -> Option<&WitnessReceipt> {
        self.receipts.get(&sequence)
    }

    /// Get the latest sequence number
    pub fn latest_sequence(&self) -> Option<u64> {
        self.latest_sequence
    }

    /// Verify the hash chain up to a sequence number
    pub fn verify_chain_to(&self, sequence: u64) -> Result<(), ChainVerifyError> {
        let mut expected_previous = [0u8; 32]; // Genesis

        for seq in 0..=sequence {
            let receipt = self
                .receipts
                .get(&seq)
                .ok_or(ChainVerifyError::ReceiptNotFound { sequence: seq })?;

            if receipt.previous_hash != expected_previous {
                return Err(ChainVerifyError::HashMismatch { sequence: seq });
            }

            expected_previous = receipt.hash();
        }

        Ok(())
    }

    /// Get the number of receipts
    pub fn len(&self) -> usize {
        self.receipts.len()
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.receipts.is_empty()
    }

    /// Iterate over receipts
    pub fn iter(&self) -> impl Iterator<Item = &WitnessReceipt> {
        self.receipts.values()
    }
}

impl Default for ReceiptLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GateDecision;

    #[test]
    fn test_receipt_hash() {
        let receipt = WitnessReceipt {
            sequence: 0,
            token: PermitToken {
                decision: GateDecision::Permit,
                action_id: "test".to_string(),
                timestamp: 1000,
                ttl_ns: 60000,
                witness_hash: [0u8; 32],
                sequence: 0,
                signature: [0u8; 64],
            },
            previous_hash: [0u8; 32],
            witness_summary: WitnessSummary::empty(),
            timestamp_proof: TimestampProof {
                timestamp: 1000,
                previous_receipt_hash: [0u8; 32],
                merkle_root: [0u8; 32],
            },
        };

        let hash = receipt.hash();
        assert_ne!(hash, [0u8; 32]);
    }

    #[test]
    fn test_receipt_log_chain() {
        let mut log = ReceiptLog::new();

        for i in 0..3 {
            let receipt = WitnessReceipt {
                sequence: i,
                token: PermitToken {
                    decision: GateDecision::Permit,
                    action_id: format!("action-{}", i),
                    timestamp: 1000 + i,
                    ttl_ns: 60000,
                    witness_hash: [0u8; 32],
                    sequence: i,
                    signature: [0u8; 64],
                },
                previous_hash: log.last_hash(),
                witness_summary: WitnessSummary::empty(),
                timestamp_proof: TimestampProof {
                    timestamp: 1000 + i,
                    previous_receipt_hash: log.last_hash(),
                    merkle_root: [0u8; 32],
                },
            };
            log.append(receipt);
        }

        assert_eq!(log.len(), 3);
        assert!(log.verify_chain_to(2).is_ok());
    }
}
