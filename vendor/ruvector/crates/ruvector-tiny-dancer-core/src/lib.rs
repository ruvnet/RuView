//! # Tiny Dancer: Production-Grade AI Agent Routing System
//!
//! High-performance neural routing system for optimizing LLM inference costs.
//!
//! This crate provides:
//! - FastGRNN model inference (sub-millisecond latency)
//! - Feature engineering for candidate scoring
//! - Model optimization (quantization, pruning)
//! - Uncertainty quantification with conformal prediction
//! - Circuit breaker patterns for graceful degradation
//! - SQLite/AgentDB integration
//! - Training infrastructure with knowledge distillation

#![deny(unsafe_op_in_unsafe_fn)]
#![warn(missing_docs, rustdoc::broken_intra_doc_links)]

pub mod circuit_breaker;
pub mod error;
pub mod feature_engineering;
pub mod model;
pub mod optimization;
pub mod router;
pub mod storage;
pub mod training;
pub mod types;
pub mod uncertainty;

// Re-exports for convenience
pub use error::{Result, TinyDancerError};
pub use model::{FastGRNN, FastGRNNConfig};
pub use router::Router;
pub use training::{
    generate_teacher_predictions, Trainer, TrainingConfig, TrainingDataset, TrainingMetrics,
};
pub use types::{
    Candidate, RouterConfig, RoutingDecision, RoutingMetrics, RoutingRequest, RoutingResponse,
};

/// Version of the Tiny Dancer library
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
