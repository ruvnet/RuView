//! # 10. Legal Forensics for AI Decisions
//!
//! Court case asks: "Was the AI system malformed?"
//!
//! You produce:
//! - Witness chain (ordered proof attestations)
//! - Proof term replay (re-verify from scratch)
//! - Structural invariants (dimension, metric, pipeline)
//!
//! Result: mathematical evidence, not just logs.

use ruvector_verified::{
    pipeline::compose_chain,
    proof_store::{self, ProofAttestation},
    vector_types, ProofEnvironment, ProofStats,
};

/// A forensic evidence bundle for court submission.
#[derive(Debug)]
pub struct ForensicBundle {
    pub case_id: String,
    pub witness_chain: Vec<ProofAttestation>,
    pub replay_passed: bool,
    pub invariants: ForensicInvariants,
    pub stats: ProofStats,
}

/// Structural invariants extracted from the proof environment.
#[derive(Debug)]
pub struct ForensicInvariants {
    pub declared_dim: u32,
    pub actual_dim: u32,
    pub metric: String,
    pub pipeline_stages: Vec<String>,
    pub pipeline_verified: bool,
    pub total_proof_terms: u32,
}

/// Build a forensic evidence bundle by replaying the full proof chain.
///
/// This re-constructs all proofs from scratch -- if any step fails,
/// the system is malformed.
pub fn build_forensic_bundle(
    case_id: &str,
    vectors: &[&[f32]],
    declared_dim: u32,
    metric: &str,
    pipeline_stages: &[&str],
) -> ForensicBundle {
    let mut env = ProofEnvironment::new();
    let mut witness_chain = Vec::new();
    let mut all_passed = true;

    // Replay 1: Verify all vector dimensions
    for (i, vec) in vectors.iter().enumerate() {
        match vector_types::verified_dim_check(&mut env, declared_dim, vec) {
            Ok(op) => {
                witness_chain.push(proof_store::create_attestation(&env, op.proof_id));
            }
            Err(_) => {
                all_passed = false;
                witness_chain.push(proof_store::create_attestation(&env, 0));
            }
        }
    }

    // Replay 2: Verify metric type
    let metric_ok = vector_types::mk_distance_metric(&mut env, metric).is_ok();
    if !metric_ok {
        all_passed = false;
    }

    // Replay 3: Verify pipeline composition
    let chain: Vec<(String, u32, u32)> = pipeline_stages
        .iter()
        .enumerate()
        .map(|(i, s)| (s.to_string(), i as u32 + 1, i as u32 + 2))
        .collect();
    let pipeline_ok = compose_chain(&chain, &mut env).is_ok();
    if !pipeline_ok {
        all_passed = false;
    }

    let actual_dim = vectors.first().map(|v| v.len() as u32).unwrap_or(0);
    let stats = env.stats().clone();

    ForensicBundle {
        case_id: case_id.into(),
        witness_chain,
        replay_passed: all_passed,
        invariants: ForensicInvariants {
            declared_dim,
            actual_dim,
            metric: metric.into(),
            pipeline_stages: pipeline_stages.iter().map(|s| s.to_string()).collect(),
            pipeline_verified: pipeline_ok,
            total_proof_terms: env.terms_allocated(),
        },
        stats,
    }
}

/// Verify that two forensic bundles agree on structural invariants.
pub fn bundles_structurally_equal(a: &ForensicBundle, b: &ForensicBundle) -> bool {
    a.invariants.declared_dim == b.invariants.declared_dim
        && a.invariants.metric == b.invariants.metric
        && a.invariants.pipeline_stages == b.invariants.pipeline_stages
        && a.replay_passed == b.replay_passed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_system_passes_forensics() {
        let v1 = vec![0.5f32; 256];
        let v2 = vec![0.3f32; 256];
        let vecs: Vec<&[f32]> = vec![&v1, &v2];
        let bundle = build_forensic_bundle(
            "CASE-001",
            &vecs,
            256,
            "Cosine",
            &["embed", "search", "classify"],
        );
        assert!(bundle.replay_passed);
        assert_eq!(bundle.witness_chain.len(), 2);
        assert!(bundle.invariants.pipeline_verified);
        assert_eq!(
            bundle.invariants.total_proof_terms,
            bundle.stats.proofs_constructed as u32
        );
    }

    #[test]
    fn malformed_system_detected() {
        let v1 = vec![0.5f32; 256];
        let v2 = vec![0.3f32; 128]; // wrong dimension
        let vecs: Vec<&[f32]> = vec![&v1, &v2];
        let bundle = build_forensic_bundle("CASE-002", &vecs, 256, "L2", &["embed", "classify"]);
        assert!(!bundle.replay_passed);
    }

    #[test]
    fn two_identical_systems_agree() {
        let v = vec![0.5f32; 64];
        let vecs: Vec<&[f32]> = vec![&v];
        let stages = &["encode", "decode"];
        let b1 = build_forensic_bundle("A", &vecs, 64, "L2", stages);
        let b2 = build_forensic_bundle("B", &vecs, 64, "L2", stages);
        assert!(bundles_structurally_equal(&b1, &b2));
    }

    #[test]
    fn different_metrics_disagree() {
        let v = vec![0.5f32; 64];
        let vecs: Vec<&[f32]> = vec![&v];
        let b1 = build_forensic_bundle("A", &vecs, 64, "L2", &["step"]);
        let b2 = build_forensic_bundle("B", &vecs, 64, "Cosine", &["step"]);
        assert!(!bundles_structurally_equal(&b1, &b2));
    }
}
