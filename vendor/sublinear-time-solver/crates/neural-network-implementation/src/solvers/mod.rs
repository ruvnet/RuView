//! Sublinear solver integration and supporting components
//!
//! This module provides the key innovation of the temporal neural network:
//! integration with sublinear-time mathematical solvers for prediction
//! verification and Kalman filter priors.

use crate::error::{Result, TemporalNeuralError};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

pub mod kalman;
// pub mod solver_gate; // Temporarily disabled
pub mod solver_gate_simple;
pub mod pagerank_selector;

pub use kalman::KalmanFilter;
// pub use solver_gate::SolverGate;
pub use solver_gate_simple::{SolverGate, SolverGateConfig, GateResult, SolverGateStats};
pub use pagerank_selector::PageRankSelector;

/// Trait for components that can be prepared for inference
pub trait InferenceReadyTrait {
    /// Prepare the component for inference mode
    fn prepare_for_inference(&mut self) -> Result<()>;

    /// Check if the component is ready for inference
    fn is_inference_ready(&self) -> bool;

    /// Get memory usage in bytes
    fn memory_usage(&self) -> usize;

    /// Reset component state
    fn reset(&mut self) -> Result<()>;
}

/// Common mathematical utilities used by solver components
pub mod math_utils {
    use nalgebra::{DMatrix, DVector};

    /// Compute Jacobian matrix numerically using finite differences
    pub fn compute_jacobian<F>(
        f: F,
        x: &DVector<f64>,
        h: f64,
    ) -> nalgebra::DMatrix<f64>
    where
        F: Fn(&DVector<f64>) -> DVector<f64>,
    {
        let n = x.len();
        let fx = f(x);
        let m = fx.len();
        let mut jacobian = DMatrix::zeros(m, n);

        for j in 0..n {
            let mut x_plus = x.clone();
            x_plus[j] += h;
            let fx_plus = f(&x_plus);

            for i in 0..m {
                jacobian[(i, j)] = (fx_plus[i] - fx[i]) / h;
            }
        }

        jacobian
    }

    /// Compute matrix condition number estimate
    pub fn condition_number_estimate(matrix: &DMatrix<f64>) -> f64 {
        // Simple estimate using ratio of max to min singular values
        // In practice, use proper SVD
        let max_elem = matrix.iter().map(|x| x.abs()).fold(0.0, f64::max);
        let min_elem = matrix.iter()
            .filter(|&&x| x.abs() > 1e-12)
            .map(|x| x.abs())
            .fold(f64::INFINITY, f64::min);

        if min_elem.is_infinite() || min_elem == 0.0 {
            f64::INFINITY
        } else {
            max_elem / min_elem
        }
    }

    /// Check if matrix is diagonally dominant
    pub fn is_diagonally_dominant(matrix: &DMatrix<f64>) -> bool {
        let (rows, cols) = matrix.shape();
        if rows != cols {
            return false;
        }

        for i in 0..rows {
            let diagonal_elem = matrix[(i, i)].abs();
            let off_diagonal_sum: f64 = (0..cols)
                .filter(|&j| j != i)
                .map(|j| matrix[(i, j)].abs())
                .sum();

            if diagonal_elem <= off_diagonal_sum {
                return false;
            }
        }

        true
    }

    /// Create a simple test matrix that is diagonally dominant
    pub fn create_test_dd_matrix(size: usize) -> DMatrix<f64> {
        let mut matrix = DMatrix::zeros(size, size);

        for i in 0..size {
            // Set diagonal elements to be larger than sum of off-diagonal
            let mut off_diag_sum = 0.0;
            for j in 0..size {
                if i != j {
                    let val = (i + j + 1) as f64 * 0.1;
                    matrix[(i, j)] = val;
                    off_diag_sum += val.abs();
                }
            }
            matrix[(i, i)] = off_diag_sum * 1.5 + 1.0; // Ensure diagonal dominance
        }

        matrix
    }

    /// Spectral radius estimation using power iteration
    pub fn spectral_radius_estimate(matrix: &DMatrix<f64>, max_iterations: usize) -> f64 {
        let n = matrix.nrows();
        if n != matrix.ncols() {
            return f64::NAN;
        }

        let mut v = DVector::from_vec((0..n).map(|_| rand::random::<f64>()).collect());
        v /= v.norm();

        let mut lambda = 0.0;

        for _ in 0..max_iterations {
            let new_v = matrix * &v;
            lambda = v.dot(&new_v);
            v = new_v;
            if v.norm() > 1e-10 {
                v /= v.norm();
            } else {
                break;
            }
        }

        lambda.abs()
    }
}

/// Certificate information from solver verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    /// Estimated error bound
    pub error_bound: f64,
    /// Confidence level (0.0 to 1.0)
    pub confidence: f64,
    /// Computational work performed
    pub work_performed: u64,
    /// Solver algorithm used
    pub algorithm: String,
    /// Whether the certificate is valid
    pub is_valid: bool,
    /// Additional metadata
    pub metadata: CertificateMetadata,
}

/// Additional metadata for certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateMetadata {
    /// Matrix condition number
    pub condition_number: Option<f64>,
    /// Whether matrix was diagonally dominant
    pub diagonally_dominant: bool,
    /// Convergence iterations performed
    pub iterations: u32,
    /// Final residual norm
    pub residual_norm: f64,
    /// Computation time in microseconds
    pub computation_time_us: f64,
}

impl Certificate {
    /// Create a new certificate
    pub fn new(
        error_bound: f64,
        confidence: f64,
        work_performed: u64,
        algorithm: String,
    ) -> Self {
        Self {
            error_bound,
            confidence,
            work_performed,
            algorithm,
            is_valid: error_bound >= 0.0 && confidence >= 0.0 && confidence <= 1.0,
            metadata: CertificateMetadata {
                condition_number: None,
                diagonally_dominant: false,
                iterations: 0,
                residual_norm: 0.0,
                computation_time_us: 0.0,
            },
        }
    }

    /// Check if certificate passes given tolerance
    pub fn passes_tolerance(&self, tolerance: f64) -> bool {
        self.is_valid && self.error_bound <= tolerance
    }

    /// Get quality score (0.0 to 1.0, higher is better)
    pub fn quality_score(&self) -> f64 {
        if !self.is_valid {
            return 0.0;
        }

        // Combine error bound (lower is better) and confidence (higher is better)
        let error_score = 1.0 / (1.0 + self.error_bound);
        let confidence_score = self.confidence;

        (error_score + confidence_score) / 2.0
    }
}

/// Factory for creating solver components
pub struct SolverFactory;

impl SolverFactory {
    /// Create a Kalman filter with the given configuration
    pub fn create_kalman_filter(config: &crate::config::KalmanConfig) -> Result<KalmanFilter> {
        KalmanFilter::new(config)
    }

    /// Create a solver gate with the given configuration
    pub fn create_solver_gate(config: &SolverGateConfig) -> Result<SolverGate> {
        SolverGate::new(config)
    }

    /// Create a PageRank selector with the given configuration
    pub fn create_pagerank_selector(
        config: &crate::config::ActiveSelectionConfig,
    ) -> Result<PageRankSelector> {
        PageRankSelector::new(config)
    }
}

/// Performance monitoring for solver components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverPerformanceMetrics {
    /// Average prediction latency in microseconds
    pub avg_latency_us: f64,
    /// P50 latency in microseconds
    pub p50_latency_us: f64,
    /// P99 latency in microseconds
    pub p99_latency_us: f64,
    /// P99.9 latency in microseconds
    pub p99_9_latency_us: f64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// Total predictions made
    pub total_predictions: u64,
    /// Average certificate error
    pub avg_certificate_error: f64,
    /// Gate pass rate
    pub gate_pass_rate: f64,
}

impl Default for SolverPerformanceMetrics {
    fn default() -> Self {
        Self {
            avg_latency_us: 0.0,
            p50_latency_us: 0.0,
            p99_latency_us: 0.0,
            p99_9_latency_us: 0.0,
            success_rate: 1.0,
            memory_usage_bytes: 0,
            total_predictions: 0,
            avg_certificate_error: 0.0,
            gate_pass_rate: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::math_utils::*;

    #[test]
    fn test_diagonal_dominance() {
        let dd_matrix = create_test_dd_matrix(3);
        assert!(is_diagonally_dominant(&dd_matrix));

        // Test non-diagonally dominant matrix
        let non_dd = DMatrix::from_row_slice(2, 2, &[1.0, 2.0, 3.0, 1.0]);
        assert!(!is_diagonally_dominant(&non_dd));
    }

    #[test]
    fn test_certificate_validation() {
        let cert = Certificate::new(0.01, 0.95, 1000, "neumann".to_string());
        assert!(cert.is_valid);
        assert!(cert.passes_tolerance(0.02));
        assert!(!cert.passes_tolerance(0.005));

        let quality = cert.quality_score();
        assert!(quality > 0.0 && quality <= 1.0);
    }

    #[test]
    fn test_condition_number() {
        let well_conditioned = DMatrix::identity(3, 3);
        let cond = condition_number_estimate(&well_conditioned);
        assert!(cond < 2.0); // Should be close to 1.0

        let ill_conditioned = DMatrix::from_row_slice(2, 2, &[1.0, 1.0, 1.0, 1.0001]);
        let cond_ill = condition_number_estimate(&ill_conditioned);
        assert!(cond_ill > 1000.0);
    }

    #[test]
    fn test_jacobian_computation() {
        // Test with simple linear function f(x) = Ax
        let a = DMatrix::from_row_slice(2, 2, &[2.0, 1.0, 0.5, 3.0]);
        let f = |x: &DVector<f64>| &a * x;

        let x = DVector::from_vec(vec![1.0, 2.0]);
        let jac = compute_jacobian(f, &x, 1e-6);

        // Jacobian should be approximately equal to A
        assert!((jac[(0, 0)] - 2.0).abs() < 1e-4);
        assert!((jac[(0, 1)] - 1.0).abs() < 1e-4);
        assert!((jac[(1, 0)] - 0.5).abs() < 1e-4);
        assert!((jac[(1, 1)] - 3.0).abs() < 1e-4);
    }
}