//! # 3. Financial Order Routing Integrity
//!
//! Before routing a trade decision:
//! - Prove feature vector dimension matches model
//! - Prove metric compatibility (L2 for risk, Cosine for similarity)
//! - Prove risk scoring pipeline composition
//!
//! Store proof hash with trade ID. Replay the proof term if questioned later.
//! Result: the feature pipeline itself was mathematically coherent.

use crate::ProofReceipt;
use ruvector_verified::{
    gated::{self, ProofKind, ProofTier},
    pipeline::compose_chain,
    proof_store, vector_types, ProofEnvironment,
};

/// A trade order with its verified proof chain.
#[derive(Debug)]
pub struct VerifiedTradeOrder {
    pub trade_id: String,
    pub direction: String,
    pub feature_dim: u32,
    pub risk_score_proof: u32,
    pub pipeline_proof: u32,
    pub attestation: Vec<u8>,
    pub proof_hash: u64,
}

/// Verify and emit proof for a trade order routing decision.
pub fn verify_trade_order(
    trade_id: &str,
    feature_vector: &[f32],
    feature_dim: u32,
    risk_metric: &str,
    direction: &str,
) -> Result<VerifiedTradeOrder, String> {
    let mut env = ProofEnvironment::new();

    // 1. Feature dimension proof
    let dim_check = vector_types::verified_dim_check(&mut env, feature_dim, feature_vector)
        .map_err(|e| format!("feature dim: {e}"))?;

    // 2. Risk metric proof
    let _metric = vector_types::mk_distance_metric(&mut env, risk_metric)
        .map_err(|e| format!("metric: {e}"))?;

    // 3. Index type proof
    let _index = vector_types::mk_hnsw_index_type(&mut env, feature_dim, risk_metric)
        .map_err(|e| format!("index: {e}"))?;

    // 4. Pipeline: feature_extract -> risk_score -> order_route
    let chain = vec![
        ("feature_extract".into(), 10u32, 11),
        ("risk_score".into(), 11, 12),
        ("order_route".into(), 12, 13),
    ];
    let (_in_ty, _out_ty, pipeline_proof) =
        compose_chain(&chain, &mut env).map_err(|e| format!("pipeline: {e}"))?;

    // 5. Route proof to appropriate tier
    let _decision = gated::route_proof(ProofKind::PipelineComposition { stages: 3 }, &env);

    // 6. Create attestation and compute hash for storage
    let attestation = proof_store::create_attestation(&env, pipeline_proof);
    let proof_hash = attestation.content_hash();

    Ok(VerifiedTradeOrder {
        trade_id: trade_id.into(),
        direction: direction.into(),
        feature_dim,
        risk_score_proof: dim_check.proof_id,
        pipeline_proof,
        attestation: attestation.to_bytes(),
        proof_hash,
    })
}

/// Verify a batch of trade orders and return pass/fail counts.
pub fn verify_trade_batch(
    orders: &[(&str, &[f32], u32)], // (trade_id, features, dim)
) -> (usize, usize) {
    let mut passed = 0;
    let mut failed = 0;
    for (id, features, dim) in orders {
        match verify_trade_order(id, features, *dim, "L2", "BUY") {
            Ok(_) => passed += 1,
            Err(_) => failed += 1,
        }
    }
    (passed, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_trade_verified() {
        let features = vec![0.3f32; 128];
        let order = verify_trade_order("TRD-001", &features, 128, "L2", "BUY");
        assert!(order.is_ok());
        let o = order.unwrap();
        assert_eq!(o.attestation.len(), 82);
        assert_ne!(o.proof_hash, 0);
    }

    #[test]
    fn wrong_dimension_blocks_trade() {
        let features = vec![0.3f32; 64]; // Wrong
        let result = verify_trade_order("TRD-002", &features, 128, "L2", "SELL");
        assert!(result.is_err());
    }

    #[test]
    fn batch_mixed_results() {
        let good = vec![0.5f32; 128];
        let bad = vec![0.5f32; 64];
        let orders: Vec<(&str, &[f32], u32)> =
            vec![("T1", &good, 128), ("T2", &bad, 128), ("T3", &good, 128)];
        let (pass, fail) = verify_trade_batch(&orders);
        assert_eq!(pass, 2);
        assert_eq!(fail, 1);
    }
}
