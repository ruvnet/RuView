//! Model Architectures for RuvLLM
//!
//! This module contains model architecture implementations optimized for
//! various hardware targets including Apple Neural Engine (ANE), Metal GPU,
//! and CPU.
//!
//! ## Available Models
//!
//! | Model | Architecture | Params | ANE Optimized | Use Case |
//! |-------|--------------|--------|---------------|----------|
//! | RuvLTRA-Small | Qwen 0.5B | 500M | Yes | Edge inference, mobile |
//! | RuvLTRA-Medium | Qwen2.5-3B | 3B | Yes | Balanced quality/performance |
//!
//! ## Model Selection Guide
//!
//! ```text
//! Model Size vs Performance:
//!
//!   RuvLTRA-Small (0.5B)  ████████░░  Good quality, fast inference
//!                                      ANE: 38 TOPS, ~200 tok/s
//!
//!   RuvLTRA-Medium (3B)   ██████████  High quality, moderate speed
//!                                      GPU/ANE: ~50-80 tok/s, SONA learning
//!
//!   Phi-3 (3B)            ██████████  High quality, moderate speed
//!                                      GPU: Metal, ~50 tok/s
//!
//!   Qwen 1.8B             █████████░  Balanced quality/speed
//!                                      GPU: Metal, ~80 tok/s
//! ```
//!
//! ## Usage
//!
//! ### RuvLTRA-Small (0.5B)
//!
//! ```rust,ignore
//! use ruvllm::models::ruvltra::{RuvLtraConfig, RuvLtraModel};
//!
//! // Create model with default Qwen 0.5B config
//! let config = RuvLtraConfig::default();
//! let model = RuvLtraModel::new(&config)?;
//!
//! // Run inference
//! let logits = model.forward(&input_ids, &positions, None)?;
//! ```
//!
//! ### RuvLTRA-Medium (3B)
//!
//! ```rust,ignore
//! use ruvllm::models::ruvltra_medium::{RuvLtraMediumConfig, RuvLtraMediumModel};
//!
//! // Create base variant
//! let config = RuvLtraMediumConfig::base();
//! let mut model = RuvLtraMediumModel::new(&config)?;
//!
//! // Enable SONA learning hooks at layers 8, 16, 24
//! model.enable_sona_with_hooks(&[8, 16, 24])?;
//!
//! // Run inference with paged attention
//! let logits = model.forward(&input_ids, &positions)?;
//! ```

pub mod ruvltra;
pub mod ruvltra_medium;

// Re-export RuvLTRA-Small types
pub use ruvltra::{
    AneDispatcher,
    AneOptimization,
    MemoryLayout,
    QuantizationType,
    RuvLtraAttention,
    // Configuration
    RuvLtraConfig,
    RuvLtraDecoderLayer,
    RuvLtraMLP,
    // Model components
    RuvLtraModel,
    // Utilities
    RuvLtraModelInfo,
};

// Re-export RuvLTRA-Medium types
pub use ruvltra_medium::{
    RuvLtraMediumAttention,
    // Configuration
    RuvLtraMediumConfig,
    RuvLtraMediumDecoderLayer,
    RuvLtraMediumMLP,
    // Model components
    RuvLtraMediumModel,
    // Utilities
    RuvLtraMediumModelInfo,
    RuvLtraMediumQuant,
    RuvLtraMediumVariant,
    SonaHookConfig,
};
