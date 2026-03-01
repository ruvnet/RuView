//! DAG Attention Mechanisms
//!
//! This module provides graph-topology-aware attention mechanisms for DAG-based
//! query optimization. Unlike traditional neural attention, these mechanisms
//! leverage the structural properties of the DAG (topology, paths, cuts) to
//! compute attention scores.

// Team 2 (Agent #2) - Base attention mechanisms
mod causal_cone;
mod critical_path;
mod mincut_gated;
mod topological;
mod traits;

// Team 2 (Agent #3) - Advanced attention mechanisms
mod cache;
mod hierarchical_lorentz;
mod parallel_branch;
mod selector;
mod temporal_btsp;
mod trait_def;

// Export base mechanisms
pub use causal_cone::{CausalConeAttention, CausalConeConfig};
pub use critical_path::{CriticalPathAttention, CriticalPathConfig};
pub use mincut_gated::{FlowCapacity, MinCutConfig, MinCutGatedAttention};
pub use topological::{TopologicalAttention, TopologicalConfig};
pub use traits::{AttentionConfig, AttentionError, AttentionScores, DagAttention};

// Export advanced mechanisms
pub use cache::{AttentionCache, CacheConfig, CacheStats};
pub use hierarchical_lorentz::{HierarchicalLorentzAttention, HierarchicalLorentzConfig};
pub use parallel_branch::{ParallelBranchAttention, ParallelBranchConfig};
pub use selector::{AttentionSelector, MechanismStats, SelectorConfig};
pub use temporal_btsp::{TemporalBTSPAttention, TemporalBTSPConfig};
pub use trait_def::{
    AttentionError as AttentionErrorV2, AttentionScores as AttentionScoresV2, DagAttentionMechanism,
};
