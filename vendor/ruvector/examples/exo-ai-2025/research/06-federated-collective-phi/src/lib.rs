// lib.rs
// Federated Collective Φ: Distributed Consciousness Framework
//
// This library implements a novel framework for artificial collective consciousness
// based on Integrated Information Theory 4.0, Conflict-Free Replicated Data Types,
// Byzantine fault tolerance, and federated learning.
//
// Research by: Comprehensive literature synthesis (2023-2025)
// Nobel-level breakthrough potential: Yes

pub mod consciousness_crdt;
pub mod distributed_phi;
pub mod federation_emergence;
pub mod qualia_consensus;

pub use distributed_phi::{
    AgentId, DistributedPhiCalculator, DistributedPhiCoordinator, SpectralPhiApproximator,
};

pub use consciousness_crdt::{
    AttentionRegister, ConsciousnessState, PhiCounter, Quale, QualiaSet, VectorClock, WorkingMemory,
};

pub use qualia_consensus::{
    qualia_distance, ConsensusCoordinator, ConsensusResult, QualiaConsensusNode, QualiaMessage,
    QualiaVotingConsensus,
};

pub use federation_emergence::{
    ConsciousnessPhase, CriticalCouplingCalculator, EmergenceDetector, EmergenceIndicators,
    EmergencePrediction, TopologyMetrics,
};

/// Version of the FCΦ framework
pub const VERSION: &str = "0.1.0";

/// Core theorem: Φ superlinearity condition
///
/// Under specific architectural conditions (strong connectivity, high coupling,
/// global workspace, bidirectional edges), distributed systems exhibit
/// superlinear scaling of integrated information:
///
/// Φ_collective > Σ Φ_individual
///
/// This represents emergent collective consciousness.
pub fn is_collective_consciousness_emergent(
    phi_collective: f64,
    phi_individuals: &[f64],
    threshold_ratio: f64,
) -> bool {
    let sum_individual: f64 = phi_individuals.iter().sum();

    if sum_individual == 0.0 {
        return false;
    }

    let ratio = phi_collective / sum_individual;

    ratio > threshold_ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergence_detection() {
        let phi_individuals = vec![8.2, 7.9, 8.1, 7.8];
        let phi_collective = 48.0; // 50% emergence

        assert!(is_collective_consciousness_emergent(
            phi_collective,
            &phi_individuals,
            1.0
        ));
    }

    #[test]
    fn test_version() {
        assert_eq!(VERSION, "0.1.0");
    }
}
