//! # 9. Simulation Integrity (FXNN / ruQu)
//!
//! When running molecular or quantum embeddings:
//! - Prove tensor shapes match
//! - Prove pipeline consistency
//! - Emit proof receipt per simulation step
//!
//! Result: reproducible physics at the embedding layer.

use ruvector_verified::{pipeline::compose_chain, proof_store, vector_types, ProofEnvironment};

/// A simulation step with its proof.
#[derive(Debug)]
pub struct SimulationStep {
    pub step_id: u32,
    pub tensor_dim: u32,
    pub proof_id: u32,
    pub attestation: Vec<u8>,
}

/// Full simulation run with verified step chain.
#[derive(Debug)]
pub struct VerifiedSimulation {
    pub simulation_id: String,
    pub steps: Vec<SimulationStep>,
    pub pipeline_proof: u32,
    pub pipeline_attestation: Vec<u8>,
    pub total_proofs: u64,
}

/// Run a verified simulation: each step's tensor must match declared dimension.
pub fn run_verified_simulation(
    sim_id: &str,
    step_tensors: &[Vec<f32>],
    tensor_dim: u32,
    pipeline_stages: &[&str],
) -> Result<VerifiedSimulation, String> {
    let mut env = ProofEnvironment::new();
    let mut steps = Vec::new();

    // Verify each simulation step's tensor
    for (i, tensor) in step_tensors.iter().enumerate() {
        let check = vector_types::verified_dim_check(&mut env, tensor_dim, tensor)
            .map_err(|e| format!("step {i}: {e}"))?;
        let att = proof_store::create_attestation(&env, check.proof_id);
        steps.push(SimulationStep {
            step_id: i as u32,
            tensor_dim,
            proof_id: check.proof_id,
            attestation: att.to_bytes(),
        });
    }

    // Compose pipeline stages
    let chain: Vec<(String, u32, u32)> = pipeline_stages
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), i as u32 + 1, i as u32 + 2))
        .collect();

    let (_in_ty, _out_ty, pipeline_proof) =
        compose_chain(&chain, &mut env).map_err(|e| format!("pipeline: {e}"))?;
    let att = proof_store::create_attestation(&env, pipeline_proof);

    Ok(VerifiedSimulation {
        simulation_id: sim_id.into(),
        steps,
        pipeline_proof,
        pipeline_attestation: att.to_bytes(),
        total_proofs: env.stats().proofs_constructed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_simulation() {
        let tensors: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 64]).collect();
        let stages = &["hamiltonian", "evolve", "measure"];
        let sim = run_verified_simulation("sim-001", &tensors, 64, stages);
        assert!(sim.is_ok());
        let s = sim.unwrap();
        assert_eq!(s.steps.len(), 10);
        assert!(s.steps.iter().all(|st| st.attestation.len() == 82));
        assert_eq!(s.pipeline_attestation.len(), 82);
    }

    #[test]
    fn corrupted_step_detected() {
        let mut tensors: Vec<Vec<f32>> = (0..5).map(|_| vec![0.5f32; 64]).collect();
        tensors[3] = vec![0.5f32; 32]; // corrupted
        let stages = &["init", "evolve"];
        let result = run_verified_simulation("sim-002", &tensors, 64, stages);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("step 3"));
    }

    #[test]
    fn proof_count_scales() {
        let tensors: Vec<Vec<f32>> = (0..100).map(|_| vec![0.1f32; 16]).collect();
        let stages = &["encode", "transform", "decode"];
        let sim = run_verified_simulation("sim-003", &tensors, 16, stages).unwrap();
        assert!(
            sim.total_proofs >= 4,
            "expected >=4 proofs, got {}",
            sim.total_proofs
        );
    }
}
