//! MicroLoRA WASM - Ultra-fast Low-Rank Adaptation for Edge AI
//!
//! This crate provides rank-2 LoRA (Low-Rank Adaptation) matrices optimized for
//! WASM execution with <100us adaptation latency. Designed for real-time
//! per-operator-type learning in query optimization systems.
//!
//! ## Key Features
//!
//! - **Rank-2 LoRA**: Minimal parameter count (2d parameters per adapter)
//! - **Per-Operator Scoping**: Separate adapters for different operator types
//! - **<100us Adaptation**: Instant weight updates for real-time learning
//! - **WASM-Optimized**: no_std compatible, minimal allocations
//!
//! ## Architecture
//!
//! ```text
//! Input Embedding (d-dim)
//!        |
//!        v
//!   +---------+
//!   | A: d x 2 |  Down projection
//!   +---------+
//!        |
//!        v
//!   +---------+
//!   | B: 2 x d |  Up projection
//!   +---------+
//!        |
//!        v
//! Delta W = alpha * (A @ B)
//!        |
//!        v
//! Output = Input + Delta W
//! ```

mod lora;
mod operator_scope;
mod trajectory;

pub use lora::{LoRAConfig, LoRAPair, MicroLoRAEngine};
pub use operator_scope::{OperatorScope, ScopedLoRA};
pub use trajectory::{Trajectory, TrajectoryBuffer, TrajectoryStats};

// Re-export core types for JS
pub use lora::wasm_exports::*;
pub use operator_scope::wasm_exports::*;
