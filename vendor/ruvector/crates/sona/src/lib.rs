//! SONA (Self-Optimizing Neural Architecture)
//!
//! A lightweight adaptive learning system with ReasoningBank integration.
//!
//! ## Features
//!
//! - **Micro-LoRA**: Ultra-low rank (1-2) LoRA for instant learning
//! - **Base-LoRA**: Standard LoRA for background learning
//! - **EWC++**: Elastic Weight Consolidation to prevent catastrophic forgetting
//! - **ReasoningBank**: Pattern extraction and similarity search
//! - **Three Learning Loops**: Instant, Background, and Coordination loops
//! - **WASM Support**: Run in browsers and edge devices (enable `wasm` feature)
//!
//! ## Example
//!
//! ```rust,ignore
//! use sona::{SonaEngine, SonaConfig};
//!
//! // Create engine
//! let engine = SonaEngine::new(SonaConfig {
//!     hidden_dim: 256,
//!     embedding_dim: 256,
//!     ..Default::default()
//! });
//!
//! // Begin trajectory
//! let mut builder = engine.begin_trajectory(vec![0.1; 256]);
//! builder.add_step(vec![0.5; 256], vec![], 0.8);
//!
//! // End trajectory
//! engine.end_trajectory(builder, 0.85);
//!
//! // Apply learned transformations
//! let input = vec![1.0; 256];
//! let mut output = vec![0.0; 256];
//! engine.apply_micro_lora(&input, &mut output);
//! ```
//!
//! ## WASM Usage
//!
//! Enable the `wasm` feature and build with:
//! ```bash
//! wasm-pack build --target web --features wasm
//! ```

#![allow(missing_docs)]

pub mod engine;
pub mod ewc;
pub mod loops;
pub mod lora;
pub mod reasoning_bank;
pub mod time_compat;
pub mod trajectory;
pub mod types;

#[cfg(feature = "serde-support")]
pub mod export;

#[cfg(feature = "serde-support")]
pub mod training;

#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(feature = "napi")]
pub mod napi_simple;

// Re-export main types
pub use engine::SonaEngine;
pub use ewc::{EwcConfig, EwcPlusPlus, TaskFisher};
pub use loops::{BackgroundLoop, InstantLoop, LoopCoordinator};
pub use lora::{BaseLoRA, LoRAEngine, LoRALayer, MicroLoRA};
pub use reasoning_bank::{PatternConfig, ReasoningBank};
pub use trajectory::{TrajectoryBuffer, TrajectoryBuilder, TrajectoryIdGen};
pub use types::{
    LearnedPattern, LearningSignal, PatternType, QueryTrajectory, SignalMetadata, SonaConfig,
    TrajectoryStep,
};

#[cfg(feature = "serde-support")]
pub use export::{
    DatasetExporter, ExportConfig, ExportError, ExportResult, ExportType, HuggingFaceExporter,
    HuggingFaceHub, PretrainConfig, PretrainPipeline, SafeTensorsExporter,
};

#[cfg(feature = "serde-support")]
pub use training::{
    AgentExport, AgentFactory, AgentHandle, AgentStats, AgentType, AggregationResult, BatchConfig,
    CoordinatorStats, DataSizeHint, EphemeralAgent, EpochStats, FederatedCoordinator,
    FederatedTopology, ManagedAgent, PipelineStage, TaskDomain, TemplatePreset, TrainingMethod,
    TrainingMetrics, TrainingPipeline, TrainingResult, TrainingTemplate, VerticalConfig,
};

#[cfg(feature = "wasm")]
pub use wasm::WasmSonaEngine;
