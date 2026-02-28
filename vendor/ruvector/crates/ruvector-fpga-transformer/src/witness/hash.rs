//! Witness hashing for integrity verification

use crate::types::WitnessLog;
use sha2::{Digest, Sha256};

/// Compute a hash of the witness log for integrity verification
pub fn compute_witness_hash(witness: &WitnessLog) -> [u8; 32] {
    let mut hasher = Sha256::new();

    hasher.update(&witness.model_hash);
    hasher.update(&witness.quant_hash);
    hasher.update(&[witness.backend as u8]);
    hasher.update(&witness.cycles.to_le_bytes());
    hasher.update(&witness.latency_ns.to_le_bytes());

    // Hash gate decision
    match witness.gate_decision {
        crate::types::GateDecision::RanFull => {
            hasher.update(&[0u8]);
        }
        crate::types::GateDecision::EarlyExit { layer } => {
            hasher.update(&[1u8, layer]);
        }
        crate::types::GateDecision::Skipped { reason } => {
            hasher.update(&[2u8, reason as u8]);
        }
    }

    hasher.finalize().into()
}

/// Verify a witness hash
pub fn verify_witness_hash(witness: &WitnessLog, expected: &[u8; 32]) -> bool {
    let computed = compute_witness_hash(witness);
    computed == *expected
}

/// Compute a combined hash for a sequence of witnesses
/// Useful for verifying an entire inference chain
pub fn compute_chain_hash(witnesses: &[WitnessLog]) -> [u8; 32] {
    let mut hasher = Sha256::new();

    for witness in witnesses {
        let witness_hash = compute_witness_hash(witness);
        hasher.update(&witness_hash);
    }

    hasher.finalize().into()
}

/// Witness proof for verification
#[derive(Debug, Clone)]
pub struct WitnessProof {
    /// Hash of the witness
    pub hash: [u8; 32],
    /// Timestamp when proof was created
    pub timestamp_ns: u64,
    /// Optional signature
    pub signature: Option<[u8; 64]>,
}

impl WitnessProof {
    /// Create a new proof from a witness
    pub fn new(witness: &WitnessLog) -> Self {
        Self {
            hash: compute_witness_hash(witness),
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            signature: None,
        }
    }

    /// Create a proof with signature
    #[cfg(feature = "sign")]
    pub fn signed(witness: &WitnessLog, secret_key: &[u8; 32]) -> Self {
        use ed25519_dalek::{Signer, SigningKey};

        let hash = compute_witness_hash(witness);
        let timestamp_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        // Create message to sign
        let mut message = [0u8; 40];
        message[..32].copy_from_slice(&hash);
        message[32..40].copy_from_slice(&timestamp_ns.to_le_bytes());

        let signing_key = SigningKey::from_bytes(secret_key);
        let signature = signing_key.sign(&message);

        Self {
            hash,
            timestamp_ns,
            signature: Some(signature.to_bytes()),
        }
    }

    /// Verify the proof against a witness
    pub fn verify(&self, witness: &WitnessLog) -> bool {
        verify_witness_hash(witness, &self.hash)
    }

    /// Verify the signature
    #[cfg(feature = "sign")]
    pub fn verify_signature(&self, pubkey: &[u8; 32]) -> bool {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};

        let Some(sig_bytes) = self.signature else {
            return false;
        };

        let Ok(verifying_key) = VerifyingKey::from_bytes(pubkey) else {
            return false;
        };

        let signature = Signature::from_bytes(&sig_bytes);

        let mut message = [0u8; 40];
        message[..32].copy_from_slice(&self.hash);
        message[32..40].copy_from_slice(&self.timestamp_ns.to_le_bytes());

        verifying_key.verify(&message, &signature).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BackendKind, GateDecision};

    #[test]
    fn test_witness_hash_deterministic() {
        let witness = WitnessLog::new(
            [1u8; 32],
            [2u8; 32],
            BackendKind::NativeSim,
            1000,
            50000,
            GateDecision::RanFull,
        );

        let hash1 = compute_witness_hash(&witness);
        let hash2 = compute_witness_hash(&witness);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_witness_hash_changes() {
        let witness1 = WitnessLog::new(
            [1u8; 32],
            [2u8; 32],
            BackendKind::NativeSim,
            1000,
            50000,
            GateDecision::RanFull,
        );

        let witness2 = WitnessLog::new(
            [1u8; 32],
            [2u8; 32],
            BackendKind::NativeSim,
            1001, // Different cycles
            50000,
            GateDecision::RanFull,
        );

        let hash1 = compute_witness_hash(&witness1);
        let hash2 = compute_witness_hash(&witness2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_witness_hash() {
        let witness = WitnessLog::empty();
        let hash = compute_witness_hash(&witness);

        assert!(verify_witness_hash(&witness, &hash));
        assert!(!verify_witness_hash(&witness, &[0u8; 32]));
    }

    #[test]
    fn test_chain_hash() {
        let witnesses: Vec<WitnessLog> = (0..5)
            .map(|i| {
                WitnessLog::new(
                    [i as u8; 32],
                    [0u8; 32],
                    BackendKind::NativeSim,
                    i * 100,
                    i * 1000,
                    GateDecision::RanFull,
                )
            })
            .collect();

        let chain_hash1 = compute_chain_hash(&witnesses);
        let chain_hash2 = compute_chain_hash(&witnesses);

        assert_eq!(chain_hash1, chain_hash2);
    }

    #[test]
    fn test_witness_proof() {
        let witness = WitnessLog::empty();
        let proof = WitnessProof::new(&witness);

        assert!(proof.verify(&witness));
        assert!(proof.timestamp_ns > 0);
    }
}
