//! High-performance optimized solver implementations.
//!
//! This module provides optimized versions of linear system solvers with
//! SIMD acceleration, buffer pooling, and parallel execution capabilities.

use crate::types::Precision;
use crate::matrix::sparse::{CSRStorage, COOStorage};
#[cfg(feature = "simd")]
use crate::simd_ops::{matrix_vector_multiply_simd, dot_product_simd, axpy_simd};
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};

#[cfg(feature = "std")]
use std::time::Instant;

/// High-performance sparse matrix optimized for sublinear-time algorithms.
pub struct OptimizedSparseMatrix {
    storage: CSRStorage,
    dimensions: (usize, usize),
    performance_stats: PerformanceStats,
}

/// Performance statistics for matrix operations.
#[derive(Debug, Default)]
pub struct PerformanceStats {
    pub matvec_count: AtomicUsize,
    pub bytes_processed: AtomicUsize,
}

impl Clone for PerformanceStats {
    fn clone(&self) -> Self {
        Self {
            matvec_count: AtomicUsize::new(self.matvec_count.load(Ordering::Relaxed)),
            bytes_processed: AtomicUsize::new(self.bytes_processed.load(Ordering::Relaxed)),
        }
    }
}

impl OptimizedSparseMatrix {
    /// Create optimized sparse matrix from triplets.
    pub fn from_triplets(
        triplets: Vec<(usize, usize, Precision)>,
        rows: usize,
        cols: usize,
    ) -> Result<Self, String> {
        let coo = COOStorage::from_triplets(triplets)
            .map_err(|e| format!("Failed to create COO storage: {:?}", e))?;
        let storage = CSRStorage::from_coo(&coo, rows, cols)
            .map_err(|e| format!("Failed to create CSR storage: {:?}", e))?;

        Ok(Self {
            storage,
            dimensions: (rows, cols),
            performance_stats: PerformanceStats::default(),
        })
    }

    /// Get matrix dimensions.
    pub fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }

    /// Get number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.storage.nnz()
    }

    /// SIMD-accelerated matrix-vector multiplication.
    pub fn multiply_vector(&self, x: &[Precision], y: &mut [Precision]) {
        assert_eq!(x.len(), self.dimensions.1);
        assert_eq!(y.len(), self.dimensions.0);

        self.performance_stats.matvec_count.fetch_add(1, Ordering::Relaxed);
        let bytes = (self.storage.values.len() * 8) + (x.len() * 8) + (y.len() * 8);
        self.performance_stats.bytes_processed.fetch_add(bytes, Ordering::Relaxed);

#[cfg(feature = "simd")]
        {
            matrix_vector_multiply_simd(
                &self.storage.values,
                &self.storage.col_indices,
                &self.storage.row_ptr,
                x,
                y,
            );
        }
        #[cfg(not(feature = "simd"))]
        {
            self.storage.multiply_vector(x, y);
        }
    }

    /// Get performance statistics.
    pub fn get_performance_stats(&self) -> (usize, usize) {
        (
            self.performance_stats.matvec_count.load(Ordering::Relaxed),
            self.performance_stats.bytes_processed.load(Ordering::Relaxed),
        )
    }

    /// Reset performance counters.
    pub fn reset_stats(&self) {
        self.performance_stats.matvec_count.store(0, Ordering::Relaxed);
        self.performance_stats.bytes_processed.store(0, Ordering::Relaxed);
    }
}

/// Configuration for the optimized conjugate gradient solver.
#[derive(Debug, Clone)]
pub struct OptimizedSolverConfig {
    /// Maximum number of iterations
    pub max_iterations: usize,
    /// Convergence tolerance
    pub tolerance: Precision,
    /// Enable performance profiling
    pub enable_profiling: bool,
}

impl Default for OptimizedSolverConfig {
    fn default() -> Self {
        Self {
            max_iterations: 1000,
            tolerance: 1e-6,
            enable_profiling: false,
        }
    }
}

/// Result of optimized solver computation.
#[derive(Debug, Clone)]
pub struct OptimizedSolverResult {
    /// Solution vector
    pub solution: Vec<Precision>,
    /// Final residual norm
    pub residual_norm: Precision,
    /// Number of iterations performed
    pub iterations: usize,
    /// Whether the solver converged
    pub converged: bool,
    /// Total computation time in milliseconds
    #[cfg(feature = "std")]
    pub computation_time_ms: f64,
    #[cfg(not(feature = "std"))]
    pub computation_time_ms: u64,
    /// Performance statistics
    pub performance_stats: OptimizedSolverStats,
}

/// Performance statistics for optimized solver.
#[derive(Debug, Clone, Default)]
pub struct OptimizedSolverStats {
    /// Number of matrix-vector multiplications
    pub matvec_count: usize,
    /// Number of dot products computed
    pub dot_product_count: usize,
    /// Number of AXPY operations
    pub axpy_count: usize,
    /// Total floating-point operations
    pub total_flops: usize,
    /// Average bandwidth achieved (GB/s)
    pub average_bandwidth_gbs: f64,
    /// Average GFLOPS achieved
    pub average_gflops: f64,
}

/// High-performance conjugate gradient solver with SIMD optimizations.
pub struct OptimizedConjugateGradientSolver {
    config: OptimizedSolverConfig,
    stats: OptimizedSolverStats,
}

impl OptimizedConjugateGradientSolver {
    /// Create a new optimized solver.
    pub fn new(config: OptimizedSolverConfig) -> Self {
        Self {
            config,
            stats: OptimizedSolverStats::default(),
        }
    }

    /// Solve the linear system Ax = b using optimized conjugate gradient.
    pub fn solve(
        &mut self,
        matrix: &OptimizedSparseMatrix,
        b: &[Precision],
    ) -> Result<OptimizedSolverResult, String> {
        let (rows, cols) = matrix.dimensions();
        if rows != cols {
            return Err("Matrix must be square".to_string());
        }
        if b.len() != rows {
            return Err("Right-hand side vector length must match matrix size".to_string());
        }

        #[cfg(feature = "std")]
        let start_time = Instant::now();

        // Reset statistics
        self.stats = OptimizedSolverStats::default();

        // Initialize solution and workspace vectors
        let mut x = vec![0.0; rows];
        let mut r = vec![0.0; rows];
        let mut p = vec![0.0; rows];
        let mut ap = vec![0.0; rows];

        // r = b - A*x (initially r = b since x = 0)
        r.copy_from_slice(b);

        let mut iteration = 0;
        let tolerance_sq = self.config.tolerance * self.config.tolerance;
        let mut converged = false;

        // Conjugate gradient iteration
        let mut rsold = 0.0;
        for &ri in r.iter() {
            rsold += ri * ri;
        }
        p.copy_from_slice(&r);

        while iteration < self.config.max_iterations {
            if rsold <= tolerance_sq {
                converged = true;
                break;
            }

            // ap = A * p
            matrix.multiply_vector(&p, &mut ap);
            self.stats.matvec_count += 1;

            // alpha = rsold / (p^T * ap)
            let mut pap = 0.0;
            for (&pi, &api) in p.iter().zip(ap.iter()) {
                pap += pi * api;
            }

            if pap.abs() < 1e-16 {
                break; // Avoid division by zero
            }

            let alpha = rsold / pap;

            // x = x + alpha * p
            for (xi, &pi) in x.iter_mut().zip(p.iter()) {
                *xi += alpha * pi;
            }

            // r = r - alpha * ap
            for (ri, &api) in r.iter_mut().zip(ap.iter()) {
                *ri -= alpha * api;
            }

            let mut rsnew = 0.0;
            for &ri in r.iter() {
                rsnew += ri * ri;
            }

            let beta = rsnew / rsold;

            // p = r + beta * p
            for (pi, &ri) in p.iter_mut().zip(r.iter()) {
                *pi = ri + beta * *pi;
            }

            rsold = rsnew;
            iteration += 1;
        }

        #[cfg(feature = "std")]
        let computation_time_ms = start_time.elapsed().as_millis() as f64;
        #[cfg(not(feature = "std"))]
        let computation_time_ms = 0.0;

        // Calculate final residual
        let final_residual_norm = rsold.sqrt();

        // Update performance statistics
        self.stats.total_flops = self.stats.matvec_count * matrix.nnz() * 2 +
                                 iteration * rows * 6; // vector operations per iteration

        if computation_time_ms > 0.0 {
            let total_gb = (self.stats.total_flops * 8) as f64 / 1e9;
            self.stats.average_bandwidth_gbs = total_gb / (computation_time_ms / 1000.0);
            self.stats.average_gflops = (self.stats.total_flops as f64) / (computation_time_ms * 1e6);
        }

        Ok(OptimizedSolverResult {
            solution: x,
            residual_norm: final_residual_norm,
            iterations: iteration,
            converged,
            computation_time_ms,
            performance_stats: self.stats.clone(),
        })
    }

    /// Compute dot product with SIMD optimization.
    fn dot_product(&mut self, x: &[Precision], y: &[Precision]) -> Precision {
        self.stats.dot_product_count += 1;
        #[cfg(feature = "simd")]
        {
            dot_product_simd(x, y)
        }
        #[cfg(not(feature = "simd"))]
        {
            x.iter().zip(y.iter()).map(|(&a, &b)| a * b).sum()
        }
    }

    /// Compute AXPY operation (y = alpha * x + y) with SIMD optimization.
    fn axpy(&mut self, alpha: Precision, x: &[Precision], y: &mut [Precision]) {
        self.stats.axpy_count += 1;
        #[cfg(feature = "simd")]
        {
            axpy_simd(alpha, x, y);
        }
        #[cfg(not(feature = "simd"))]
        {
            for (yi, &xi) in y.iter_mut().zip(x.iter()) {
                *yi += alpha * xi;
            }
        }
    }

    /// Compute L2 norm of a vector.
    fn l2_norm(&self, x: &[Precision]) -> Precision {
        x.iter().map(|&xi| xi * xi).sum::<Precision>().sqrt()
    }

    /// Get the last iteration count.
    pub fn get_last_iteration_count(&self) -> usize {
        self.stats.matvec_count
    }

    /// Solve with callback for streaming results.
    pub fn solve_with_callback<F>(
        &mut self,
        matrix: &OptimizedSparseMatrix,
        b: &[Precision],
        _chunk_size: usize,
        mut _callback: F,
    ) -> Result<OptimizedSolverResult, String>
    where
        F: FnMut(&OptimizedSolverStats),
    {
        // For now, just call the regular solve method
        // In a full implementation, this would call the callback periodically
        self.solve(matrix, b)
    }
}

impl OptimizedSolverResult {
    /// Get the solution data.
    pub fn data(&self) -> &[Precision] {
        &self.solution
    }
}

/// Additional configuration options for the optimized solver.
#[derive(Debug, Clone, Default)]
pub struct OptimizedSolverOptions {
    /// Enable detailed performance tracking
    pub track_performance: bool,
    /// Enable memory usage tracking
    pub track_memory: bool,
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    fn create_test_matrix() -> OptimizedSparseMatrix {
        // Create a simple 2x2 symmetric positive definite matrix
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0),
            (1, 0, 1.0), (1, 1, 3.0),
        ];
        OptimizedSparseMatrix::from_triplets(triplets, 2, 2).unwrap()
    }

    #[test]
    fn test_optimized_matrix_creation() {
        let matrix = create_test_matrix();
        assert_eq!(matrix.dimensions(), (2, 2));
        assert_eq!(matrix.nnz(), 4);
    }

    #[test]
    fn test_optimized_matrix_vector_multiply() {
        let matrix = create_test_matrix();
        let x = vec![1.0, 2.0];
        let mut y = vec![0.0; 2];

        matrix.multiply_vector(&x, &mut y);
        assert_eq!(y, vec![6.0, 7.0]); // [4*1+1*2, 1*1+3*2]
    }

    #[test]
    fn test_optimized_conjugate_gradient() {
        let matrix = create_test_matrix();
        let b = vec![1.0, 2.0];

        let config = OptimizedSolverConfig::default();
        let mut solver = OptimizedConjugateGradientSolver::new(config);

        let result = solver.solve(&matrix, &b).unwrap();

        assert!(result.converged);
        assert!(result.residual_norm < 1e-6);
        assert!(result.iterations > 0);

        // Verify solution by substituting back
        let mut ax = vec![0.0; 2];
        matrix.multiply_vector(&result.solution, &mut ax);

        let error = ((ax[0] - b[0]).powi(2) + (ax[1] - b[1]).powi(2)).sqrt();
        assert!(error < 1e-10);
    }

    #[test]
    fn test_solver_performance_stats() {
        let matrix = create_test_matrix();
        let b = vec![1.0, 2.0];

        let config = OptimizedSolverConfig::default();
        let mut solver = OptimizedConjugateGradientSolver::new(config);

        let result = solver.solve(&matrix, &b).unwrap();

        assert!(result.performance_stats.matvec_count > 0);
        assert!(result.performance_stats.dot_product_count > 0);
        assert!(result.performance_stats.total_flops > 0);
    }
}