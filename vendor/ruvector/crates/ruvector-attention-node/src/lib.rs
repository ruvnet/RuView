//! ruvector-attention-node
//!
//! Node.js bindings for ruvector-attention via NAPI-RS
//!
//! This crate provides comprehensive Node.js bindings for:
//! - Attention mechanisms (dot-product, multi-head, hyperbolic, flash, linear, local-global, MoE)
//! - Training utilities (loss functions, optimizers, schedulers)
//! - Async/batch processing
//! - Graph attention mechanisms
//! - Benchmarking utilities

#![deny(clippy::all)]

use napi_derive::napi;

pub mod async_ops;
pub mod attention;
pub mod graph;
pub mod training;

// Re-export main attention types
pub use attention::{
    AttentionConfig, DotProductAttention, FlashAttention, HyperbolicAttention, LinearAttention,
    LocalGlobalAttention, MoEAttention, MoEConfig, MultiHeadAttention,
};

// Re-export training types
pub use training::{
    AdamOptimizer, AdamWOptimizer, CurriculumScheduler, CurriculumStageConfig, DecayType,
    HardNegativeMiner, InBatchMiner, InfoNCELoss, LearningRateScheduler, LocalContrastiveLoss,
    LossWithGradients, MiningStrategy, SGDOptimizer, SpectralRegularization, TemperatureAnnealing,
};

// Re-export async/batch types
pub use async_ops::{
    AttentionType, BatchConfig, BatchResult, BenchmarkResult, ParallelConfig, StreamProcessor,
};

// Re-export graph attention types
pub use graph::{
    DualSpaceAttention, DualSpaceConfig, EdgeFeaturedAttention, EdgeFeaturedConfig,
    GraphRoPEAttention, RoPEConfig,
};

/// Get library version
#[napi]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Get library info
#[napi]
pub fn info() -> LibraryInfo {
    LibraryInfo {
        name: "ruvector-attention-node".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Node.js bindings for ruvector-attention".to_string(),
        features: vec![
            "scaled-dot-product".to_string(),
            "multi-head".to_string(),
            "hyperbolic".to_string(),
            "flash".to_string(),
            "linear".to_string(),
            "local-global".to_string(),
            "moe".to_string(),
            "edge-featured".to_string(),
            "graph-rope".to_string(),
            "dual-space".to_string(),
            "training".to_string(),
            "async".to_string(),
            "batch".to_string(),
            "benchmark".to_string(),
        ],
    }
}

/// Library information
#[napi(object)]
pub struct LibraryInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub features: Vec<String>,
}
