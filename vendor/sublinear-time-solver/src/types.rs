//! Common types and type aliases used throughout the solver.
//!
//! This module defines fundamental types for numerical computations,
//! graph operations, and solver configuration.

use alloc::{string::String, vec::Vec};
use core::fmt;

/// Node identifier for graph-based algorithms.
pub type NodeId = u32;

/// Edge identifier for graph operations.
pub type EdgeId = u32;

/// Floating-point precision type.
/// 
/// Currently fixed to f64 for numerical stability, but may be
/// parameterized in future versions for memory optimization.
pub type Precision = f64;

/// Integer type for array indices and counts.
pub type IndexType = u32;

/// Type for storing matrix/vector dimensions.
pub type DimensionType = usize;

/// Convergence detection modes for iterative solvers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ConvergenceMode {
    /// Check residual norm: ||Ax - b|| < tolerance
    ResidualNorm,
    /// Check relative residual: ||Ax - b|| / ||b|| < tolerance
    RelativeResidual,
    /// Check solution change: ||x_new - x_old|| < tolerance
    SolutionChange,
    /// Check relative solution change: ||x_new - x_old|| / ||x_old|| < tolerance
    RelativeSolutionChange,
    /// Use multiple criteria (most conservative)
    Combined,
}

/// Vector norm types for error measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NormType {
    /// L1 norm (sum of absolute values)
    L1,
    /// L2 norm (Euclidean norm)
    L2,
    /// L∞ norm (maximum absolute value)
    LInfinity,
    /// Weighted norm with custom weights
    Weighted,
}

/// Error bounds for approximate solutions.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ErrorBounds {
    /// Lower bound on the true error
    pub lower_bound: Precision,
    /// Upper bound on the true error
    pub upper_bound: Precision,
    /// Confidence level (0.0 to 1.0) for probabilistic bounds
    pub confidence: Option<Precision>,
    /// Method used to compute the bounds
    pub method: ErrorBoundMethod,
}

/// Methods for computing error bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ErrorBoundMethod {
    /// Deterministic bounds based on matrix properties
    Deterministic,
    /// Probabilistic bounds from random sampling
    Probabilistic,
    /// Adaptive bounds that tighten during iteration
    Adaptive,
    /// Bounds from Neumann series truncation analysis
    NeumannTruncation,
}

/// Comprehensive statistics about solver execution.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SolverStats {
    /// Total wall-clock time for solving
    pub total_time_ms: f64,
    /// Time spent in matrix operations
    pub matrix_ops_time_ms: f64,
    /// Time spent in convergence checking
    pub convergence_check_time_ms: f64,
    /// Number of matrix-vector multiplications performed
    pub matvec_count: usize,
    /// Number of vector operations (add, scale, etc.)
    pub vector_ops_count: usize,
    /// Peak memory usage in bytes
    pub peak_memory_bytes: usize,
    /// Number of cache misses (if available)
    pub cache_misses: Option<usize>,
    /// FLOPS (floating-point operations per second) achieved
    pub flops: Option<f64>,
    /// Whether SIMD optimizations were used
    pub simd_used: bool,
    /// Number of parallel threads used
    pub thread_count: usize,
}

/// Matrix sparsity pattern information.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SparsityInfo {
    /// Total number of non-zero elements
    pub nnz: usize,
    /// Matrix dimensions (rows, cols)
    pub dimensions: (DimensionType, DimensionType),
    /// Sparsity ratio (nnz / (rows * cols))
    pub sparsity_ratio: Precision,
    /// Average number of non-zeros per row
    pub avg_nnz_per_row: Precision,
    /// Maximum number of non-zeros in any row
    pub max_nnz_per_row: usize,
    /// Bandwidth of the matrix
    pub bandwidth: Option<usize>,
    /// Whether the matrix has a banded structure
    pub is_banded: bool,
}

/// Graph connectivity information for push algorithms.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphInfo {
    /// Number of nodes in the graph
    pub node_count: usize,
    /// Number of edges in the graph
    pub edge_count: usize,
    /// Average degree (edges per node)
    pub avg_degree: Precision,
    /// Maximum degree in the graph
    pub max_degree: usize,
    /// Graph diameter (longest shortest path)
    pub diameter: Option<usize>,
    /// Whether the graph is strongly connected
    pub is_strongly_connected: bool,
    /// Number of strongly connected components
    pub scc_count: usize,
}

/// Matrix conditioning information.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ConditioningInfo {
    /// Estimated condition number
    pub condition_number: Option<Precision>,
    /// Whether matrix is diagonally dominant
    pub is_diagonally_dominant: bool,
    /// Diagonal dominance factor (minimum ratio)
    pub diagonal_dominance_factor: Option<Precision>,
    /// Spectral radius estimate
    pub spectral_radius: Option<Precision>,
    /// Whether matrix is positive definite
    pub is_positive_definite: Option<bool>,
}

/// Algorithm selection hints based on problem characteristics.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlgorithmHints {
    /// Recommended primary algorithm
    pub primary_algorithm: String,
    /// Alternative algorithms in order of preference
    pub alternative_algorithms: Vec<String>,
    /// Confidence in the recommendation (0.0 to 1.0)
    pub confidence: Precision,
    /// Reasoning for the recommendation
    pub reasoning: Vec<String>,
}

/// Update operation for incremental solving.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DeltaUpdate {
    /// Indices of updated elements
    pub indices: Vec<IndexType>,
    /// New values for the updated elements
    pub values: Vec<Precision>,
    /// Timestamp of the update
    pub timestamp: u64,
    /// Update sequence number for ordering
    pub sequence_number: u64,
}

/// Streaming solution chunk for real-time applications.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SolutionChunk {
    /// Iteration number when this chunk was produced
    pub iteration: usize,
    /// Partial solution values (sparse representation)
    pub values: Vec<(IndexType, Precision)>,
    /// Current residual norm
    pub residual_norm: Precision,
    /// Whether the solution has converged
    pub converged: bool,
    /// Estimated remaining iterations
    pub estimated_remaining_iterations: Option<usize>,
    /// Timestamp when chunk was generated
    pub timestamp: u64,
}

/// Memory usage tracking information.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemoryInfo {
    /// Current memory usage in bytes
    pub current_usage_bytes: usize,
    /// Peak memory usage in bytes
    pub peak_usage_bytes: usize,
    /// Memory allocated for matrix storage
    pub matrix_memory_bytes: usize,
    /// Memory allocated for vectors
    pub vector_memory_bytes: usize,
    /// Memory allocated for temporary workspace
    pub workspace_memory_bytes: usize,
    /// Number of memory allocations
    pub allocation_count: usize,
    /// Number of memory deallocations
    pub deallocation_count: usize,
}

/// Performance profiling data.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileData {
    /// Function name or operation description
    pub operation: String,
    /// Number of times this operation was called
    pub call_count: usize,
    /// Total time spent in this operation (microseconds)
    pub total_time_us: u64,
    /// Average time per call (microseconds)
    pub avg_time_us: f64,
    /// Minimum time for a single call (microseconds)
    pub min_time_us: u64,
    /// Maximum time for a single call (microseconds)
    pub max_time_us: u64,
    /// Percentage of total execution time
    pub time_percentage: f64,
}

impl ErrorBounds {
    /// Create error bounds with only an upper bound.
    pub fn upper_bound_only(upper: Precision, method: ErrorBoundMethod) -> Self {
        Self {
            lower_bound: 0.0,
            upper_bound: upper,
            confidence: None,
            method,
        }
    }
    
    /// Create deterministic error bounds.
    pub fn deterministic(lower: Precision, upper: Precision) -> Self {
        Self {
            lower_bound: lower,
            upper_bound: upper,
            confidence: None,
            method: ErrorBoundMethod::Deterministic,
        }
    }
    
    /// Create probabilistic error bounds with confidence level.
    pub fn probabilistic(lower: Precision, upper: Precision, confidence: Precision) -> Self {
        Self {
            lower_bound: lower,
            upper_bound: upper,
            confidence: Some(confidence.clamp(0.0, 1.0)),
            method: ErrorBoundMethod::Probabilistic,
        }
    }
    
    /// Check if the bounds are valid (lower <= upper).
    pub fn is_valid(&self) -> bool {
        self.lower_bound <= self.upper_bound && 
        self.lower_bound >= 0.0 && 
        self.upper_bound >= 0.0
    }
    
    /// Get the width of the error bounds.
    pub fn width(&self) -> Precision {
        self.upper_bound - self.lower_bound
    }
    
    /// Get the midpoint of the error bounds.
    pub fn midpoint(&self) -> Precision {
        (self.lower_bound + self.upper_bound) / 2.0
    }
}

impl SolverStats {
    /// Create a new empty statistics object.
    pub fn new() -> Self {
        Self {
            total_time_ms: 0.0,
            matrix_ops_time_ms: 0.0,
            convergence_check_time_ms: 0.0,
            matvec_count: 0,
            vector_ops_count: 0,
            peak_memory_bytes: 0,
            cache_misses: None,
            flops: None,
            simd_used: false,
            thread_count: 1,
        }
    }
    
    /// Calculate matrix operations percentage of total time.
    pub fn matrix_ops_percentage(&self) -> f64 {
        if self.total_time_ms > 0.0 {
            (self.matrix_ops_time_ms / self.total_time_ms) * 100.0
        } else {
            0.0
        }
    }
    
    /// Calculate convergence checking percentage of total time.
    pub fn convergence_percentage(&self) -> f64 {
        if self.total_time_ms > 0.0 {
            (self.convergence_check_time_ms / self.total_time_ms) * 100.0
        } else {
            0.0
        }
    }
}

impl Default for SolverStats {
    fn default() -> Self {
        Self::new()
    }
}

impl SparsityInfo {
    /// Create sparsity information from basic matrix data.
    pub fn new(nnz: usize, rows: DimensionType, cols: DimensionType) -> Self {
        let total_elements = rows * cols;
        let sparsity_ratio = if total_elements > 0 {
            nnz as Precision / total_elements as Precision
        } else {
            0.0
        };
        
        let avg_nnz_per_row = if rows > 0 {
            nnz as Precision / rows as Precision
        } else {
            0.0
        };
        
        Self {
            nnz,
            dimensions: (rows, cols),
            sparsity_ratio,
            avg_nnz_per_row,
            max_nnz_per_row: 0, // To be computed separately
            bandwidth: None,
            is_banded: false,
        }
    }
    
    /// Check if the matrix is considered sparse (< 10% non-zero).
    pub fn is_sparse(&self) -> bool {
        self.sparsity_ratio < 0.1
    }
    
    /// Check if the matrix is very sparse (< 1% non-zero).
    pub fn is_very_sparse(&self) -> bool {
        self.sparsity_ratio < 0.01
    }
}

impl fmt::Display for ConvergenceMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConvergenceMode::ResidualNorm => write!(f, "residual_norm"),
            ConvergenceMode::RelativeResidual => write!(f, "relative_residual"),
            ConvergenceMode::SolutionChange => write!(f, "solution_change"),
            ConvergenceMode::RelativeSolutionChange => write!(f, "relative_solution_change"),
            ConvergenceMode::Combined => write!(f, "combined"),
        }
    }
}

impl fmt::Display for NormType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NormType::L1 => write!(f, "L1"),
            NormType::L2 => write!(f, "L2"),
            NormType::LInfinity => write!(f, "L∞"),
            NormType::Weighted => write!(f, "weighted"),
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_bounds_validity() {
        let valid_bounds = ErrorBounds::deterministic(1.0, 2.0);
        assert!(valid_bounds.is_valid());
        assert_eq!(valid_bounds.width(), 1.0);
        assert_eq!(valid_bounds.midpoint(), 1.5);
        
        let invalid_bounds = ErrorBounds {
            lower_bound: 2.0,
            upper_bound: 1.0,
            confidence: None,
            method: ErrorBoundMethod::Deterministic,
        };
        assert!(!invalid_bounds.is_valid());
    }
    
    #[test]
    fn test_sparsity_info() {
        let info = SparsityInfo::new(100, 1000, 1000);
        assert_eq!(info.sparsity_ratio, 0.0001);
        assert!(info.is_very_sparse());
        assert!(info.is_sparse());
        assert_eq!(info.avg_nnz_per_row, 0.1);
    }
    
    #[test]
    fn test_solver_stats_percentages() {
        let mut stats = SolverStats::new();
        stats.total_time_ms = 100.0;
        stats.matrix_ops_time_ms = 60.0;
        stats.convergence_check_time_ms = 10.0;
        
        assert_eq!(stats.matrix_ops_percentage(), 60.0);
        assert_eq!(stats.convergence_percentage(), 10.0);
    }
}