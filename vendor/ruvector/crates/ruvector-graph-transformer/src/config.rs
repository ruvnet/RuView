//! Configuration types for all graph transformer modules.

use serde::{Deserialize, Serialize};

/// Top-level configuration for the graph transformer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphTransformerConfig {
    /// Embedding dimension for node features.
    pub embed_dim: usize,
    /// Number of attention heads.
    pub num_heads: usize,
    /// Dropout rate (0.0 to 1.0).
    pub dropout: f32,
    /// Whether to enable proof gating on all mutations.
    pub proof_gated: bool,
    /// Sublinear attention configuration.
    #[cfg(feature = "sublinear")]
    pub sublinear: SublinearConfig,
    /// Physics module configuration.
    #[cfg(feature = "physics")]
    pub physics: PhysicsConfig,
    /// Biological module configuration.
    #[cfg(feature = "biological")]
    pub biological: BiologicalConfig,
    /// Self-organizing module configuration.
    #[cfg(feature = "self-organizing")]
    pub self_organizing: SelfOrganizingConfig,
    /// Verified training configuration.
    #[cfg(feature = "verified-training")]
    pub verified_training: VerifiedTrainingConfig,
    /// Manifold module configuration.
    #[cfg(feature = "manifold")]
    pub manifold: ManifoldConfig,
    /// Temporal module configuration.
    #[cfg(feature = "temporal")]
    pub temporal: TemporalConfig,
    /// Economic module configuration.
    #[cfg(feature = "economic")]
    pub economic: EconomicConfig,
}

impl Default for GraphTransformerConfig {
    fn default() -> Self {
        Self {
            embed_dim: 64,
            num_heads: 4,
            dropout: 0.1,
            proof_gated: true,
            #[cfg(feature = "sublinear")]
            sublinear: SublinearConfig::default(),
            #[cfg(feature = "physics")]
            physics: PhysicsConfig::default(),
            #[cfg(feature = "biological")]
            biological: BiologicalConfig::default(),
            #[cfg(feature = "self-organizing")]
            self_organizing: SelfOrganizingConfig::default(),
            #[cfg(feature = "verified-training")]
            verified_training: VerifiedTrainingConfig::default(),
            #[cfg(feature = "manifold")]
            manifold: ManifoldConfig::default(),
            #[cfg(feature = "temporal")]
            temporal: TemporalConfig::default(),
            #[cfg(feature = "economic")]
            economic: EconomicConfig::default(),
        }
    }
}

/// Configuration for sublinear attention.
#[cfg(feature = "sublinear")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SublinearConfig {
    /// Number of LSH buckets for locality-sensitive hashing.
    pub lsh_buckets: usize,
    /// Number of PPR random walk samples.
    pub ppr_samples: usize,
    /// Spectral sparsification factor (0.0 to 1.0).
    pub sparsification_factor: f32,
}

#[cfg(feature = "sublinear")]
impl Default for SublinearConfig {
    fn default() -> Self {
        Self {
            lsh_buckets: 16,
            ppr_samples: 32,
            sparsification_factor: 0.5,
        }
    }
}

/// Configuration for Hamiltonian graph networks.
#[cfg(feature = "physics")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsConfig {
    /// Time step for symplectic integration.
    pub dt: f32,
    /// Number of leapfrog steps per update.
    pub leapfrog_steps: usize,
    /// Energy conservation tolerance.
    pub energy_tolerance: f32,
}

#[cfg(feature = "physics")]
impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            dt: 0.01,
            leapfrog_steps: 10,
            energy_tolerance: 1e-4,
        }
    }
}

/// Configuration for biological graph attention.
#[cfg(feature = "biological")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BiologicalConfig {
    /// Membrane time constant for LIF neurons.
    pub tau_membrane: f32,
    /// Spike threshold voltage.
    pub threshold: f32,
    /// STDP learning rate.
    pub stdp_rate: f32,
    /// Maximum weight bound for stability proofs.
    pub max_weight: f32,
}

#[cfg(feature = "biological")]
impl Default for BiologicalConfig {
    fn default() -> Self {
        Self {
            tau_membrane: 20.0,
            threshold: 1.0,
            stdp_rate: 0.01,
            max_weight: 5.0,
        }
    }
}

/// Configuration for self-organizing modules.
#[cfg(feature = "self-organizing")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelfOrganizingConfig {
    /// Diffusion rate for morphogenetic fields.
    pub diffusion_rate: f32,
    /// Reaction rate for Turing patterns.
    pub reaction_rate: f32,
    /// Maximum growth steps in developmental programs.
    pub max_growth_steps: usize,
    /// Coherence threshold for topology maintenance.
    pub coherence_threshold: f32,
}

#[cfg(feature = "self-organizing")]
impl Default for SelfOrganizingConfig {
    fn default() -> Self {
        Self {
            diffusion_rate: 0.1,
            reaction_rate: 0.05,
            max_growth_steps: 100,
            coherence_threshold: 0.8,
        }
    }
}

/// Configuration for verified training (ADR-049 hardened).
#[cfg(feature = "verified-training")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedTrainingConfig {
    /// Maximum Lipschitz constant for weight updates.
    pub lipschitz_bound: f32,
    /// Whether to verify loss monotonicity at each step (legacy; prefer LossStabilityBound).
    pub verify_monotonicity: bool,
    /// Learning rate for training.
    pub learning_rate: f32,
    /// Whether the trainer operates in fail-closed mode (default: true).
    /// When true, invariant violations reject the step and discard the delta.
    /// When false, violations are logged but the step proceeds (degraded mode).
    pub fail_closed: bool,
    /// Warmup steps during which invariant violations are logged but
    /// do not trigger rollback. After warmup, the fail_closed policy applies.
    pub warmup_steps: u64,
    /// Optional dataset manifest hash for certificate binding.
    pub dataset_manifest_hash: Option<[u8; 32]>,
    /// Optional code/build hash for certificate binding.
    pub code_build_hash: Option<[u8; 32]>,
}

#[cfg(feature = "verified-training")]
impl Default for VerifiedTrainingConfig {
    fn default() -> Self {
        Self {
            lipschitz_bound: 10.0,
            verify_monotonicity: true,
            learning_rate: 0.001,
            fail_closed: true,
            warmup_steps: 0,
            dataset_manifest_hash: None,
            code_build_hash: None,
        }
    }
}

/// Configuration for product manifold attention.
#[cfg(feature = "manifold")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifoldConfig {
    /// Dimension of the spherical component S^n.
    pub spherical_dim: usize,
    /// Dimension of the hyperbolic component H^m.
    pub hyperbolic_dim: usize,
    /// Dimension of the Euclidean component R^k.
    pub euclidean_dim: usize,
    /// Curvature for the hyperbolic component (negative).
    pub curvature: f32,
}

#[cfg(feature = "manifold")]
impl Default for ManifoldConfig {
    fn default() -> Self {
        Self {
            spherical_dim: 16,
            hyperbolic_dim: 16,
            euclidean_dim: 16,
            curvature: -1.0,
        }
    }
}

/// Configuration for causal temporal attention.
#[cfg(feature = "temporal")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalConfig {
    /// Decay rate for temporal attention weights.
    pub decay_rate: f32,
    /// Maximum time lag for causal masking.
    pub max_lag: usize,
    /// Number of Granger causality lags to test.
    pub granger_lags: usize,
}

#[cfg(feature = "temporal")]
impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            decay_rate: 0.9,
            max_lag: 10,
            granger_lags: 5,
        }
    }
}

/// Configuration for economic graph attention mechanisms.
#[cfg(feature = "economic")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EconomicConfig {
    /// Utility weight for game-theoretic attention.
    pub utility_weight: f32,
    /// Temperature for softmax in Nash equilibrium computation.
    pub temperature: f32,
    /// Convergence threshold for iterated best response.
    pub convergence_threshold: f32,
    /// Maximum iterations for Nash equilibrium.
    pub max_iterations: usize,
    /// Minimum stake for incentive-aligned MPNN.
    pub min_stake: f32,
    /// Fraction of stake slashed on misbehavior.
    pub slash_fraction: f32,
    /// Number of permutation samples for Shapley value estimation.
    pub num_permutations: usize,
}

#[cfg(feature = "economic")]
impl Default for EconomicConfig {
    fn default() -> Self {
        Self {
            utility_weight: 1.0,
            temperature: 1.0,
            convergence_threshold: 0.01,
            max_iterations: 100,
            min_stake: 1.0,
            slash_fraction: 0.1,
            num_permutations: 100,
        }
    }
}
