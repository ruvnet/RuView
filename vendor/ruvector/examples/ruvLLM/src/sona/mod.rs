//! SONA (Self-Optimizing Neural Architecture)
//!
//! Adaptive learning system with ReasoningBank integration.

pub mod engine;
pub mod ewc;
pub mod loops;
pub mod lora;
pub mod reasoning_bank;
pub mod trajectory;
pub mod types;

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
