//! MicroLoRA Fine-tuning Pipeline for Real-time Per-request Adaptation
//!
//! This module provides an ultra-lightweight LoRA implementation optimized for
//! real-time adaptation with minimal overhead (<1MB per adapter).
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvllm::lora::{MicroLoRA, MicroLoraConfig, TargetModule, AdaptFeedback};
//!
//! // Create MicroLoRA for hidden dimension 4096
//! let config = MicroLoraConfig::for_hidden_dim(4096);
//! let mut lora = MicroLoRA::new(config);
//!
//! // Apply LoRA during inference
//! let delta = lora.forward(&input_tensor, &TargetModule::QProj);
//! let output: Vec<f32> = base_output.iter()
//!     .zip(delta.iter())
//!     .map(|(b, d)| b + d)
//!     .collect();
//!
//! // Adapt based on quality feedback
//! let feedback = AdaptFeedback::from_quality(0.85);
//! lora.adapt(&input_tensor, feedback)?;
//! lora.apply_updates(0.01);  // learning rate
//! ```
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Request           |---->| MicroLoRA         |
//! | (input tensor)    |     | - Rank 1-2        |
//! +-------------------+     | - <1ms forward    |
//!                           | - Per-request     |
//!                           +--------+----------+
//!                                    |
//!                                    v (async feedback)
//!                           +--------+----------+
//!                           | Training Pipeline |
//!                           | - EWC++ regul.    |
//!                           | - Single-example  |
//!                           | - LR scheduling   |
//!                           +--------+----------+
//!                                    |
//!                                    v
//!                           +--------+----------+
//!                           | Adapter Manager   |
//!                           | - Hot-swapping    |
//!                           | - Composition     |
//!                           | - Persistence     |
//!                           +-------------------+
//! ```
//!
//! ## Target Modules
//!
//! Choose which transformer components to adapt:
//!
//! | Module | Memory | Impact | Recommended For |
//! |--------|--------|--------|-----------------|
//! | `QProj` | Low | High | Attention focus |
//! | `KProj` | Low | Medium | Key patterns |
//! | `VProj` | Low | High | Content generation |
//! | `OProj` | Low | Medium | Output projection |
//! | `GateProj` | Medium | High | FFN routing |
//! | `UpProj` | High | Medium | FFN expansion |
//! | `DownProj` | High | Medium | FFN compression |
//!
//! ## Features
//!
//! - **Ultra-lightweight**: Rank 1-2 adapters with <1MB memory footprint
//! - **Real-time**: Per-request adaptation with <1ms forward pass
//! - **EWC++ Integration**: Prevents catastrophic forgetting during adaptation
//! - **NEON/SIMD Optimized**: Hardware-accelerated forward and backward passes
//! - **Async Adaptation**: Non-blocking training with feedback loops
//! - **Hot-swapping**: Seamlessly switch adapters without model reload
//!
//! ## Training with EWC++
//!
//! ```rust,ignore
//! use ruvllm::lora::{TrainingPipeline, TrainingConfig, EwcRegularizer};
//!
//! let config = TrainingConfig {
//!     learning_rate: 0.001,
//!     ewc_lambda: 0.1,  // Regularization strength
//!     quality_threshold: 0.5,
//!     ..Default::default()
//! };
//!
//! let mut pipeline = TrainingPipeline::new(config);
//! pipeline.init_for_lora(&lora);
//!
//! // Train on samples
//! for sample in samples {
//!     pipeline.train_step(&lora, &sample.input, sample.feedback)?;
//! }
//!
//! // Mark task boundary (computes Fisher information)
//! pipeline.start_new_task(&lora);
//! ```

pub mod adapter;
pub mod adapters;
pub mod micro_lora;
pub mod training;

// Re-exports
pub use adapter::{
    AdapterComposer, AdapterHandle, AdapterPool, AdapterRegistry, CompositionStrategy,
};
pub use adapters::{
    merge::{AdapterMerger, HotSwapManager, MergeConfig, MergeStrategy},
    trainer::{
        AdapterDataset, AdapterTrainer, AdapterTrainingConfig, SyntheticDataGenerator,
        TrainingExample,
    },
    AdapterMetadata, LoraConfig, RuvLtraAdapters,
};
pub use micro_lora::{AdaptFeedback, LoraAdapter, MicroLoRA, MicroLoraConfig, TargetModule};
pub use training::{
    EwcRegularizer, GradientAccumulator, LearningRateSchedule, TrainingConfig, TrainingPipeline,
};
