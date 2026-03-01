//! Real-time Optimization System for RuvLLM
//!
//! This module provides the optimization infrastructure for LLM inference,
//! integrating SONA learning with MicroLoRA and custom kernels.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvllm::optimization::{
//!     SonaLlm, SonaLlmConfig, RealtimeOptimizer, RealtimeConfig,
//!     MetricsCollector, ConsolidationStrategy,
//! };
//!
//! // Create SONA integration for three-tier learning
//! let sona_config = SonaLlmConfig {
//!     instant_lr: 0.01,
//!     background_interval_ms: 100,
//!     background_min_samples: 10,
//!     consolidation_strategy: ConsolidationStrategy::EwcMerge,
//!     ..Default::default()
//! };
//! let sona = SonaLlm::new(sona_config);
//!
//! // During inference: instant adaptation
//! let result = sona.instant_adapt(&query_embedding, &response_embedding, 0.85);
//! println!("Adapt latency: {}us", result.latency_us);
//!
//! // Periodic: background consolidation
//! if let Some(bg_result) = sona.maybe_background() {
//!     println!("Consolidated {} samples", bg_result.samples_used);
//! }
//!
//! // Triggered: deep optimization
//! if sona.should_trigger_deep() {
//!     let deep_result = sona.deep_optimize(&samples);
//!     println!("Quality delta: {:.3}", deep_result.quality_delta);
//! }
//! ```
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Inference Request |---->| RealtimeOptimizer |
//! | (tokens, params)  |     | - Batch sizing    |
//! +-------------------+     | - KV management   |
//!                           | - Token budgets   |
//!                           +--------+----------+
//!                                    |
//!                                    v (metrics)
//!                           +--------+----------+
//!                           | InferenceMetrics  |
//!                           | - TTFT tracking   |
//!                           | - TPS monitoring  |
//!                           | - Memory usage    |
//!                           +--------+----------+
//!                                    |
//!                                    v (feedback)
//!                           +--------+----------+
//!                           | SonaLlm           |
//!                           | - Instant adapt   |
//!                           | - Background loop |
//!                           | - Deep optimize   |
//!                           +-------------------+
//! ```
//!
//! ## SONA Learning Tiers
//!
//! | Tier | Latency | Trigger | Action |
//! |------|---------|---------|--------|
//! | Instant | <1ms | Every request | MicroLoRA gradient update |
//! | Background | ~100ms | Timer/threshold | Pattern consolidation |
//! | Deep | Minutes | Manual/scheduled | Full training pipeline |
//!
//! ## Features
//!
//! - **Real-time Optimization**: Dynamic batch sizing and KV cache management
//! - **SONA Integration**: Three-tier learning loops for continuous improvement
//! - **Metrics Collection**: Comprehensive inference telemetry
//! - **Speculative Decoding**: Draft model integration for faster generation
//!
//! ## Consolidation Strategies
//!
//! ```rust,ignore
//! use ruvllm::optimization::ConsolidationStrategy;
//!
//! // EWC++ merge (default) - preserves important weights
//! let strategy = ConsolidationStrategy::EwcMerge;
//!
//! // Quality-weighted - higher quality samples have more influence
//! let strategy = ConsolidationStrategy::QualityWeighted;
//!
//! // Best only - keep top 20% by quality
//! let strategy = ConsolidationStrategy::BestOnly;
//! ```

pub mod metrics;
pub mod realtime;
pub mod sona_llm;

// Re-exports
pub use metrics::{
    InferenceMetrics, LatencyHistogram, MetricsCollector, MetricsSnapshot, MovingAverage,
};
pub use realtime::{
    BatchSizeStrategy, KvCachePressurePolicy, OptimizationDecision, RealtimeConfig,
    RealtimeOptimizer, SpeculativeConfig, TokenBudgetAllocation,
};
pub use sona_llm::{
    AdaptationResult, ConsolidationStrategy, LearningLoopStats, OptimizationTrigger, SonaLlm,
    SonaLlmConfig, TrainingSample,
};
