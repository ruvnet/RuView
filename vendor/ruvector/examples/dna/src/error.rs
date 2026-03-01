//! Error types for DNA analysis operations

use thiserror::Error;

/// DNA analysis error types
#[derive(Error, Debug)]
pub enum DnaError {
    /// Invalid DNA sequence (e.g., non-ACGTN characters)
    #[error("Invalid DNA sequence: {0}")]
    InvalidSequence(String),

    /// K-mer indexing error
    #[error("K-mer index error: {0}")]
    IndexError(String),

    /// Sequence alignment error
    #[error("Alignment error: {0}")]
    AlignmentError(String),

    /// Variant calling error
    #[error("Variant calling error: {0}")]
    VariantCallError(String),

    /// Analysis pipeline error
    #[error("Pipeline error: {0}")]
    PipelineError(String),

    /// I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// RuVector core error
    #[error("Vector database error: {0}")]
    VectorDbError(#[from] ruvector_core::RuvectorError),

    /// Dimension mismatch
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },

    /// Empty sequence
    #[error("Empty sequence provided")]
    EmptySequence,

    /// Invalid quality score
    #[error("Invalid quality score: {0}")]
    InvalidQuality(u8),

    /// Invalid k-mer size
    #[error("Invalid k-mer size: {0}")]
    InvalidKmerSize(usize),

    /// 23andMe file parse error
    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Result type for DNA analysis operations
pub type Result<T> = std::result::Result<T, DnaError>;
