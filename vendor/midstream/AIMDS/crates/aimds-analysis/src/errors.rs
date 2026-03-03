//! Error types for AIMDS analysis layer

use thiserror::Error;

/// Analysis error types
#[derive(Error, Debug)]
pub enum AnalysisError {
    #[error("Behavioral analysis failed: {0}")]
    BehavioralAnalysis(String),

    #[error("Policy verification failed: {0}")]
    PolicyVerification(String),

    #[error("LTL checking failed: {0}")]
    LTLCheck(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Configuration error: {0}")]
    Configuration(String),

    #[error("Temporal attractor error: {0}")]
    TemporalAttractor(String),

    #[error("Neural solver error: {0}")]
    NeuralSolver(String),

    #[error("Core error: {0}")]
    Core(#[from] aimds_core::error::AimdsError),

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type for analysis operations
pub type AnalysisResult<T> = Result<T, AnalysisError>;
