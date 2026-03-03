//! # Temporal Computational Lead via Sublinear Local Solvers
//!
//! A Rust implementation of sublinear-time algorithms for diagonally dominant
//! linear systems that enable temporal computational lead - computing predictions
//! before network messages arrive through model-based inference.
//!
//! Based on recent 2025 research in asymmetric diagonally dominant systems.

pub mod core;
pub mod physics;
pub mod solver;
pub mod predictor;
pub mod validation;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub use core::{Matrix, Vector, SparseMatrix, Complexity};
pub use physics::{Distance, SpeedOfLight, TemporalAdvantage};
pub use predictor::{TemporalPredictor, PredictionResult, DominanceParameters};
pub use solver::{SublinearSolver, SolverMethod, SolverResult};
pub use validation::{ProofValidator, TheoremProver};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum FTLError {
    #[error("Matrix operation failed: {0}")]
    MatrixError(String),

    #[error("Solver convergence failed: {0}")]
    SolverError(String),

    #[error("Physical constraint violation: {0}")]
    PhysicsError(String),

    #[error("Validation failed: {0}")]
    ValidationError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FTLError>;