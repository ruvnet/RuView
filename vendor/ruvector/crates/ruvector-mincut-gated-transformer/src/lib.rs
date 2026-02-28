//! # Mincut Gated Transformer
//!
//! Ultra low latency transformer inference designed for continuous systems.
//! Governed by a coherence controller driven by dynamic minimum cut signals
//! and optionally a spiking scheduler that skips work when nothing meaningful
//! is happening.
//!
//! ## Academic Foundations
//!
//! This crate integrates multiple state-of-the-art optimization techniques:
//!
//! 1. **Mixture-of-Depths** (Raposo et al., 2024) - Dynamic compute allocation with 50% FLOPs reduction
//! 2. **Early Exit** (Elhoushi et al., 2024) - Layer-skipping with 30-50% latency reduction
//! 3. **Sparse Attention** (Jiang et al., 2024) - 90% attention FLOPs reduction for long contexts
//! 4. **Energy-Based Transformers** (Gladstone et al., 2025) - Principled compute-quality tradeoffs
//! 5. **Spike-Driven Inference** (Yao et al., 2023, 2024) - 87× energy reduction via event-driven compute
//! 6. **Spectral Methods** (Kreuzer et al., 2021) - Graph-based coherence via spectral partitioning
//!
//! See `docs/THEORY.md` for detailed academic references and theoretical analysis.
//!
//! ## Primary Outcomes
//!
//! 1. **Deterministic, bounded inference** - Same inputs yield same outputs
//! 2. **Allocation-free hot path** - Zero heap allocations after initialization
//! 3. **Predictable tail latency** - Bounded p99 latency guarantees
//! 4. **Explainable interventions** - Every gate decision produces a witness
//! 5. **Easy integration** - Works with RuVector, ruvector-mincut, and agent orchestration
//!
//! ## Core Concepts
//!
//! The system has three roles:
//!
//! 1. **Transformer Kernel** - Produces logits or scores under fixed compute budgets
//! 2. **Spike Scheduler** (optional) - Decides whether to run and selects compute tier
//! 3. **Mincut Gate** (authoritative) - Decides what state changes are allowed
//!
//! ## Example
//!
//! ```rust,no_run
//! use ruvector_mincut_gated_transformer::{
//!     MincutGatedTransformer, TransformerConfig, GatePolicy,
//!     GatePacket, InferInput, InferOutput,
//! };
//!
//! // Create configuration
//! let config = TransformerConfig::micro();
//! let policy = GatePolicy::default();
//!
//! // Load weights (pseudo-code)
//! # let weights = ruvector_mincut_gated_transformer::QuantizedWeights::empty(&config);
//!
//! // Create transformer
//! let mut transformer = MincutGatedTransformer::new(config, policy, weights).unwrap();
//!
//! // Create gate packet from mincut signals
//! let gate = GatePacket {
//!     lambda: 100,
//!     lambda_prev: 95,
//!     boundary_edges: 5,
//!     boundary_concentration_q15: 8192,
//!     partition_count: 3,
//!     flags: 0,
//! };
//!
//! // Prepare input
//! let input = InferInput {
//!     tokens: Some(&[1, 2, 3, 4]),
//!     embedding_q: None,
//!     embedding_scale: 1.0,
//!     input_signature: None,
//!     gate,
//!     spikes: None,
//! };
//!
//! // Allocate output buffer
//! let mut logits = vec![0i32; 1024];
//! let mut output = InferOutput::new(&mut logits);
//!
//! // Run inference
//! transformer.infer(&input, &mut output).unwrap();
//!
//! // Check witness for allowed actions
//! if output.witness.external_writes_enabled == 1 {
//!     // Safe to persist memory
//! }
//! ```

#![cfg_attr(feature = "no_std_gateway", no_std)]

#[cfg(feature = "no_std_gateway")]
extern crate alloc;

pub mod arena;
pub mod attention;
pub mod config;
pub mod early_exit;
pub mod error;
pub mod ffn;
pub mod flash_attention;
pub mod gate;
pub mod kernel;
pub mod kv_cache;
pub mod mamba;
pub mod mod_routing;
pub mod model;
pub mod packets;
pub mod q15;
pub mod rope;
pub mod speculative;
pub mod spike;
pub mod state;

#[cfg(feature = "trace")]
pub mod trace;

#[cfg(feature = "spectral_pe")]
pub mod spectral;

#[cfg(feature = "sparse_attention")]
pub mod sparse_attention;

#[cfg(feature = "energy_gate")]
pub mod energy_gate;

// Re-exports for convenient access
pub use arena::{calculate_arena_size, LayerWeights, WeightArena, WeightRef};
pub use config::{GatePolicy, TransformerConfig};
pub use early_exit::{CoherenceEarlyExit, EarlyExitConfig, EarlyExitDecision, ExitReason};
pub use error::{Error, Result};
pub use flash_attention::{
    flash_attention_forward, flash_attention_forward_i8, flash_mha, FlashAttentionConfig,
};
pub use gate::{GateController, TierDecision};
// Legacy KV cache types (backward compatibility)
pub use kv_cache::{HadamardTransform, QuantBits, QuantizedKVCache};
// New three-tier KV cache types (ADR-004)
pub use kv_cache::{
    AdaptiveKVCache, AdaptiveKVCacheConfig, ArchiveQuantizer, CacheTier, EvictionDecision,
    HotBuffer, HotBufferConfig, KVQuantKeyMode, KVQuantQuantizer, KVQuantValueMode, KiviQuantizer,
    MemoryStats, QualityFeedback, QualityMetric, QualityTracker, QuantScheme, QuantizedKV,
    RematerializationPolicy, SQuatCompressed, SQuatQuantizer, TierBoundary, TierConfig, TierPolicy,
};
pub use mamba::{MambaConfig, MambaLayer, MambaState, MambaWeights};
pub use mod_routing::{MincutDepthRouter, ModRoutingConfig, RoutingStats, TokenRoute};
pub use model::{MincutGatedTransformer, QuantizedWeights, WeightsLoader};
pub use packets::{
    GateDecision, GatePacket, GateReason, InferInput, InferOutput, InferStats, SpikePacket, Witness,
};
pub use q15::{
    f32_to_q15_batch, q15_batch_add, q15_batch_lerp, q15_batch_mul, q15_dot, q15_to_f32_batch, Q15,
};
pub use rope::{RopeConfig, RopeEmbedding, RopeScaling};
pub use speculative::{
    generate_tree_attention_mask, DraftToken, DraftTree, SpeculativeConfig, SpeculativeDecoder,
    VerificationResult,
};
pub use spike::SpikeScheduler;
pub use state::RuntimeState;

#[cfg(feature = "trace")]
pub use trace::{TraceCounters, TraceSnapshot, TraceState};

#[cfg(feature = "spike_attention")]
pub use attention::spike_driven::{SpikeDrivenAttention, SpikeDrivenConfig, SpikeTrain};

#[cfg(feature = "spectral_pe")]
pub use spectral::{
    lanczos_sparse, power_iteration_sparse, SparseCSR, SpectralPEConfig, SpectralPositionEncoder,
};

#[cfg(feature = "sparse_attention")]
pub use sparse_attention::{
    LambdaDensitySchedule, MincutSparseAttention, SparseMask, SparsityConfig,
};

#[cfg(feature = "energy_gate")]
pub use energy_gate::{EnergyGate, EnergyGateConfig, EnergyGradient};

/// Crate version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::{
        generate_tree_attention_mask,
        // Three-tier KV cache (ADR-004)
        AdaptiveKVCache,
        AdaptiveKVCacheConfig,
        ArchiveQuantizer,
        CacheTier,
        CoherenceEarlyExit,
        DraftToken,
        DraftTree,
        EarlyExitConfig,
        EarlyExitDecision,
        Error,
        ExitReason,
        GateDecision,
        GatePacket,
        GatePolicy,
        GateReason,
        HadamardTransform,
        InferInput,
        InferOutput,
        InferStats,
        KVQuantQuantizer,
        KiviQuantizer,
        MambaConfig,
        MambaLayer,
        MambaState,
        MambaWeights,
        MincutDepthRouter,
        MincutGatedTransformer,
        ModRoutingConfig,
        QuantBits,
        QuantizedKVCache,
        QuantizedWeights,
        Result,
        RopeConfig,
        RopeEmbedding,
        RopeScaling,
        RoutingStats,
        SQuatQuantizer,
        SpeculativeConfig,
        SpeculativeDecoder,
        SpikePacket,
        TierBoundary,
        TokenRoute,
        TransformerConfig,
        VerificationResult,
        WeightsLoader,
        Witness,
    };

    #[cfg(feature = "trace")]
    pub use crate::{TraceCounters, TraceSnapshot};
}

/// Supported model configurations
pub mod configs {
    use super::TransformerConfig;

    /// Baseline CPU configuration
    /// - Sequence length: 64
    /// - Hidden size: 256
    /// - Heads: 4
    /// - Layers: 4
    pub fn baseline() -> TransformerConfig {
        TransformerConfig::baseline()
    }

    /// Micro configuration for WASM and edge gateways
    /// - Sequence length: 32
    /// - Hidden size: 128
    /// - Heads: 4
    /// - Layers: 2
    pub fn micro() -> TransformerConfig {
        TransformerConfig::micro()
    }
}
