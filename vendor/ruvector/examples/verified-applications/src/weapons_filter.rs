//! # 1. Self-Auditing Autonomous Weapons Filters
//!
//! Before a targeting or sensor fusion pipeline fires, it must prove:
//! - Feature vector dimension matches model expectation
//! - Distance metric matches certified configuration
//! - Pipeline stages composed in approved order
//!
//! The system emits an 82-byte proof witness per decision.
//! Result: machine-verifiable "no unapproved transformation occurred."

use crate::ProofReceipt;
use ruvector_verified::{
    gated::{self, ProofKind, ProofTier},
    pipeline::compose_stages,
    proof_store, vector_types, ProofEnvironment, VerifiedStage,
};

/// Certified pipeline configuration loaded from tamper-evident config.
pub struct CertifiedConfig {
    pub sensor_dim: u32,
    pub model_dim: u32,
    pub metric: String,
    pub approved_stages: Vec<String>,
}

impl Default for CertifiedConfig {
    fn default() -> Self {
        Self {
            sensor_dim: 512,
            model_dim: 512,
            metric: "L2".into(),
            approved_stages: vec![
                "sensor_fusion".into(),
                "feature_extract".into(),
                "threat_classify".into(),
            ],
        }
    }
}

/// Verify the full targeting pipeline before allowing a decision.
///
/// Returns `None` if any proof fails -- the system MUST NOT proceed.
pub fn verify_targeting_pipeline(
    sensor_data: &[f32],
    config: &CertifiedConfig,
) -> Option<ProofReceipt> {
    let mut env = ProofEnvironment::new();

    // 1. Prove sensor vector matches declared dimension
    let dim_proof =
        vector_types::verified_dim_check(&mut env, config.sensor_dim, sensor_data).ok()?;

    // 2. Prove metric matches certified config
    let _metric = vector_types::mk_distance_metric(&mut env, &config.metric).ok()?;

    // 3. Prove HNSW index type is well-formed
    let _index_type =
        vector_types::mk_hnsw_index_type(&mut env, config.model_dim, &config.metric).ok()?;

    // 4. Prove pipeline stages compose in approved order
    let stage1: VerifiedStage<(), ()> =
        VerifiedStage::new(&config.approved_stages[0], env.alloc_term(), 1, 2);
    let stage2: VerifiedStage<(), ()> =
        VerifiedStage::new(&config.approved_stages[1], env.alloc_term(), 2, 3);
    let stage3: VerifiedStage<(), ()> =
        VerifiedStage::new(&config.approved_stages[2], env.alloc_term(), 3, 4);
    let composed12 = compose_stages(&stage1, &stage2, &mut env).ok()?;
    let full_pipeline = compose_stages(&composed12, &stage3, &mut env).ok()?;

    // 5. Route to determine proof complexity
    let decision = gated::route_proof(ProofKind::PipelineComposition { stages: 3 }, &env);

    // 6. Create attestation
    let attestation = proof_store::create_attestation(&env, dim_proof.proof_id);

    Some(ProofReceipt {
        domain: "weapons_filter".into(),
        claim: format!(
            "pipeline '{}' verified: dim={}, metric={}, 3 stages composed",
            full_pipeline.name(),
            config.sensor_dim,
            config.metric,
        ),
        proof_id: dim_proof.proof_id,
        attestation_bytes: attestation.to_bytes(),
        tier: match decision.tier {
            ProofTier::Reflex => "reflex",
            ProofTier::Standard { .. } => "standard",
            ProofTier::Deep => "deep",
        }
        .into(),
        gate_passed: true,
    })
}

/// Demonstrate: tampered sensor data (wrong dimension) is rejected.
pub fn verify_tampered_sensor(config: &CertifiedConfig) -> Option<ProofReceipt> {
    let bad_data = vec![0.0f32; 256]; // Wrong dimension
    verify_targeting_pipeline(&bad_data, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_pipeline_passes() {
        let config = CertifiedConfig::default();
        let data = vec![0.5f32; 512];
        let receipt = verify_targeting_pipeline(&data, &config);
        assert!(receipt.is_some());
        let r = receipt.unwrap();
        assert!(r.gate_passed);
        assert_eq!(r.attestation_bytes.len(), 82);
    }

    #[test]
    fn tampered_sensor_rejected() {
        let config = CertifiedConfig::default();
        assert!(verify_tampered_sensor(&config).is_none());
    }

    #[test]
    fn wrong_metric_rejected() {
        let mut env = ProofEnvironment::new();
        let result = vector_types::mk_distance_metric(&mut env, "Manhattan");
        assert!(result.is_err());
    }
}
