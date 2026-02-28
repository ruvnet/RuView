//! # ruvector-attention SDK
//!
//! High-level, ergonomic APIs for building attention mechanisms.

pub mod builder;
pub mod pipeline;
pub mod presets;

pub use builder::{flash, multi_head, scaled_dot, AttentionBuilder, AttentionType};
pub use pipeline::{AttentionPipeline, NormType, PipelineStage};
pub use presets::{for_graphs, for_large_scale, for_sequences, AttentionPreset};
