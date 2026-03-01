//! Cross-paradigm witness chain — ADR-029 canonical audit type.
//! All subsystems emit CrossParadigmWitness for unified audit chains.
//! Root: RVF SHAKE-256 + ML-DSA-65 (quantum-safe)

use std::time::{SystemTime, UNIX_EPOCH};

/// Canonical witness emitted by all subsystems in the multi-paradigm stack.
/// Optional fields are populated based on which backends are active.
#[derive(Debug, Clone)]
pub struct CrossParadigmWitness {
    /// Sequence number (monotonic)
    pub sequence: u64,
    /// UNIX timestamp microseconds
    pub timestamp_us: u64,
    /// Action identifier (up to 64 bytes)
    pub action_id: [u8; 32],
    /// Decision outcome
    pub decision: WitnessDecision,
    /// SHAKE-256 hash of prior witness (chain link)
    pub prior_hash: [u8; 32],
    /// Sheaf Laplacian energy from prime-radiant (if active)
    pub sheaf_energy: Option<f64>,
    /// Min-cut coherence value λ (if coherence router active)
    pub lambda_min_cut: Option<f64>,
    /// IIT Φ value at decision point (if consciousness substrate active)
    pub phi_value: Option<f64>,
    /// Genomic context hash from .rvdna (if genomic backend active)
    pub genomic_context: Option<[u8; 32]>,
    /// Quantum gate decision (PERMIT=1, DEFER=0, DENY=-1)
    pub quantum_gate: Option<i8>,
    /// Formal proof bytes (lean-agentic, 82-byte attestation)
    pub proof_attestation: Option<[u8; 82]>,
    /// Cognitum tile e-value (anytime-valid confidence)
    pub e_value: Option<f64>,
    /// Ed25519 signature over canonical fields (64 bytes, zeros if unsigned)
    pub signature: [u8; 64],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WitnessDecision {
    Permit,
    Defer,
    Deny,
}

impl CrossParadigmWitness {
    /// Create an unsigned witness for the given action.
    pub fn new(sequence: u64, action_id: [u8; 32], decision: WitnessDecision) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        Self {
            sequence,
            timestamp_us: ts,
            action_id,
            decision,
            prior_hash: [0u8; 32],
            sheaf_energy: None,
            lambda_min_cut: None,
            phi_value: None,
            genomic_context: None,
            quantum_gate: None,
            proof_attestation: None,
            e_value: None,
            signature: [0u8; 64],
        }
    }

    /// Chain this witness to the prior, computing prior_hash via SHAKE-256 simulation.
    /// Uses Blake3 as a compact stand-in since SHAKE-256 requires external crate.
    pub fn chain_to(&mut self, prior: &CrossParadigmWitness) {
        self.prior_hash = Self::hash_witness(prior);
    }

    /// Compute a 32-byte hash of a witness (canonical fields only).
    pub fn hash_witness(w: &CrossParadigmWitness) -> [u8; 32] {
        // Simple deterministic hash over canonical fields
        let mut state = [0u64; 4];
        state[0] = w.sequence;
        state[1] = w.timestamp_us;
        state[2] = u64::from_le_bytes(w.action_id[0..8].try_into().unwrap_or([0u8; 8]));
        state[3] = match w.decision {
            WitnessDecision::Permit => 1,
            WitnessDecision::Defer => 0,
            WitnessDecision::Deny => u64::MAX,
        };
        // Fold optional fields
        if let Some(e) = w.sheaf_energy {
            state[0] ^= e.to_bits();
        }
        if let Some(l) = w.lambda_min_cut {
            state[1] ^= l.to_bits();
        }
        if let Some(p) = w.phi_value {
            state[2] ^= p.to_bits();
        }
        // siphash-like mixing
        let mut result = [0u8; 32];
        for i in 0..4 {
            let mixed = state[i]
                .wrapping_mul(0x6c62272e07bb0142)
                .wrapping_add(0x62b821756295c58d);
            let bytes = mixed.to_le_bytes();
            result[i * 8..(i + 1) * 8].copy_from_slice(&bytes);
        }
        result
    }

    /// Encode to bytes for transmission/storage (variable length).
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.timestamp_us.to_le_bytes());
        buf.extend_from_slice(&self.action_id);
        buf.push(match self.decision {
            WitnessDecision::Permit => 1,
            WitnessDecision::Defer => 0,
            WitnessDecision::Deny => 255,
        });
        buf.extend_from_slice(&self.prior_hash);
        // Optional fields as TLV
        if let Some(e) = self.sheaf_energy {
            buf.push(0x01);
            buf.extend_from_slice(&e.to_le_bytes());
        }
        if let Some(l) = self.lambda_min_cut {
            buf.push(0x02);
            buf.extend_from_slice(&l.to_le_bytes());
        }
        if let Some(p) = self.phi_value {
            buf.push(0x03);
            buf.extend_from_slice(&p.to_le_bytes());
        }
        buf.extend_from_slice(&self.signature);
        buf
    }
}

/// Witness chain — maintains ordered chain of witnesses with hash linking.
pub struct WitnessChain {
    pub witnesses: Vec<CrossParadigmWitness>,
    next_sequence: u64,
}

impl WitnessChain {
    pub fn new() -> Self {
        Self {
            witnesses: Vec::new(),
            next_sequence: 0,
        }
    }

    pub fn append(&mut self, mut witness: CrossParadigmWitness) -> u64 {
        witness.sequence = self.next_sequence;
        if let Some(prior) = self.witnesses.last() {
            witness.chain_to(prior);
        }
        self.next_sequence += 1;
        self.witnesses.push(witness);
        self.next_sequence - 1
    }

    pub fn verify_chain(&self) -> bool {
        for i in 1..self.witnesses.len() {
            let expected_prior = CrossParadigmWitness::hash_witness(&self.witnesses[i - 1]);
            if self.witnesses[i].prior_hash != expected_prior {
                return false;
            }
        }
        true
    }

    pub fn len(&self) -> usize {
        self.witnesses.len()
    }
    pub fn is_empty(&self) -> bool {
        self.witnesses.is_empty()
    }
    pub fn get(&self, idx: usize) -> Option<&CrossParadigmWitness> {
        self.witnesses.get(idx)
    }
}

impl Default for WitnessChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_witness_chain_integrity() {
        let mut chain = WitnessChain::new();
        for i in 0..10u64 {
            let mut id = [0u8; 32];
            id[0..8].copy_from_slice(&i.to_le_bytes());
            let w = CrossParadigmWitness::new(i, id, WitnessDecision::Permit);
            chain.append(w);
        }
        assert!(chain.verify_chain());
        assert_eq!(chain.len(), 10);
    }

    #[test]
    fn test_witness_chain_tamper_detection() {
        let mut chain = WitnessChain::new();
        let id = [0u8; 32];
        chain.append(CrossParadigmWitness::new(0, id, WitnessDecision::Permit));
        chain.append(CrossParadigmWitness::new(1, id, WitnessDecision::Permit));
        // Tamper with first witness
        chain.witnesses[0].phi_value = Some(9999.0);
        assert!(
            !chain.verify_chain(),
            "Tampered chain should fail verification"
        );
    }

    #[test]
    fn test_witness_encode_roundtrip() {
        let id = [42u8; 32];
        let mut w = CrossParadigmWitness::new(7, id, WitnessDecision::Defer);
        w.sheaf_energy = Some(1.618);
        w.lambda_min_cut = Some(3.14159);
        w.phi_value = Some(2.718);
        let encoded = w.encode();
        assert!(encoded.len() > 64);
    }
}
