//! # ruvector-attention
//!
//! Attention mechanisms for ruvector, including geometric, graph, and sparse attention.
//!
//! This crate provides efficient implementations of various attention mechanisms:
//! - Scaled dot-product attention
//! - Multi-head attention with parallel processing
//! - Graph attention for GNN applications
//! - Geometric attention in hyperbolic spaces
//! - Sparse attention patterns
//!
//! ## Features
//!
//! - **SIMD Acceleration**: Optional SIMD optimizations for performance
//! - **Parallel Processing**: Rayon-based parallel head computation
//! - **WASM Support**: WebAssembly compilation support
//! - **NAPI Bindings**: Node.js bindings for JavaScript integration
//!
//! ## Example
//!
//! ```rust
//! use ruvector_attention::{
//!     attention::ScaledDotProductAttention,
//!     traits::Attention,
//! };
//!
//! // Create scaled dot-product attention
//! let attention = ScaledDotProductAttention::new(512);
//!
//! // Prepare inputs
//! let query = vec![1.0; 512];
//! let keys = vec![vec![0.5; 512], vec![0.3; 512]];
//! let values = vec![vec![1.0; 512], vec![2.0; 512]];
//!
//! let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
//! let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();
//!
//! // Compute attention
//! let output = attention.compute(&query, &keys_refs, &values_refs).unwrap();
//! assert_eq!(output.len(), 512);
//! ```

pub mod attention;
pub mod config;
pub mod error;
pub mod graph;
pub mod hyperbolic;
pub mod moe;
pub mod sdk;
pub mod sparse;
pub mod training;
pub mod traits;
pub mod utils;

// Advanced attention mechanisms
pub mod curvature;
pub mod topology;
pub mod transport;

// Mathematical foundations
pub mod info_bottleneck;
pub mod info_geometry;
pub mod pde_attention;
pub mod unified_report;

// Sheaf attention (Coherence-Gated Transformer per ADR-015)
#[cfg(feature = "sheaf")]
pub mod sheaf;

// Re-export main types
pub use attention::{MultiHeadAttention, ScaledDotProductAttention};
pub use config::{AttentionConfig, GraphAttentionConfig, SparseAttentionConfig};
pub use error::{AttentionError, AttentionResult};
pub use hyperbolic::{
    exp_map, log_map, mobius_add, poincare_distance, project_to_ball, HyperbolicAttention,
    HyperbolicAttentionConfig, MixedCurvatureAttention, MixedCurvatureConfig,
};
pub use traits::{
    Attention, EdgeInfo, GeometricAttention, Gradients, GraphAttention, SparseAttention,
    SparseMask, TrainableAttention,
};

// Sparse attention exports
pub use sparse::{
    AttentionMask, FlashAttention, LinearAttention, LocalGlobalAttention, SparseMaskBuilder,
};

// MoE exports
pub use moe::{
    Expert, ExpertType, HyperbolicExpert, LearnedRouter, LinearExpert, MoEAttention, MoEConfig,
    Router, StandardExpert, TopKRouting,
};

// Graph attention exports
pub use graph::{
    DualSpaceAttention, DualSpaceConfig, EdgeFeaturedAttention, EdgeFeaturedConfig, GraphRoPE,
    RoPEConfig,
};

// Training exports
pub use training::{
    Adam, AdamW, CurriculumScheduler, CurriculumStage, DecayType, HardNegativeMiner, InfoNCELoss,
    LocalContrastiveLoss, Loss, MiningStrategy, NegativeMiner, Optimizer, Reduction,
    SpectralRegularization, TemperatureAnnealing, SGD,
};

// SDK exports
pub use sdk::{presets, AttentionBuilder, AttentionPipeline};

// Transport (OT-based attention) exports
pub use transport::{
    CentroidCache, CentroidOTAttention, CentroidOTConfig, ProjectionCache,
    SlicedWassersteinAttention, SlicedWassersteinConfig, WindowCache,
};

// Curvature (Mixed curvature attention) exports
pub use curvature::{
    ComponentQuantizer, FusedCurvatureConfig, MixedCurvatureCache, MixedCurvatureFusedAttention,
    QuantizationConfig, QuantizedVector, TangentSpaceConfig, TangentSpaceMapper,
};

// Topology (Gated attention) exports
pub use topology::{
    AttentionMode, AttentionPolicy, CoherenceMetric, PolicyConfig, TopologyGatedAttention,
    TopologyGatedConfig, WindowCoherence,
};

// Information Geometry exports
pub use info_geometry::{FisherConfig, FisherMetric, NaturalGradient, NaturalGradientConfig};

// Information Bottleneck exports
pub use info_bottleneck::{DiagonalGaussian, IBConfig, InformationBottleneck, KLDivergence};

// PDE Attention exports
pub use pde_attention::{DiffusionAttention, DiffusionConfig, GraphLaplacian, LaplacianType};

// Sheaf Attention exports (Coherence-Gated Transformer per ADR-015)
#[cfg(feature = "sheaf")]
pub use sheaf::{
    process_with_early_exit, ComputeLane, EarlyExit, EarlyExitConfig, EarlyExitResult,
    EarlyExitStatistics, ExitReason, LaneStatistics, ResidualSparseMask, RestrictionMap,
    RestrictionMapConfig, RoutingDecision, SheafAttention, SheafAttentionConfig,
    SparseResidualAttention, SparseResidualConfig, SparsityStatistics, TokenRouter,
    TokenRouterConfig,
};

// Unified Report exports
pub use unified_report::{
    AttentionRecommendation, GeometryReport, MetricType, MetricValue, ReportBuilder, ReportConfig,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_basic_attention_workflow() {
        let config = AttentionConfig::builder()
            .dim(64)
            .num_heads(4)
            .build()
            .unwrap();

        assert_eq!(config.dim, 64);
        assert_eq!(config.num_heads, 4);
        assert_eq!(config.head_dim(), 16);
    }
}
