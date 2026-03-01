//! Error types for Prime-Radiant

use thiserror::Error;

/// Result type alias using the library's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in Prime-Radiant computations
#[derive(Error, Debug)]
pub enum Error {
    /// Dimension mismatch in mathematical operations
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension
        actual: usize,
    },

    /// Invalid topology configuration
    #[error("Invalid topology: {0}")]
    InvalidTopology(String),

    /// Computation failed to converge
    #[error("Computation failed to converge after {iterations} iterations")]
    ConvergenceFailure {
        /// Number of iterations attempted
        iterations: usize,
    },

    /// Singular matrix encountered
    #[error("Singular matrix: cannot compute inverse")]
    SingularMatrix,

    /// Invalid morphism composition
    #[error("Invalid morphism composition: {0}")]
    InvalidComposition(String),

    /// Category theory constraint violation
    #[error("Category constraint violated: {0}")]
    CategoryViolation(String),

    /// Sheaf condition not satisfied
    #[error("Sheaf condition violated: {0}")]
    SheafViolation(String),

    /// Invalid path in HoTT
    #[error("Invalid path: {0}")]
    InvalidPath(String),

    /// Quantum state normalization error
    #[error("Quantum state not normalized: norm = {norm}")]
    NormalizationError {
        /// Actual norm
        norm: f64,
    },

    /// Causal graph cycle detected
    #[error("Causal graph contains cycle: {0}")]
    CyclicGraph(String),

    /// Invalid intervention
    #[error("Invalid intervention: {0}")]
    InvalidIntervention(String),

    /// Numerical instability
    #[error("Numerical instability: {0}")]
    NumericalInstability(String),

    /// Feature not available
    #[error("Feature not available: {0}")]
    FeatureNotAvailable(String),
}

impl Error {
    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: usize, actual: usize) -> Self {
        Self::DimensionMismatch { expected, actual }
    }

    /// Create a convergence failure error
    pub fn convergence_failure(iterations: usize) -> Self {
        Self::ConvergenceFailure { iterations }
    }

    /// Create a normalization error
    pub fn normalization_error(norm: f64) -> Self {
        Self::NormalizationError { norm }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::dimension_mismatch(3, 5);
        assert!(err.to_string().contains("3"));
        assert!(err.to_string().contains("5"));
    }
}
