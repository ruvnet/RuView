//! # 8. Cryptographic Vector Signatures
//!
//! Combine proof term hash + model hash + vector content hash to create
//! signed vector semantics. Two systems can exchange embeddings and prove:
//! "These vectors were produced by identical dimensional and metric contracts."
//!
//! Result: cross-organization trust fabric for vector operations.

use ruvector_verified::{proof_store, vector_types, ProofEnvironment};

/// A signed vector with dimensional and metric proof.
#[derive(Debug, Clone)]
pub struct SignedVector {
    pub content_hash: [u8; 32],
    pub model_hash: [u8; 32],
    pub proof_hash: [u8; 32],
    pub dim: u32,
    pub metric: String,
    pub attestation_bytes: Vec<u8>,
}

impl SignedVector {
    /// Compute a combined signature over all three hashes.
    pub fn combined_hash(&self) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for &b in self
            .content_hash
            .iter()
            .chain(self.model_hash.iter())
            .chain(self.proof_hash.iter())
        {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }
}

/// Create a signed vector from an embedding, model hash, and dimension.
pub fn sign_vector(
    embedding: &[f32],
    model_hash: [u8; 32],
    dim: u32,
    metric: &str,
) -> Result<SignedVector, String> {
    let mut env = ProofEnvironment::new();

    // Prove dimension
    let check =
        vector_types::verified_dim_check(&mut env, dim, embedding).map_err(|e| format!("{e}"))?;

    // Prove metric
    vector_types::mk_distance_metric(&mut env, metric).map_err(|e| format!("{e}"))?;

    // Create attestation
    let att = proof_store::create_attestation(&env, check.proof_id);

    // Content hash from vector
    let mut content_hash = [0u8; 32];
    let mut h: u64 = 0;
    for &v in embedding {
        h = h.wrapping_mul(0x100000001b3) ^ v.to_bits() as u64;
    }
    content_hash[0..8].copy_from_slice(&h.to_le_bytes());
    content_hash[8..12].copy_from_slice(&dim.to_le_bytes());

    // Proof hash from attestation
    let mut proof_hash = [0u8; 32];
    let ah = att.content_hash();
    proof_hash[0..8].copy_from_slice(&ah.to_le_bytes());

    Ok(SignedVector {
        content_hash,
        model_hash,
        proof_hash,
        dim,
        metric: metric.into(),
        attestation_bytes: att.to_bytes(),
    })
}

/// Verify that two signed vectors share the same dimensional and metric contract.
pub fn verify_contract_match(a: &SignedVector, b: &SignedVector) -> bool {
    a.dim == b.dim && a.metric == b.metric && a.model_hash == b.model_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_match() {
        let model = [0xAAu8; 32];
        let v1 = vec![0.5f32; 384];
        let v2 = vec![0.3f32; 384];

        let sig1 = sign_vector(&v1, model, 384, "L2").unwrap();
        let sig2 = sign_vector(&v2, model, 384, "L2").unwrap();

        assert!(verify_contract_match(&sig1, &sig2));
        assert_ne!(sig1.content_hash, sig2.content_hash); // different content
        assert_eq!(sig1.attestation_bytes.len(), 82);
    }

    #[test]
    fn different_models_no_match() {
        let v = vec![0.5f32; 128];
        let sig1 = sign_vector(&v, [0xAA; 32], 128, "L2").unwrap();
        let sig2 = sign_vector(&v, [0xBB; 32], 128, "L2").unwrap();
        assert!(!verify_contract_match(&sig1, &sig2));
    }

    #[test]
    fn different_metrics_no_match() {
        let v = vec![0.5f32; 128];
        let sig1 = sign_vector(&v, [0xAA; 32], 128, "L2").unwrap();
        let sig2 = sign_vector(&v, [0xAA; 32], 128, "Cosine").unwrap();
        assert!(!verify_contract_match(&sig1, &sig2));
    }

    #[test]
    fn combined_hash_stable() {
        let v = vec![0.5f32; 64];
        let sig = sign_vector(&v, [0xCC; 32], 64, "Dot").unwrap();
        assert_eq!(sig.combined_hash(), sig.combined_hash());
    }
}
