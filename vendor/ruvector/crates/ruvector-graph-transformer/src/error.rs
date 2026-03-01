//! Error types for the graph transformer crate.
//!
//! Composes errors from all sub-crates into a unified error type.

use thiserror::Error;

/// Unified error type for graph transformer operations.
#[derive(Debug, Error)]
pub enum GraphTransformerError {
    /// Verification error from ruvector-verified.
    #[error("verification error: {0}")]
    Verification(#[from] ruvector_verified::VerificationError),

    /// GNN layer error from ruvector-gnn.
    #[error("gnn error: {0}")]
    Gnn(#[from] ruvector_gnn::GnnError),

    /// Attention error from ruvector-attention.
    #[error("attention error: {0}")]
    Attention(#[from] ruvector_attention::AttentionError),

    /// MinCut error from ruvector-mincut.
    #[error("mincut error: {0}")]
    MinCut(#[from] ruvector_mincut::MinCutError),

    /// Proof gate violation: mutation attempted without valid proof.
    #[error("proof gate violation: {0}")]
    ProofGateViolation(String),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Invariant violation detected during execution.
    #[error("invariant violation: {0}")]
    InvariantViolation(String),

    /// Dimension mismatch in graph transformer operations.
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension.
        expected: usize,
        /// Actual dimension.
        actual: usize,
    },

    /// Numerical error (NaN, Inf, or other instability).
    #[error("numerical error: {0}")]
    NumericalError(String),
}

/// Convenience result type for graph transformer operations.
pub type Result<T> = std::result::Result<T, GraphTransformerError>;
