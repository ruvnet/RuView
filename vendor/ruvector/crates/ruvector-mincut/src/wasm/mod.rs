//! WASM bindings and optimizations for agentic chip
//!
//! Provides:
//! - SIMD-accelerated boundary computation
//! - Agentic chip interface
//! - Inter-core messaging

pub mod agentic;
pub mod simd;

pub use agentic::*;
pub use simd::*;
