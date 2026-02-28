use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Coherence decision emitted after each epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceDecision {
    Pass,
    Fail { severity: u8 },
    Inconclusive,
}

/// A single witness receipt linking an epoch to its predecessor via hashes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerWitnessReceipt {
    /// Epoch number this receipt covers.
    pub epoch: u64,
    /// Hash of the previous receipt (zero for the genesis receipt).
    pub prev_hash: [u8; 32],
    /// Hash of the input deltas for this epoch.
    pub input_hash: [u8; 32],
    /// Hash of the min-cut result.
    pub mincut_hash: [u8; 32],
    /// Spectral coherence score in fixed-point 32.32 representation.
    pub spectral_scs: u64,
    /// Hash of the evidence accumulation state.
    pub evidence_hash: [u8; 32],
    /// Decision for this epoch.
    pub decision: CoherenceDecision,
    /// Hash of this receipt (covers all fields above).
    pub receipt_hash: [u8; 32],
}

impl ContainerWitnessReceipt {
    /// Serialize all fields except `receipt_hash` into a byte vector for hashing.
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(&self.epoch.to_le_bytes());
        buf.extend_from_slice(&self.prev_hash);
        buf.extend_from_slice(&self.input_hash);
        buf.extend_from_slice(&self.mincut_hash);
        buf.extend_from_slice(&self.spectral_scs.to_le_bytes());
        buf.extend_from_slice(&self.evidence_hash);
        match self.decision {
            CoherenceDecision::Pass => buf.push(0),
            CoherenceDecision::Fail { severity } => {
                buf.push(1);
                buf.push(severity);
            }
            CoherenceDecision::Inconclusive => buf.push(2),
        }
        buf
    }

    /// Compute and set `receipt_hash` from the signable portion of this receipt.
    pub fn compute_hash(&mut self) {
        self.receipt_hash = deterministic_hash(&self.signable_bytes());
    }
}

/// Result of verifying a witness chain.
#[derive(Debug, Clone)]
pub enum VerificationResult {
    /// Chain is valid.
    Valid {
        chain_length: usize,
        first_epoch: u64,
        last_epoch: u64,
    },
    /// Chain is empty (no receipts).
    Empty,
    /// A receipt's `prev_hash` does not match the preceding receipt's `receipt_hash`.
    BrokenChain { epoch: u64 },
    /// Epoch numbers are not strictly monotonic.
    EpochGap { expected: u64, got: u64 },
}

/// Append-only chain of witness receipts with hash linking.
pub struct WitnessChain {
    current_epoch: u64,
    prev_hash: [u8; 32],
    receipts: Vec<ContainerWitnessReceipt>,
    max_receipts: usize,
}

impl WitnessChain {
    /// Create a new empty chain that retains at most `max_receipts` entries.
    pub fn new(max_receipts: usize) -> Self {
        Self {
            current_epoch: 0,
            prev_hash: [0u8; 32],
            receipts: Vec::with_capacity(max_receipts.min(1024)),
            max_receipts,
        }
    }

    /// Generate a new receipt, append it to the chain, and return a clone.
    pub fn generate_receipt(
        &mut self,
        input_deltas: &[u8],
        mincut_data: &[u8],
        spectral_scs: f64,
        evidence_data: &[u8],
        decision: CoherenceDecision,
    ) -> ContainerWitnessReceipt {
        let scs_fixed = f64_to_fixed_32_32(spectral_scs);

        let mut receipt = ContainerWitnessReceipt {
            epoch: self.current_epoch,
            prev_hash: self.prev_hash,
            input_hash: deterministic_hash(input_deltas),
            mincut_hash: deterministic_hash(mincut_data),
            spectral_scs: scs_fixed,
            evidence_hash: deterministic_hash(evidence_data),
            decision,
            receipt_hash: [0u8; 32],
        };
        receipt.compute_hash();

        self.prev_hash = receipt.receipt_hash;
        self.current_epoch += 1;

        // Ring-buffer behavior: drop oldest when full.
        if self.receipts.len() >= self.max_receipts {
            self.receipts.remove(0);
        }
        self.receipts.push(receipt.clone());

        receipt
    }

    /// Current epoch counter (next epoch to be generated).
    pub fn current_epoch(&self) -> u64 {
        self.current_epoch
    }

    /// Most recent receipt, if any.
    pub fn latest_receipt(&self) -> Option<&ContainerWitnessReceipt> {
        self.receipts.last()
    }

    /// Slice of all retained receipts.
    pub fn receipt_chain(&self) -> &[ContainerWitnessReceipt] {
        &self.receipts
    }

    /// Verify hash-chain integrity and epoch monotonicity for a slice of receipts.
    pub fn verify_chain(receipts: &[ContainerWitnessReceipt]) -> VerificationResult {
        if receipts.is_empty() {
            return VerificationResult::Empty;
        }

        // Verify each receipt's self-hash.
        for r in receipts {
            let expected = deterministic_hash(&r.signable_bytes());
            if expected != r.receipt_hash {
                return VerificationResult::BrokenChain { epoch: r.epoch };
            }
        }

        // Verify prev_hash linkage and epoch ordering.
        for i in 1..receipts.len() {
            let prev = &receipts[i - 1];
            let curr = &receipts[i];

            if curr.prev_hash != prev.receipt_hash {
                return VerificationResult::BrokenChain { epoch: curr.epoch };
            }

            let expected_epoch = prev.epoch + 1;
            if curr.epoch != expected_epoch {
                return VerificationResult::EpochGap {
                    expected: expected_epoch,
                    got: curr.epoch,
                };
            }
        }

        VerificationResult::Valid {
            chain_length: receipts.len(),
            first_epoch: receipts[0].epoch,
            last_epoch: receipts[receipts.len() - 1].epoch,
        }
    }
}

/// Convert an f64 to a 32.32 fixed-point representation.
fn f64_to_fixed_32_32(value: f64) -> u64 {
    let clamped = value.clamp(0.0, (u32::MAX as f64) + 0.999_999_999);
    (clamped * (1u64 << 32) as f64) as u64
}

/// Public wrapper for deterministic hashing, used by other modules.
pub fn deterministic_hash_public(data: &[u8]) -> [u8; 32] {
    deterministic_hash(data)
}

/// Deterministic hash producing 32 bytes.
///
/// Uses `std::hash::DefaultHasher` (SipHash-2-4) run with four different seeds
/// to fill 32 bytes. This is NOT cryptographic but fully deterministic across
/// runs on the same platform.
fn deterministic_hash(data: &[u8]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0u64..4 {
        let mut hasher = DefaultHasher::new();
        i.hash(&mut hasher);
        data.hash(&mut hasher);
        let h = hasher.finish();
        let offset = (i as usize) * 8;
        result[offset..offset + 8].copy_from_slice(&h.to_le_bytes());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_hash_consistency() {
        let a = deterministic_hash(b"hello world");
        let b = deterministic_hash(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn test_deterministic_hash_differs_for_different_inputs() {
        let a = deterministic_hash(b"alpha");
        let b = deterministic_hash(b"beta");
        assert_ne!(a, b);
    }

    #[test]
    fn test_witness_chain_integrity() {
        let mut chain = WitnessChain::new(100);

        for i in 0..5 {
            let data = format!("epoch-{i}");
            chain.generate_receipt(
                data.as_bytes(),
                b"mincut",
                0.95,
                b"evidence",
                CoherenceDecision::Pass,
            );
        }

        assert_eq!(chain.current_epoch(), 5);

        match WitnessChain::verify_chain(chain.receipt_chain()) {
            VerificationResult::Valid {
                chain_length,
                first_epoch,
                last_epoch,
            } => {
                assert_eq!(chain_length, 5);
                assert_eq!(first_epoch, 0);
                assert_eq!(last_epoch, 4);
            }
            other => panic!("Expected Valid, got {other:?}"),
        }
    }

    #[test]
    fn test_witness_chain_epoch_monotonicity() {
        let mut chain = WitnessChain::new(100);
        for _ in 0..3 {
            chain.generate_receipt(
                b"input",
                b"mincut",
                1.0,
                b"evidence",
                CoherenceDecision::Pass,
            );
        }

        let receipts = chain.receipt_chain();
        for i in 1..receipts.len() {
            assert_eq!(receipts[i].epoch, receipts[i - 1].epoch + 1);
        }
    }

    #[test]
    fn test_verification_detects_tampering() {
        let mut chain = WitnessChain::new(100);
        for _ in 0..3 {
            chain.generate_receipt(
                b"input",
                b"mincut",
                0.5,
                b"evidence",
                CoherenceDecision::Inconclusive,
            );
        }

        // Tamper with the second receipt's input_hash.
        let mut tampered: Vec<ContainerWitnessReceipt> = chain.receipt_chain().to_vec();
        tampered[1].input_hash[0] ^= 0xFF;

        match WitnessChain::verify_chain(&tampered) {
            VerificationResult::BrokenChain { epoch } => {
                assert_eq!(epoch, 1);
            }
            other => panic!("Expected BrokenChain, got {other:?}"),
        }
    }

    #[test]
    fn test_empty_chain_verification() {
        let receipts: Vec<ContainerWitnessReceipt> = vec![];
        match WitnessChain::verify_chain(&receipts) {
            VerificationResult::Empty => {}
            other => panic!("Expected Empty, got {other:?}"),
        }
    }

    #[test]
    fn test_ring_buffer_eviction() {
        let mut chain = WitnessChain::new(3);
        for _ in 0..5 {
            chain.generate_receipt(b"data", b"mc", 0.1, b"ev", CoherenceDecision::Pass);
        }
        assert_eq!(chain.receipt_chain().len(), 3);
        assert_eq!(chain.receipt_chain()[0].epoch, 2);
        assert_eq!(chain.receipt_chain()[2].epoch, 4);
    }

    #[test]
    fn test_f64_to_fixed() {
        assert_eq!(f64_to_fixed_32_32(1.0), 1u64 << 32);
        assert_eq!(f64_to_fixed_32_32(0.0), 0);
        let half = f64_to_fixed_32_32(0.5);
        assert_eq!(half, 1u64 << 31);
    }

    #[test]
    fn test_signable_bytes_determinism() {
        let receipt = ContainerWitnessReceipt {
            epoch: 42,
            prev_hash: [1u8; 32],
            input_hash: [2u8; 32],
            mincut_hash: [3u8; 32],
            spectral_scs: 100,
            evidence_hash: [4u8; 32],
            decision: CoherenceDecision::Fail { severity: 7 },
            receipt_hash: [0u8; 32],
        };
        let a = receipt.signable_bytes();
        let b = receipt.signable_bytes();
        assert_eq!(a, b);
    }
}
