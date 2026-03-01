//! Error types for ruvector-math

use thiserror::Error;

/// Result type alias for ruvector-math operations
pub type Result<T> = std::result::Result<T, MathError>;

/// Errors that can occur in mathematical operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MathError {
    /// Dimension mismatch between inputs
    #[error("Dimension mismatch: expected {expected}, got {got}")]
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension received
        got: usize,
    },

    /// Empty input where non-empty was required
    #[error("Empty input: {context}")]
    EmptyInput {
        /// Context describing what was empty
        context: String,
    },

    /// Numerical instability detected
    #[error("Numerical instability: {message}")]
    NumericalInstability {
        /// Description of the instability
        message: String,
    },

    /// Convergence failure in iterative algorithm
    #[error("Convergence failed after {iterations} iterations (residual: {residual:.2e})")]
    ConvergenceFailure {
        /// Number of iterations attempted
        iterations: usize,
        /// Final residual/error value
        residual: f64,
    },

    /// Invalid parameter value
    #[error("Invalid parameter '{name}': {reason}")]
    InvalidParameter {
        /// Parameter name
        name: String,
        /// Reason why it's invalid
        reason: String,
    },

    /// Point not on manifold
    #[error("Point not on manifold: {message}")]
    NotOnManifold {
        /// Description of the constraint violation
        message: String,
    },

    /// Singular matrix encountered
    #[error("Singular matrix encountered: {context}")]
    SingularMatrix {
        /// Context where singularity occurred
        context: String,
    },

    /// Curvature constraint violated
    #[error("Curvature constraint violated: {message}")]
    CurvatureViolation {
        /// Description of the violation
        message: String,
    },
}

impl MathError {
    /// Create a dimension mismatch error
    pub fn dimension_mismatch(expected: usize, got: usize) -> Self {
        Self::DimensionMismatch { expected, got }
    }

    /// Create an empty input error
    pub fn empty_input(context: impl Into<String>) -> Self {
        Self::EmptyInput {
            context: context.into(),
        }
    }

    /// Create a numerical instability error
    pub fn numerical_instability(message: impl Into<String>) -> Self {
        Self::NumericalInstability {
            message: message.into(),
        }
    }

    /// Create a convergence failure error
    pub fn convergence_failure(iterations: usize, residual: f64) -> Self {
        Self::ConvergenceFailure {
            iterations,
            residual,
        }
    }

    /// Create an invalid parameter error
    pub fn invalid_parameter(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidParameter {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Create a not on manifold error
    pub fn not_on_manifold(message: impl Into<String>) -> Self {
        Self::NotOnManifold {
            message: message.into(),
        }
    }

    /// Create a singular matrix error
    pub fn singular_matrix(context: impl Into<String>) -> Self {
        Self::SingularMatrix {
            context: context.into(),
        }
    }

    /// Create a curvature violation error
    pub fn curvature_violation(message: impl Into<String>) -> Self {
        Self::CurvatureViolation {
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = MathError::dimension_mismatch(128, 64);
        assert!(err.to_string().contains("128"));
        assert!(err.to_string().contains("64"));
    }

    #[test]
    fn test_convergence_error() {
        let err = MathError::convergence_failure(100, 1e-3);
        assert!(err.to_string().contains("100"));
    }
}
