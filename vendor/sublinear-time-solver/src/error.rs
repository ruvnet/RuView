//! Error types and handling for the sublinear solver.
//!
//! This module defines all error conditions that can occur during matrix operations
//! and solver execution, providing detailed error information for debugging and
//! recovery strategies.

use core::fmt;
use alloc::{string::String, vec::Vec};

/// Result type alias for solver operations.
pub type Result<T> = core::result::Result<T, SolverError>;

/// Comprehensive error type for all solver operations.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SolverError {
    /// Matrix is not diagonally dominant, which is required for convergence guarantees.
    MatrixNotDiagonallyDominant {
        /// The row where diagonal dominance fails
        row: usize,
        /// Diagonal element value
        diagonal: f64,
        /// Sum of off-diagonal absolute values
        off_diagonal_sum: f64,
    },
    
    /// Numerical instability detected during computation.
    NumericalInstability {
        /// Description of the instability
        reason: String,
        /// Iteration where instability was detected
        iteration: usize,
        /// Current residual norm when instability occurred
        residual_norm: f64,
    },
    
    /// Algorithm failed to converge within specified iterations.
    ConvergenceFailure {
        /// Number of iterations performed
        iterations: usize,
        /// Final residual norm achieved
        residual_norm: f64,
        /// Target tolerance that wasn't reached
        tolerance: f64,
        /// Algorithm that failed to converge
        algorithm: String,
    },
    
    /// Invalid input parameters or data.
    InvalidInput {
        /// Description of the invalid input
        message: String,
        /// Optional parameter name that was invalid
        parameter: Option<String>,
    },
    
    /// Dimension mismatch between matrix and vector operations.
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension found
        actual: usize,
        /// Context where mismatch occurred
        operation: String,
    },
    
    /// Matrix format is not supported for the requested operation.
    UnsupportedMatrixFormat {
        /// Current matrix format
        current_format: String,
        /// Required format for the operation
        required_format: String,
        /// Operation that was attempted
        operation: String,
    },
    
    /// Memory allocation failure.
    MemoryAllocationError {
        /// Requested allocation size in bytes
        requested_size: usize,
        /// Available memory at time of failure (if known)
        available_memory: Option<usize>,
    },
    
    /// Index out of bounds for matrix or vector access.
    IndexOutOfBounds {
        /// The invalid index
        index: usize,
        /// Maximum valid index
        max_index: usize,
        /// Context where out-of-bounds access occurred
        context: String,
    },
    
    /// Sparse matrix contains invalid data.
    InvalidSparseMatrix {
        /// Description of the invalid data
        reason: String,
        /// Position where invalid data was found
        position: Option<(usize, usize)>,
    },
    
    /// Algorithm-specific error conditions.
    AlgorithmError {
        /// Name of the algorithm
        algorithm: String,
        /// Specific error message
        message: String,
        /// Additional context data
        context: Vec<(String, String)>,
    },
    
    /// WebAssembly binding error (when WASM feature is enabled).
    #[cfg(feature = "wasm")]
    WasmBindingError {
        /// Error message from WASM binding
        message: String,
        /// JavaScript error if available
        js_error: Option<String>,
    },
    
    /// I/O error for file operations (when std feature is enabled).
    #[cfg(feature = "std")]
    IoError {
        /// I/O error description
        #[cfg_attr(feature = "serde", serde(skip))]
        message: String,
        /// Context where I/O error occurred
        context: String,
    },
    
    /// Serialization/deserialization error.
    #[cfg(feature = "serde")]
    SerializationError {
        /// Error message from serialization
        message: String,
        /// Data type being serialized
        data_type: String,
    },
}

impl SolverError {
    /// Check if this error indicates a recoverable condition.
    /// 
    /// Recoverable errors can potentially be resolved by adjusting
    /// algorithm parameters or switching to a different solver.
    pub fn is_recoverable(&self) -> bool {
        match self {
            SolverError::ConvergenceFailure { .. } => true,
            SolverError::NumericalInstability { .. } => true,
            SolverError::MatrixNotDiagonallyDominant { .. } => false, // Fundamental issue
            SolverError::InvalidInput { .. } => false, // User error
            SolverError::DimensionMismatch { .. } => false, // User error
            SolverError::MemoryAllocationError { .. } => false, // System limitation
            SolverError::IndexOutOfBounds { .. } => false, // Programming error
            SolverError::InvalidSparseMatrix { .. } => false, // Data corruption
            SolverError::UnsupportedMatrixFormat { .. } => true, // Can convert format
            SolverError::AlgorithmError { .. } => true, // Algorithm-specific, might recover
            #[cfg(feature = "wasm")]
            SolverError::WasmBindingError { .. } => false, // Runtime environment issue
            #[cfg(feature = "std")]
            SolverError::IoError { .. } => false, // External system issue
            #[cfg(feature = "serde")]
            SolverError::SerializationError { .. } => false, // Data format issue
        }
    }
    
    /// Get suggested recovery strategy for recoverable errors.
    pub fn recovery_strategy(&self) -> Option<RecoveryStrategy> {
        match self {
            SolverError::ConvergenceFailure { algorithm, .. } => {
                // Suggest alternative algorithms
                Some(match algorithm.as_str() {
                    "neumann" => RecoveryStrategy::SwitchAlgorithm("hybrid".to_string()),
                    "forward_push" => RecoveryStrategy::SwitchAlgorithm("backward_push".to_string()),
                    "backward_push" => RecoveryStrategy::SwitchAlgorithm("hybrid".to_string()),
                    _ => RecoveryStrategy::RelaxTolerance(10.0),
                })
            },
            SolverError::NumericalInstability { .. } => {
                Some(RecoveryStrategy::IncreasePrecision)
            },
            SolverError::UnsupportedMatrixFormat { required_format, .. } => {
                Some(RecoveryStrategy::ConvertMatrixFormat(required_format.clone()))
            },
            SolverError::AlgorithmError { algorithm, .. } => {
                Some(RecoveryStrategy::SwitchAlgorithm("neumann".to_string()))
            },
            _ => None,
        }
    }
    
    /// Get the error severity level.
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            SolverError::MemoryAllocationError { .. } => ErrorSeverity::Critical,
            SolverError::InvalidSparseMatrix { .. } => ErrorSeverity::Critical,
            SolverError::IndexOutOfBounds { .. } => ErrorSeverity::Critical,
            SolverError::MatrixNotDiagonallyDominant { .. } => ErrorSeverity::High,
            SolverError::ConvergenceFailure { .. } => ErrorSeverity::Medium,
            SolverError::NumericalInstability { .. } => ErrorSeverity::Medium,
            SolverError::InvalidInput { .. } => ErrorSeverity::Medium,
            SolverError::DimensionMismatch { .. } => ErrorSeverity::Medium,
            SolverError::UnsupportedMatrixFormat { .. } => ErrorSeverity::Low,
            SolverError::AlgorithmError { .. } => ErrorSeverity::Medium,
            #[cfg(feature = "wasm")]
            SolverError::WasmBindingError { .. } => ErrorSeverity::High,
            #[cfg(feature = "std")]
            SolverError::IoError { .. } => ErrorSeverity::Medium,
            #[cfg(feature = "serde")]
            SolverError::SerializationError { .. } => ErrorSeverity::Low,
        }
    }
}

/// Recovery strategies for recoverable errors.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RecoveryStrategy {
    /// Switch to a different solver algorithm.
    SwitchAlgorithm(String),
    /// Increase numerical precision (f32 -> f64).
    IncreasePrecision,
    /// Relax convergence tolerance by the given factor.
    RelaxTolerance(f64),
    /// Restart with different random seed.
    RestartWithDifferentSeed,
    /// Convert matrix to a different storage format.
    ConvertMatrixFormat(String),
    /// Increase maximum iteration count.
    IncreaseIterations(usize),
}

/// Error severity levels for logging and monitoring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorSeverity {
    /// Low severity - algorithm can continue with degraded performance
    Low,
    /// Medium severity - operation failed but system remains stable
    Medium,
    /// High severity - significant failure requiring user intervention
    High,
    /// Critical severity - system integrity compromised
    Critical,
}

impl fmt::Display for SolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverError::MatrixNotDiagonallyDominant { row, diagonal, off_diagonal_sum } => {
                write!(f, "Matrix is not diagonally dominant at row {}: diagonal = {:.6}, off-diagonal sum = {:.6}", 
                       row, diagonal, off_diagonal_sum)
            },
            SolverError::NumericalInstability { reason, iteration, residual_norm } => {
                write!(f, "Numerical instability at iteration {}: {} (residual = {:.2e})", 
                       iteration, reason, residual_norm)
            },
            SolverError::ConvergenceFailure { iterations, residual_norm, tolerance, algorithm } => {
                write!(f, "Algorithm '{}' failed to converge after {} iterations: residual = {:.2e} > tolerance = {:.2e}", 
                       algorithm, iterations, residual_norm, tolerance)
            },
            SolverError::InvalidInput { message, parameter } => {
                match parameter {
                    Some(param) => write!(f, "Invalid input for parameter '{}': {}", param, message),
                    None => write!(f, "Invalid input: {}", message),
                }
            },
            SolverError::DimensionMismatch { expected, actual, operation } => {
                write!(f, "Dimension mismatch in {}: expected {}, got {}", operation, expected, actual)
            },
            SolverError::UnsupportedMatrixFormat { current_format, required_format, operation } => {
                write!(f, "Operation '{}' requires {} format, but matrix is in {} format", 
                       operation, required_format, current_format)
            },
            SolverError::MemoryAllocationError { requested_size, available_memory } => {
                match available_memory {
                    Some(available) => write!(f, "Memory allocation failed: requested {} bytes, {} available", 
                                             requested_size, available),
                    None => write!(f, "Memory allocation failed: requested {} bytes", requested_size),
                }
            },
            SolverError::IndexOutOfBounds { index, max_index, context } => {
                write!(f, "Index {} out of bounds in {}: maximum valid index is {}", 
                       index, context, max_index)
            },
            SolverError::InvalidSparseMatrix { reason, position } => {
                match position {
                    Some((row, col)) => write!(f, "Invalid sparse matrix at ({}, {}): {}", row, col, reason),
                    None => write!(f, "Invalid sparse matrix: {}", reason),
                }
            },
            SolverError::AlgorithmError { algorithm, message, .. } => {
                write!(f, "Algorithm '{}' error: {}", algorithm, message)
            },
            #[cfg(feature = "wasm")]
            SolverError::WasmBindingError { message, js_error } => {
                match js_error {
                    Some(js_err) => write!(f, "WASM binding error: {} (JS: {})", message, js_err),
                    None => write!(f, "WASM binding error: {}", message),
                }
            },
            #[cfg(feature = "std")]
            SolverError::IoError { message, context } => {
                write!(f, "I/O error in {}: {}", context, message)
            },
            #[cfg(feature = "serde")]
            SolverError::SerializationError { message, data_type } => {
                write!(f, "Serialization error for {}: {}", data_type, message)
            },
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SolverError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

// Conversion from standard library errors
#[cfg(feature = "std")]
impl From<std::io::Error> for SolverError {
    fn from(err: std::io::Error) -> Self {
        SolverError::IoError {
            message: err.to_string(),
            context: "File operation".to_string(),
        }
    }
}

// Conversion for WASM environments
#[cfg(feature = "wasm")]
impl From<wasm_bindgen::JsValue> for SolverError {
    fn from(err: wasm_bindgen::JsValue) -> Self {
        let message = if let Some(string) = err.as_string() {
            string
        } else {
            "Unknown JavaScript error".to_string()
        };
        
        SolverError::WasmBindingError {
            message,
            js_error: None,
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_recoverability() {
        let convergence_error = SolverError::ConvergenceFailure {
            iterations: 100,
            residual_norm: 1e-3,
            tolerance: 1e-6,
            algorithm: "neumann".to_string(),
        };
        assert!(convergence_error.is_recoverable());
        
        let dimension_error = SolverError::DimensionMismatch {
            expected: 100,
            actual: 50,
            operation: "matrix_vector_multiply".to_string(),
        };
        assert!(!dimension_error.is_recoverable());
    }
    
    #[test]
    fn test_recovery_strategies() {
        let error = SolverError::ConvergenceFailure {
            iterations: 100,
            residual_norm: 1e-3,
            tolerance: 1e-6,
            algorithm: "neumann".to_string(),
        };
        
        if let Some(RecoveryStrategy::SwitchAlgorithm(algo)) = error.recovery_strategy() {
            assert_eq!(algo, "hybrid");
        } else {
            panic!("Expected SwitchAlgorithm recovery strategy");
        }
    }
    
    #[test]
    fn test_error_severity() {
        let memory_error = SolverError::MemoryAllocationError {
            requested_size: 1000000,
            available_memory: None,
        };
        assert_eq!(memory_error.severity(), ErrorSeverity::Critical);
        
        let convergence_error = SolverError::ConvergenceFailure {
            iterations: 100,
            residual_norm: 1e-3,
            tolerance: 1e-6,
            algorithm: "neumann".to_string(),
        };
        assert_eq!(convergence_error.severity(), ErrorSeverity::Medium);
    }
}