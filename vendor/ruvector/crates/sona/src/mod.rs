//! SONA (Self-Optimizing Neural Architecture)
//!
//! Adaptive learning system with ReasoningBank integration.

pub mod types;
pub mod lora;
pub mod trajectory;
pub mod ewc;
pub mod reasoning_bank;
pub mod loops;
pub mod engine;

// Re-export main types
pub use types::{
    LearningSignal, QueryTrajectory, TrajectoryStep,
    LearnedPattern, PatternType, SignalMetadata, SonaConfig,
};
pub use lora::{MicroLoRA, BaseLoRA, LoRAEngine, LoRALayer};
pub use trajectory::{TrajectoryBuffer, TrajectoryBuilder, TrajectoryIdGen};
pub use ewc::{EwcConfig, EwcPlusPlus, TaskFisher};
pub use reasoning_bank::{ReasoningBank, PatternConfig};
pub use loops::{InstantLoop, BackgroundLoop, LoopCoordinator};
pub use engine::SonaEngine;
