//! SONA: Self-Optimizing Neural Architecture for DAG Learning

mod engine;
mod ewc;
mod micro_lora;
mod reasoning_bank;
mod trajectory;

pub use engine::DagSonaEngine;
pub use ewc::{EwcConfig, EwcPlusPlus};
pub use micro_lora::{MicroLoRA, MicroLoRAConfig};
pub use reasoning_bank::{DagPattern, DagReasoningBank, ReasoningBankConfig};
pub use trajectory::{DagTrajectory, DagTrajectoryBuffer};
