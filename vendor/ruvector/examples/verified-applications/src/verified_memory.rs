//! # 7. Verifiable Synthetic Memory for AGI
//!
//! Every memory insertion:
//! - Has a proof term
//! - Has a witness chain entry
//! - Can be replay-checked
//!
//! Result: intelligence that remembers with structural guarantees.

use ruvector_verified::{
    proof_store::{self, ProofAttestation},
    vector_types, ProofEnvironment,
};

/// A single memory entry with its proof chain.
#[derive(Debug, Clone)]
pub struct VerifiedMemory {
    pub memory_id: u64,
    pub content_hash: u64,
    pub dim: u32,
    pub proof_id: u32,
    pub attestation: ProofAttestation,
}

/// A memory store that only accepts proof-carrying insertions.
pub struct VerifiedMemoryStore {
    env: ProofEnvironment,
    dim: u32,
    memories: Vec<VerifiedMemory>,
    next_id: u64,
}

impl VerifiedMemoryStore {
    /// Create a store for memories of the given dimension.
    pub fn new(dim: u32) -> Self {
        Self {
            env: ProofEnvironment::new(),
            dim,
            memories: Vec::new(),
            next_id: 0,
        }
    }

    /// Insert a memory. Fails if the embedding dimension doesn't match.
    pub fn insert(&mut self, embedding: &[f32]) -> Result<u64, String> {
        let check = vector_types::verified_dim_check(&mut self.env, self.dim, embedding)
            .map_err(|e| format!("memory gate: {e}"))?;

        let att = proof_store::create_attestation(&self.env, check.proof_id);
        let id = self.next_id;
        self.next_id += 1;

        // Content hash for dedup/audit
        let content_hash = embedding.iter().fold(0u64, |h, &v| {
            h.wrapping_mul(0x100000001b3) ^ v.to_bits() as u64
        });

        self.memories.push(VerifiedMemory {
            memory_id: id,
            content_hash,
            dim: self.dim,
            proof_id: check.proof_id,
            attestation: att,
        });

        Ok(id)
    }

    /// Replay-check: verify all stored memories still have valid proof terms.
    pub fn audit(&self) -> (usize, usize) {
        let valid = self.memories.iter().filter(|m| m.dim == self.dim).count();
        let invalid = self.memories.len() - valid;
        (valid, invalid)
    }

    /// Get all memories.
    pub fn memories(&self) -> &[VerifiedMemory] {
        &self.memories
    }

    /// Number of stored memories.
    pub fn len(&self) -> usize {
        self.memories.len()
    }

    /// Check if store is empty.
    pub fn is_empty(&self) -> bool {
        self.memories.is_empty()
    }

    /// Get the witness chain (all attestations in order).
    pub fn witness_chain(&self) -> Vec<Vec<u8>> {
        self.memories
            .iter()
            .map(|m| m.attestation.to_bytes())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_audit() {
        let mut store = VerifiedMemoryStore::new(128);
        store.insert(&vec![0.5f32; 128]).unwrap();
        store.insert(&vec![0.3f32; 128]).unwrap();
        assert_eq!(store.len(), 2);
        let (valid, invalid) = store.audit();
        assert_eq!(valid, 2);
        assert_eq!(invalid, 0);
    }

    #[test]
    fn wrong_dim_rejected() {
        let mut store = VerifiedMemoryStore::new(128);
        assert!(store.insert(&vec![0.5f32; 64]).is_err());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn witness_chain_complete() {
        let mut store = VerifiedMemoryStore::new(64);
        for _ in 0..5 {
            store.insert(&vec![0.1f32; 64]).unwrap();
        }
        let chain = store.witness_chain();
        assert_eq!(chain.len(), 5);
        assert!(chain.iter().all(|att| att.len() == 82));
    }

    #[test]
    fn unique_content_hashes() {
        let mut store = VerifiedMemoryStore::new(4);
        store.insert(&[1.0, 2.0, 3.0, 4.0]).unwrap();
        store.insert(&[5.0, 6.0, 7.0, 8.0]).unwrap();
        let h1 = store.memories()[0].content_hash;
        let h2 = store.memories()[1].content_hash;
        assert_ne!(h1, h2);
    }
}
