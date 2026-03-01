//! Federation error types.

use thiserror::Error;

/// Errors that can occur during federation operations.
#[derive(Debug, Error)]
pub enum FederationError {
    #[error("privacy budget exhausted: spent {spent:.4}, limit {limit:.4}")]
    PrivacyBudgetExhausted { spent: f64, limit: f64 },

    #[error("invalid epsilon value: {0} (must be > 0)")]
    InvalidEpsilon(f64),

    #[error("invalid delta value: {0} (must be in (0, 1))")]
    InvalidDelta(f64),

    #[error("segment validation failed: {0}")]
    SegmentValidation(String),

    #[error("version mismatch: expected {expected}, got {got}")]
    VersionMismatch { expected: u32, got: u32 },

    #[error("signature verification failed")]
    SignatureVerification,

    #[error("witness chain broken at index {0}")]
    WitnessChainBroken(usize),

    #[error("insufficient observations: need {needed}, have {have}")]
    InsufficientObservations { needed: u64, have: u64 },

    #[error("quality below threshold: {score:.4} < {threshold:.4}")]
    QualityBelowThreshold { score: f64, threshold: f64 },

    #[error("export rate limited: next export allowed at {next_allowed_epoch_s}")]
    RateLimited { next_allowed_epoch_s: u64 },

    #[error("PII detected after stripping: {field}")]
    PiiLeakDetected { field: String },

    #[error("Byzantine outlier detected from contributor {contributor}")]
    ByzantineOutlier { contributor: String },

    #[error("aggregation requires at least {min} contributions, got {got}")]
    InsufficientContributions { min: usize, got: usize },

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("io error: {0}")]
    Io(String),
}
