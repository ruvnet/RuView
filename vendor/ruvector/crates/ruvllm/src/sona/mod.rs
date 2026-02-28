//! SONA Learning Integration for RuvLLM
//!
//! This module provides SONA (Self-Optimizing Neural Architecture) integration
//! for the RuvLLM inference runtime, including:
//!
//! - **Core Integration**: Three-tier learning loops (Instant, Background, Deep)
//! - **RuvLTRA Pretraining**: Optimized configurations for RuvLTRA-Small (0.5B)
//!
//! ## Architecture
//!
//! The SONA integration consists of two main components:
//!
//! 1. **SonaIntegration**: Runtime learning during inference
//! 2. **RuvLtraPretrainer**: Pretraining configuration for models
//!
//! ## Learning Loops
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Request           |---->| Instant Loop      |
//! | (trajectory)      |     | - Ring buffer     |
//! +-------------------+     | - MicroLoRA       |
//!                           | - Edge weights    |
//!                           +--------+----------+
//!                                    |
//!                                    v (async)
//!                           +--------+----------+
//!                           | Background Loop   |
//!                           | - Router training |
//!                           | - EWC++ Fisher    |
//!                           | - BaseLoRA update |
//!                           +--------+----------+
//!                                    |
//!                                    v (scheduled)
//!                           +--------+----------+
//!                           | Deep Loop         |
//!                           | - Pattern bank    |
//!                           | - Memory prune    |
//!                           | - Knowledge xfer  |
//!                           +-------------------+
//! ```
//!
//! ## RuvLTRA-Small Configuration
//!
//! The `ruvltra_pretrain` module provides optimized settings for 0.5B models:
//!
//! | Parameter | Value | Rationale |
//! |-----------|-------|-----------|
//! | hidden_dim | 128 | Smaller projection for efficiency |
//! | embedding_dim | 384 | Match model hidden/2 |
//! | micro_lora_rank | 1 | Minimal overhead (<0.1MB) |
//! | base_lora_rank | 4 | Conservative for small model |
//! | ewc_lambda | 500 | Lower regularization (less to protect) |
//! | quality_threshold | 0.6 | Higher threshold for quality |
//!
//! ## Example
//!
//! ```rust,ignore
//! use ruvllm::sona::{SonaIntegration, SonaConfig, RuvLtraPretrainConfig, RuvLtraPretrainer};
//!
//! // Runtime integration
//! let config = SonaConfig::default();
//! let sona = SonaIntegration::new(config);
//!
//! // Pretraining for RuvLTRA-Small
//! let pretrain_config = RuvLtraPretrainConfig::for_ruvltra_small();
//! let mut pretrainer = RuvLtraPretrainer::new(pretrain_config);
//!
//! // Seed initial patterns
//! let seeding_result = pretrainer.seed_reasoning_bank();
//! println!("Seeded {} patterns", seeding_result.patterns_seeded);
//!
//! // Export for deployment
//! let state = pretrainer.export_state();
//! let sona = state.into_sona_integration();
//! ```

// Core integration module
pub mod integration;

// Pretraining for RuvLTRA-Small
pub mod ruvltra_pretrain;

// Re-export integration types (primary API)
pub use integration::{
    LearningLoop, RoutingRecommendation, SonaConfig, SonaIntegration, SonaStats, Trajectory,
};

// Re-export pretraining types
pub use ruvltra_pretrain::{
    DatasetConfig, ModelRouteMapping, PatternCategory, PretrainSample, PretrainedState,
    QualityPretrainConfig, QualityPretrainResult, RoutingPretrainConfig, RoutingPretrainResult,
    RuvLtraPretrainConfig, RuvLtraPretrainer, SeedingConfig, SeedingResult,
};
