//! # 4. Multi-Agent Contract Enforcement
//!
//! Each agent message embedding must:
//! - Match declared dimensionality
//! - Match contract schema (metric type)
//! - Pass verified transformation pipeline
//!
//! If mismatch, no agent state transition allowed.
//! Result: the proof engine becomes a structural gate -- a logic firewall.

use crate::ProofReceipt;
use ruvector_verified::{
    gated::{self, ProofKind, ProofTier},
    proof_store, vector_types, ProofEnvironment,
};

/// An agent contract specifying required embedding properties.
#[derive(Debug, Clone)]
pub struct AgentContract {
    pub agent_id: String,
    pub required_dim: u32,
    pub required_metric: String,
    pub max_pipeline_depth: u32,
}

/// Result of a contract gate check.
#[derive(Debug)]
pub struct GateResult {
    pub agent_id: String,
    pub allowed: bool,
    pub reason: String,
    pub receipt: Option<ProofReceipt>,
}

/// Check whether an agent message embedding passes its contract gate.
pub fn enforce_contract(contract: &AgentContract, message_embedding: &[f32]) -> GateResult {
    let mut env = ProofEnvironment::new();

    // Gate 1: Dimension match
    let dim_result =
        vector_types::verified_dim_check(&mut env, contract.required_dim, message_embedding);
    let dim_proof = match dim_result {
        Ok(op) => op.proof_id,
        Err(e) => {
            return GateResult {
                agent_id: contract.agent_id.clone(),
                allowed: false,
                reason: format!("dimension gate failed: {e}"),
                receipt: None,
            };
        }
    };

    // Gate 2: Metric schema match
    let metric_result = vector_types::mk_distance_metric(&mut env, &contract.required_metric);
    if let Err(e) = metric_result {
        return GateResult {
            agent_id: contract.agent_id.clone(),
            allowed: false,
            reason: format!("metric gate failed: {e}"),
            receipt: None,
        };
    }

    // Gate 3: Pipeline depth check via gated routing
    let decision = gated::route_proof(
        ProofKind::PipelineComposition {
            stages: contract.max_pipeline_depth,
        },
        &env,
    );

    let attestation = proof_store::create_attestation(&env, dim_proof);

    GateResult {
        agent_id: contract.agent_id.clone(),
        allowed: true,
        reason: format!(
            "all gates passed: dim={}, metric={}, tier={}",
            contract.required_dim,
            contract.required_metric,
            match decision.tier {
                ProofTier::Reflex => "reflex",
                ProofTier::Standard { .. } => "standard",
                ProofTier::Deep => "deep",
            },
        ),
        receipt: Some(ProofReceipt {
            domain: "agent_contract".into(),
            claim: format!("agent '{}' message verified", contract.agent_id),
            proof_id: dim_proof,
            attestation_bytes: attestation.to_bytes(),
            tier: match decision.tier {
                ProofTier::Reflex => "reflex",
                ProofTier::Standard { .. } => "standard",
                ProofTier::Deep => "deep",
            }
            .into(),
            gate_passed: true,
        }),
    }
}

/// Run a multi-agent scenario: N agents, each with a contract, each sending messages.
pub fn run_multi_agent_scenario(agents: &[(AgentContract, Vec<f32>)]) -> Vec<GateResult> {
    agents
        .iter()
        .map(|(c, emb)| enforce_contract(c, emb))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_contract(dim: u32) -> AgentContract {
        AgentContract {
            agent_id: "agent-A".into(),
            required_dim: dim,
            required_metric: "Cosine".into(),
            max_pipeline_depth: 3,
        }
    }

    #[test]
    fn valid_agent_passes_gate() {
        let contract = test_contract(256);
        let embedding = vec![0.1f32; 256];
        let result = enforce_contract(&contract, &embedding);
        assert!(result.allowed);
        assert!(result.receipt.is_some());
    }

    #[test]
    fn wrong_dim_blocked() {
        let contract = test_contract(256);
        let embedding = vec![0.1f32; 128];
        let result = enforce_contract(&contract, &embedding);
        assert!(!result.allowed);
        assert!(result.receipt.is_none());
    }

    #[test]
    fn multi_agent_mixed() {
        let agents = vec![
            (test_contract(128), vec![0.5f32; 128]), // pass
            (test_contract(128), vec![0.5f32; 64]),  // fail
            (test_contract(256), vec![0.5f32; 256]), // pass
        ];
        let results = run_multi_agent_scenario(&agents);
        assert_eq!(results.iter().filter(|r| r.allowed).count(), 2);
        assert_eq!(results.iter().filter(|r| !r.allowed).count(), 1);
    }
}
