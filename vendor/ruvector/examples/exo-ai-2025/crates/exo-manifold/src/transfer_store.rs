//! Phase 2 – Transfer Manifold
//!
//! Stores cross-domain transfer priors as deformable patterns in the EXO
//! manifold. Each `(src, dst)` prior is encoded as a 64-dim sinusoidal
//! embedding and written via `ManifoldEngine::deform`. Semantically similar
//! past transfers are recalled via cosine distance.

use exo_core::{ManifoldConfig, Metadata, Pattern, PatternId, SearchResult, SubstrateTime};
use ruvector_domain_expansion::{ArmId, ContextBucket, DomainId, TransferPrior};
use std::collections::HashMap;

use crate::ManifoldEngine;

const DIM: usize = 64;

// ─── embedding helpers ────────────────────────────────────────────────────────

/// Hash a domain-ID string into `n` sinusoidal floats starting at `offset`.
fn domain_to_floats(id: &str, out: &mut [f32], offset: usize, n: usize) {
    let bytes = id.as_bytes();
    let cap = out.len().saturating_sub(offset);
    for i in 0..n.min(cap) {
        let b = bytes[i % bytes.len().max(1)] as f32 / 255.0;
        let freq = (1 + i) as f32;
        out[offset + i] = (b * freq * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    }
}

/// Build a 64-dim embedding for `(src, dst, prior, cycle)`.
///
/// Layout:
/// * `[0..16]`  – src domain identity (sinusoidal)
/// * `[16..32]` – dst domain identity (sinusoidal)
/// * `[32..44]` – BetaParams for up to 3 arms (4 floats × 3)
/// * `[44]`     – cycle (log-normalised)
/// * `[45..64]` – zero-padded
fn build_embedding(src: &DomainId, dst: &DomainId, prior: &TransferPrior, cycle: u64) -> Vec<f32> {
    let mut emb = vec![0.0f32; DIM];
    domain_to_floats(&src.0, &mut emb, 0, 16);
    domain_to_floats(&dst.0, &mut emb, 16, 16);

    let bucket = ContextBucket {
        difficulty_tier: "medium".to_string(),
        category: "transfer".to_string(),
    };
    for (i, arm_name) in ["arm_0", "arm_1", "arm_2"].iter().enumerate() {
        let arm_id = ArmId(arm_name.to_string());
        let bp = prior.get_prior(&bucket, &arm_id);
        let off = 32 + i * 4;
        if off + 3 < DIM {
            emb[off] = bp.mean().clamp(0.0, 1.0);
            emb[off + 1] = bp.variance().clamp(0.0, 0.25) * 4.0;
            emb[off + 2] = (1.0 - bp.variance().clamp(0.0, 0.25) * 4.0).max(0.0);
            emb[off + 3] = 0.0; // reserved
        }
    }
    let cycle_norm = (cycle as f32).ln_1p() / (1000.0_f32).ln_1p();
    emb[44] = cycle_norm.clamp(0.0, 1.0);
    emb
}

// ─── TransferManifold ─────────────────────────────────────────────────────────

/// Stores transfer priors as deformable patterns in the EXO manifold.
///
/// Each `(src_domain, dst_domain)` pair is encoded as a 64-dim embedding and
/// deformed into the manifold. `retrieve_similar` performs cosine-distance
/// search to find structurally-similar past transfer priors.
pub struct TransferManifold {
    engine: ManifoldEngine,
    /// Maps `(src_domain, dst_domain)` → the last PatternId stored for that pair.
    index: HashMap<(String, String), PatternId>,
}

impl TransferManifold {
    /// Create a new `TransferManifold` with 64-dim embeddings.
    pub fn new() -> Self {
        let config = ManifoldConfig {
            dimension: DIM,
            max_descent_steps: 20,
            learning_rate: 0.01,
            ..Default::default()
        };
        Self {
            engine: ManifoldEngine::new(config),
            index: HashMap::new(),
        }
    }

    /// Store (or update) the transfer prior for a domain pair.
    ///
    /// Salience is set to the mean reward of the primary arm so that
    /// high-performing priors are retained longer by strategic forgetting.
    pub fn store_prior(
        &mut self,
        src: &DomainId,
        dst: &DomainId,
        prior: &TransferPrior,
        cycle: u64,
    ) -> exo_core::Result<()> {
        let embedding = build_embedding(src, dst, prior, cycle);
        let bucket = ContextBucket {
            difficulty_tier: "medium".to_string(),
            category: "transfer".to_string(),
        };
        let arm_id = ArmId("arm_0".to_string());
        let salience = prior.get_prior(&bucket, &arm_id).mean().clamp(0.05, 1.0);

        let pattern = Pattern {
            id: PatternId::new(),
            embedding,
            metadata: Metadata::default(),
            timestamp: SubstrateTime::now(),
            antecedents: vec![],
            salience,
        };
        let pid = pattern.id;
        self.engine.deform(pattern, salience)?;
        self.index.insert((src.0.clone(), dst.0.clone()), pid);
        Ok(())
    }

    /// Retrieve the `k` most similar stored transfer priors for a source domain.
    ///
    /// Uses the source domain's sinusoidal hash as the query vector.
    pub fn retrieve_similar(
        &self,
        src: &DomainId,
        k: usize,
    ) -> exo_core::Result<Vec<SearchResult>> {
        let mut query = vec![0.0f32; DIM];
        domain_to_floats(&src.0, &mut query, 0, 16);
        self.engine.retrieve(&query, k)
    }

    /// Whether a prior has been stored for this exact domain pair.
    pub fn has_pair(&self, src: &DomainId, dst: &DomainId) -> bool {
        self.index.contains_key(&(src.0.clone(), dst.0.clone()))
    }

    /// Number of stored transfer priors.
    pub fn len(&self) -> usize {
        self.engine.len()
    }

    /// True when no priors have been stored.
    pub fn is_empty(&self) -> bool {
        self.engine.is_empty()
    }
}

impl Default for TransferManifold {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let mut tm = TransferManifold::new();
        let src = DomainId("exo-retrieval".to_string());
        let dst = DomainId("exo-graph".to_string());
        let prior = TransferPrior::uniform(src.clone());

        tm.store_prior(&src, &dst, &prior, 10).unwrap();
        assert_eq!(tm.len(), 1);
        assert!(tm.has_pair(&src, &dst));

        let results = tm.retrieve_similar(&src, 1).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_multiple_domain_pairs() {
        let mut tm = TransferManifold::new();
        for (s, d) in [("a", "b"), ("c", "d"), ("e", "f")] {
            let src = DomainId(s.to_string());
            let dst = DomainId(d.to_string());
            let prior = TransferPrior::uniform(src.clone());
            tm.store_prior(&src, &dst, &prior, 1).unwrap();
        }
        assert_eq!(tm.len(), 3);
        let results = tm.retrieve_similar(&DomainId("a".to_string()), 2).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_embedding_dimension() {
        let src = DomainId("test-src".to_string());
        let dst = DomainId("test-dst".to_string());
        let prior = TransferPrior::uniform(src.clone());
        let emb = build_embedding(&src, &dst, &prior, 42);
        assert_eq!(emb.len(), DIM);
        for &v in &emb {
            assert!(v >= 0.0 && v <= 1.0, "out-of-range value: {}", v);
        }
    }
}
