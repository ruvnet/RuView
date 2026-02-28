//! # 5. Distributed Sensor Swarms with Verifiable Consensus
//!
//! In a sensor swarm:
//! - Each node embeds sensor data
//! - Proves dimensional invariants
//! - Emits a witness fragment
//! - Fragments aggregate into a coherence chain
//!
//! If a node drifts, its proofs diverge. That divergence becomes the
//! coherence signal -- structural integrity across distributed nodes.

use ruvector_verified::{
    proof_store::{self, ProofAttestation},
    vector_types, ProofEnvironment,
};

/// A sensor node's contribution to the swarm.
#[derive(Debug, Clone)]
pub struct SensorWitness {
    pub node_id: String,
    pub verified: bool,
    pub proof_id: u32,
    pub attestation: ProofAttestation,
}

/// Aggregated coherence check across all swarm nodes.
#[derive(Debug)]
pub struct SwarmCoherence {
    pub total_nodes: usize,
    pub verified_nodes: usize,
    pub divergent_nodes: Vec<String>,
    pub coherent: bool,
    pub attestations: Vec<ProofAttestation>,
}

/// Verify a single sensor node's embedding against the swarm contract.
pub fn verify_sensor_node(node_id: &str, reading: &[f32], expected_dim: u32) -> SensorWitness {
    let mut env = ProofEnvironment::new();
    match vector_types::verified_dim_check(&mut env, expected_dim, reading) {
        Ok(op) => {
            let att = proof_store::create_attestation(&env, op.proof_id);
            SensorWitness {
                node_id: node_id.into(),
                verified: true,
                proof_id: op.proof_id,
                attestation: att,
            }
        }
        Err(_) => {
            let att = proof_store::create_attestation(&env, 0);
            SensorWitness {
                node_id: node_id.into(),
                verified: false,
                proof_id: 0,
                attestation: att,
            }
        }
    }
}

/// Run swarm-wide coherence check. All nodes must produce valid proofs.
pub fn check_swarm_coherence(nodes: &[(&str, &[f32])], expected_dim: u32) -> SwarmCoherence {
    let witnesses: Vec<SensorWitness> = nodes
        .iter()
        .map(|(id, data)| verify_sensor_node(id, data, expected_dim))
        .collect();

    let verified = witnesses.iter().filter(|w| w.verified).count();
    let divergent: Vec<String> = witnesses
        .iter()
        .filter(|w| !w.verified)
        .map(|w| w.node_id.clone())
        .collect();

    SwarmCoherence {
        total_nodes: nodes.len(),
        verified_nodes: verified,
        divergent_nodes: divergent.clone(),
        coherent: divergent.is_empty(),
        attestations: witnesses.into_iter().map(|w| w.attestation).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_nodes_coherent() {
        let nodes: Vec<(&str, Vec<f32>)> = (0..5)
            .map(|i| (["n0", "n1", "n2", "n3", "n4"][i], vec![0.5f32; 64]))
            .collect();
        let refs: Vec<(&str, &[f32])> = nodes.iter().map(|(id, d)| (*id, d.as_slice())).collect();
        let result = check_swarm_coherence(&refs, 64);
        assert!(result.coherent);
        assert_eq!(result.verified_nodes, 5);
        assert!(result.divergent_nodes.is_empty());
    }

    #[test]
    fn drifted_node_detected() {
        let good = vec![0.5f32; 64];
        let bad = vec![0.5f32; 32]; // drifted
        let nodes: Vec<(&str, &[f32])> =
            vec![("n0", &good), ("n1", &good), ("n2", &bad), ("n3", &good)];
        let result = check_swarm_coherence(&nodes, 64);
        assert!(!result.coherent);
        assert_eq!(result.divergent_nodes, vec!["n2"]);
        assert_eq!(result.verified_nodes, 3);
    }

    #[test]
    fn attestation_per_node() {
        let data = vec![0.5f32; 128];
        let nodes: Vec<(&str, &[f32])> = vec![("a", &data), ("b", &data)];
        let result = check_swarm_coherence(&nodes, 128);
        assert_eq!(result.attestations.len(), 2);
        assert!(result.attestations.iter().all(|a| a.to_bytes().len() == 82));
    }
}
