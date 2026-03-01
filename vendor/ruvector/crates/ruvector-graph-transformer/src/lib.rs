//! Unified graph transformer with proof-gated mutation substrate.
//!
//! This crate composes existing RuVector crates through proof-gated mutation,
//! providing a unified interface for graph neural network operations with
//! formal verification guarantees.
//!
//! # Modules
//!
//! - [`proof_gated`]: Core proof-gated mutation types
//! - [`sublinear_attention`]: O(n log n) attention via LSH and PPR sampling
//! - [`physics`]: Hamiltonian graph networks with energy conservation proofs
//! - [`biological`]: Spiking attention with STDP and Hebbian learning
//! - [`self_organizing`]: Morphogenetic fields and L-system graph growth
//! - [`verified_training`]: GNN training with per-step proof certificates
//! - [`manifold`]: Product manifold attention on S^n x H^m x R^k
//! - [`temporal`]: Causal temporal attention with Granger causality
//! - [`economic`]: Game-theoretic, Shapley, and incentive-aligned attention
//!
//! # Feature Flags
//!
//! - `sublinear` (default): Sublinear attention mechanisms
//! - `verified-training` (default): Verified training with certificates
//! - `physics`: Hamiltonian graph networks
//! - `biological`: Spiking and Hebbian attention
//! - `self-organizing`: Morphogenetic fields and developmental programs
//! - `manifold`: Product manifold attention
//! - `temporal`: Causal temporal attention
//! - `economic`: Game-theoretic and incentive-aligned attention
//! - `full`: All features enabled

pub mod config;
pub mod error;
pub mod proof_gated;

#[cfg(feature = "sublinear")]
pub mod sublinear_attention;

#[cfg(feature = "physics")]
pub mod physics;

#[cfg(feature = "biological")]
pub mod biological;

#[cfg(feature = "self-organizing")]
pub mod self_organizing;

#[cfg(feature = "verified-training")]
pub mod verified_training;

#[cfg(feature = "manifold")]
pub mod manifold;

#[cfg(feature = "temporal")]
pub mod temporal;

#[cfg(feature = "economic")]
pub mod economic;

// Re-exports
pub use config::GraphTransformerConfig;
pub use error::{GraphTransformerError, Result};
pub use proof_gated::{AttestationChain, ProofGate, ProofGatedMutation};

#[cfg(feature = "sublinear")]
pub use sublinear_attention::SublinearGraphAttention;

#[cfg(feature = "physics")]
pub use physics::{
    ConservativePdeAttention, GaugeEquivariantMP, GaugeOutput, HamiltonianGraphNet,
    HamiltonianOutput, HamiltonianState, LagrangianAttention, LagrangianOutput, PdeOutput,
};

#[cfg(feature = "biological")]
pub use biological::{
    BranchAssignment, DendriticAttention, EffectiveOperator, HebbianLayer, HebbianNormBound,
    HebbianRule, InhibitionStrategy, ScopeTransitionAttestation, SpikingGraphAttention,
    StdpEdgeUpdater,
};

#[cfg(feature = "self-organizing")]
pub use self_organizing::{DevelopmentalProgram, GraphCoarsener, MorphogeneticField};

#[cfg(feature = "verified-training")]
pub use verified_training::{
    EnergyGateResult, InvariantStats, ProofClass, RollbackStrategy, TrainingCertificate,
    TrainingInvariant, TrainingStepResult, VerifiedTrainer,
};

#[cfg(feature = "manifold")]
pub use manifold::{
    CurvatureAdaptiveRouter, GeodesicMessagePassing, LieGroupEquivariantAttention, LieGroupType,
    ManifoldType, ProductManifoldAttention, RiemannianAdamOptimizer,
};

#[cfg(feature = "temporal")]
pub use temporal::{
    AttentionSnapshot, BatchModeToken, CausalGraphTransformer, ContinuousTimeODE, EdgeEventType,
    GrangerCausalityExtractor, GrangerCausalityResult, GrangerEdge, GrangerGraph, MaskStrategy,
    OdeOutput, RetrocausalAttention, SmoothedOutput, StorageTier, TemporalAttentionResult,
    TemporalEdgeEvent, TemporalEmbeddingStore,
};

#[cfg(feature = "economic")]
pub use economic::{GameTheoreticAttention, IncentiveAlignedMPNN, ShapleyAttention};

/// Unified graph transformer entry point.
///
/// Provides a single interface to all graph transformer modules,
/// configured through [`GraphTransformerConfig`].
pub struct GraphTransformer {
    config: GraphTransformerConfig,
}

impl GraphTransformer {
    /// Create a new graph transformer with the given configuration.
    pub fn new(config: GraphTransformerConfig) -> Self {
        Self { config }
    }

    /// Create a graph transformer with default configuration.
    pub fn with_defaults() -> Self {
        Self {
            config: GraphTransformerConfig::default(),
        }
    }

    /// Get the configuration.
    pub fn config(&self) -> &GraphTransformerConfig {
        &self.config
    }

    /// Get the embedding dimension.
    pub fn embed_dim(&self) -> usize {
        self.config.embed_dim
    }

    /// Create a proof gate wrapping a value.
    pub fn create_gate<T>(&self, value: T) -> ProofGate<T> {
        ProofGate::new(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_transformer_creation() {
        let gt = GraphTransformer::with_defaults();
        assert_eq!(gt.embed_dim(), 64);
        assert!(gt.config().proof_gated);
    }

    #[test]
    fn test_graph_transformer_custom_config() {
        let config = GraphTransformerConfig {
            embed_dim: 128,
            num_heads: 8,
            dropout: 0.2,
            proof_gated: false,
            ..Default::default()
        };
        let gt = GraphTransformer::new(config);
        assert_eq!(gt.embed_dim(), 128);
        assert!(!gt.config().proof_gated);
    }

    #[test]
    fn test_create_gate() {
        let gt = GraphTransformer::with_defaults();
        let gate = gt.create_gate(vec![1.0, 2.0, 3.0]);
        assert_eq!(gate.read().len(), 3);
    }
}
