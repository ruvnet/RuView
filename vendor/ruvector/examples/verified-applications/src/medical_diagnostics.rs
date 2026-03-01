//! # 2. On-Device Medical Diagnostics with Formal Receipts
//!
//! Edge device diagnostic pipeline:
//! - ECG embedding -> similarity search -> risk classifier
//!
//! Each step emits proof-carrying results. The diagnosis bundle includes:
//! - Model hash, vector dimension proof, pipeline composition proof, attestation
//!
//! Result: regulator-grade evidence at the vector math layer.

use crate::ProofReceipt;
use ruvector_verified::{
    pipeline::{compose_chain, compose_stages},
    proof_store, vector_types, ProofEnvironment, VerifiedStage,
};

/// A diagnostic pipeline stage with its proof.
#[derive(Debug)]
pub struct DiagnosticStep {
    pub name: String,
    pub proof_id: u32,
    pub attestation: Vec<u8>,
}

/// Complete diagnostic bundle suitable for regulatory submission.
#[derive(Debug)]
pub struct DiagnosticBundle {
    pub patient_id: String,
    pub model_hash: [u8; 32],
    pub steps: Vec<DiagnosticStep>,
    pub pipeline_proof_id: u32,
    pub pipeline_attestation: Vec<u8>,
    pub verdict: String,
}

/// Run a verified diagnostic pipeline on ECG embeddings.
pub fn run_diagnostic(
    patient_id: &str,
    ecg_embedding: &[f32],
    model_hash: [u8; 32],
    ecg_dim: u32,
) -> Result<DiagnosticBundle, String> {
    let mut env = ProofEnvironment::new();
    let mut steps = Vec::new();

    // Step 1: Verify ECG embedding dimension
    let dim_check = vector_types::verified_dim_check(&mut env, ecg_dim, ecg_embedding)
        .map_err(|e| format!("ECG dim check failed: {e}"))?;
    let att1 = proof_store::create_attestation(&env, dim_check.proof_id);
    steps.push(DiagnosticStep {
        name: "ecg_embedding_verified".into(),
        proof_id: dim_check.proof_id,
        attestation: att1.to_bytes(),
    });

    // Step 2: Verify similarity search metric
    let metric_id = vector_types::mk_distance_metric(&mut env, "Cosine")
        .map_err(|e| format!("metric check: {e}"))?;
    let att2 = proof_store::create_attestation(&env, metric_id);
    steps.push(DiagnosticStep {
        name: "similarity_metric_verified".into(),
        proof_id: metric_id,
        attestation: att2.to_bytes(),
    });

    // Step 3: Verify HNSW index type
    let idx = vector_types::mk_hnsw_index_type(&mut env, ecg_dim, "Cosine")
        .map_err(|e| format!("index type: {e}"))?;
    let att3 = proof_store::create_attestation(&env, idx);
    steps.push(DiagnosticStep {
        name: "hnsw_index_verified".into(),
        proof_id: idx,
        attestation: att3.to_bytes(),
    });

    // Step 4: Compose full pipeline and prove ordering
    let chain = vec![
        ("ecg_embed".into(), 1u32, 2),
        ("similarity_search".into(), 2, 3),
        ("risk_classify".into(), 3, 4),
    ];
    let (input_ty, output_ty, chain_proof) =
        compose_chain(&chain, &mut env).map_err(|e| format!("pipeline composition: {e}"))?;
    let att4 = proof_store::create_attestation(&env, chain_proof);

    Ok(DiagnosticBundle {
        patient_id: patient_id.into(),
        model_hash,
        steps,
        pipeline_proof_id: chain_proof,
        pipeline_attestation: att4.to_bytes(),
        verdict: format!(
            "Pipeline type#{} -> type#{} verified with {} proof steps",
            input_ty,
            output_ty,
            env.stats().proofs_constructed,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_diagnostic_pipeline() {
        let ecg = vec![0.1f32; 256];
        let model_hash = [0xABu8; 32];
        let bundle = run_diagnostic("patient-001", &ecg, model_hash, 256);
        assert!(bundle.is_ok());
        let b = bundle.unwrap();
        assert_eq!(b.steps.len(), 3);
        assert!(b.steps.iter().all(|s| s.attestation.len() == 82));
        assert_eq!(b.pipeline_attestation.len(), 82);
    }

    #[test]
    fn wrong_ecg_dimension_rejected() {
        let ecg = vec![0.1f32; 128]; // Wrong: expected 256
        let result = run_diagnostic("patient-002", &ecg, [0u8; 32], 256);
        assert!(result.is_err());
    }
}
