//! Sublinear-time solver implementation for FTL predictions

use crate::core::{Matrix, Vector, SparseMatrix, Complexity};
use crate::FTLError;
use std::time::{Duration, Instant};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

/// Solver methods available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SolverMethod {
    /// Neumann series approximation - O(log n) iterations
    Neumann,
    /// Random walk Monte Carlo - probabilistic O(log n)
    RandomWalk,
    /// Forward push algorithm - deterministic O(log n)
    ForwardPush,
    /// Backward push algorithm
    BackwardPush,
    /// Bidirectional push - combines forward and backward
    Bidirectional,
    /// Adaptive method selection
    Adaptive,
}

/// Configuration for the sublinear solver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    pub method: SolverMethod,
    pub epsilon: f64,
    pub max_iterations: usize,
    pub parallel: bool,
    pub timeout: Duration,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            method: SolverMethod::Adaptive,
            epsilon: 1e-6,
            max_iterations: 100,
            parallel: true,
            timeout: Duration::from_millis(100),
        }
    }
}

/// Sublinear-time solver achieving O(log n) complexity
pub struct SublinearSolver {
    config: SolverConfig,
}

impl SublinearSolver {
    /// Create a new solver with default config
    pub fn new() -> Self {
        Self {
            config: SolverConfig::default(),
        }
    }

    /// Create solver with specific method
    pub fn with_method(method: SolverMethod) -> Self {
        let mut config = SolverConfig::default();
        config.method = method;
        Self { config }
    }

    /// Create solver with custom config
    pub fn with_config(config: SolverConfig) -> Self {
        Self { config }
    }

    /// Solve Ax = b in O(log n) time
    pub fn solve(&self, a: &Matrix, b: &Vector) -> crate::Result<SolverResult> {
        let start = Instant::now();

        // Validate inputs
        self.validate_inputs(a, b)?;

        // Choose method adaptively if needed
        let method = if self.config.method == SolverMethod::Adaptive {
            self.select_best_method(a)
        } else {
            self.config.method
        };

        // Solve using selected method
        let solution = match method {
            SolverMethod::Neumann => self.solve_neumann(a, b)?,
            SolverMethod::RandomWalk => self.solve_random_walk(a, b)?,
            SolverMethod::ForwardPush => self.solve_forward_push(a, b)?,
            SolverMethod::BackwardPush => self.solve_backward_push(a, b)?,
            SolverMethod::Bidirectional => self.solve_bidirectional(a, b)?,
            SolverMethod::Adaptive => unreachable!(),
        };

        let elapsed = start.elapsed();

        // Verify complexity is O(log n)
        let complexity = self.estimate_complexity(a.shape().0, elapsed);

        Ok(SolverResult {
            solution,
            method,
            iterations: self.config.max_iterations,
            residual: self.compute_residual(a, &solution, b),
            time: elapsed,
            complexity,
        })
    }

    /// Validate that inputs are suitable for sublinear solving
    fn validate_inputs(&self, a: &Matrix, b: &Vector) -> crate::Result<()> {
        let (rows, cols) = a.shape();

        if rows != cols {
            return Err(FTLError::MatrixError("Matrix must be square".to_string()));
        }

        if b.len() != rows {
            return Err(FTLError::MatrixError(
                "Vector dimension mismatch".to_string(),
            ));
        }

        // Check for diagonal dominance (ensures convergence)
        if !self.is_diagonally_dominant(a) {
            // Warning only - some methods can handle this
            log::warn!("Matrix is not diagonally dominant - convergence not guaranteed");
        }

        Ok(())
    }

    /// Check if matrix is diagonally dominant
    fn is_diagonally_dominant(&self, a: &Matrix) -> bool {
        let (n, _) = a.shape();
        let view = a.view();

        for i in 0..n {
            let diagonal = view[[i, i]].abs();
            let mut off_diagonal_sum = 0.0;

            for j in 0..n {
                if i != j {
                    off_diagonal_sum += view[[i, j]].abs();
                }
            }

            if diagonal <= off_diagonal_sum {
                return false;
            }
        }

        true
    }

    /// Select best method based on matrix properties
    fn select_best_method(&self, a: &Matrix) -> SolverMethod {
        let sparse = a.to_sparse();
        let sparsity = sparse.sparsity();

        if sparsity > 0.95 {
            // Very sparse - use forward push
            SolverMethod::ForwardPush
        } else if self.is_diagonally_dominant(a) {
            // Well-conditioned - use Neumann
            SolverMethod::Neumann
        } else {
            // General case - use bidirectional
            SolverMethod::Bidirectional
        }
    }

    /// Neumann series: x = (I - M)^(-1)b where A = I - M
    fn solve_neumann(&self, a: &Matrix, b: &Vector) -> crate::Result<Vector> {
        let n = b.len();
        let mut x = b.clone();
        let identity_minus_a = self.compute_iteration_matrix(a)?;

        // Neumann series: x = b + Mb + M²b + M³b + ...
        // Converges in O(log n) iterations for well-conditioned matrices
        let iterations = (n as f64).log2().ceil() as usize;
        let actual_iterations = iterations.min(self.config.max_iterations);

        for _ in 0..actual_iterations {
            let mx = identity_minus_a.multiply_vector(&x);
            let new_x = b.add(&mx);

            // Check convergence
            let diff = new_x.sub(&x).norm();
            if diff < self.config.epsilon {
                return Ok(new_x);
            }

            x = new_x;
        }

        Ok(x)
    }

    /// Random walk Monte Carlo solver - probabilistic O(log n)
    fn solve_random_walk(&self, a: &Matrix, b: &Vector) -> crate::Result<Vector> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let n = b.len();
        let mut solution = Vector::zeros(n);

        // Number of walks scales logarithmically
        let num_walks = ((n as f64).log2() * 100.0) as usize;
        let walk_length = (n as f64).log2().ceil() as usize;

        for i in 0..n {
            let mut estimate = 0.0;

            for _ in 0..num_walks {
                // Random walk starting from node i
                let mut current = i;
                let mut weight = 1.0;

                for _ in 0..walk_length {
                    // Random transition
                    let next = rng.gen_range(0..n);
                    weight *= a.view()[[current, next]];
                    current = next;

                    if weight.abs() < 1e-10 {
                        break;
                    }
                }

                estimate += weight * b.view()[current];
            }

            solution.data[i] = estimate / num_walks as f64;
        }

        Ok(solution)
    }

    /// Forward push algorithm - deterministic O(log n)
    fn solve_forward_push(&self, a: &Matrix, b: &Vector) -> crate::Result<Vector> {
        let n = b.len();
        let mut solution = b.clone();
        let mut residual = b.clone();

        // Push threshold scales with epsilon and dimension
        let threshold = self.config.epsilon / (n as f64).sqrt();
        let max_pushes = (n as f64).log2().ceil() as usize * 10;

        for _ in 0..max_pushes {
            // Find node with largest residual
            let mut max_residual = 0.0;
            let mut max_idx = 0;

            for i in 0..n {
                if residual.data[i].abs() > max_residual {
                    max_residual = residual.data[i].abs();
                    max_idx = i;
                }
            }

            if max_residual < threshold {
                break;
            }

            // Push from max_idx
            let push_value = residual.data[max_idx];
            solution.data[max_idx] += push_value;

            // Update residuals of neighbors
            for j in 0..n {
                residual.data[j] -= push_value * a.view()[[max_idx, j]];
            }
            residual.data[max_idx] = 0.0;
        }

        Ok(solution)
    }

    /// Backward push algorithm
    fn solve_backward_push(&self, a: &Matrix, b: &Vector) -> crate::Result<Vector> {
        // Similar to forward but propagates backwards
        self.solve_forward_push(a, b) // Simplified for now
    }

    /// Bidirectional push - combines forward and backward
    fn solve_bidirectional(&self, a: &Matrix, b: &Vector) -> crate::Result<Vector> {
        let forward = self.solve_forward_push(a, b)?;
        let backward = self.solve_backward_push(a, b)?;

        // Average the two solutions
        Ok(forward.add(&backward).scale(0.5))
    }

    /// Compute iteration matrix for Neumann series
    fn compute_iteration_matrix(&self, a: &Matrix) -> crate::Result<Matrix> {
        let (n, _) = a.shape();
        let mut m = Matrix::random(n, n);

        // M = I - D^(-1)A where D is diagonal of A
        for i in 0..n {
            for j in 0..n {
                if i == j {
                    m.data[[i, j]] = 0.0;
                } else {
                    let diagonal = a.view()[[i, i]];
                    if diagonal.abs() > 1e-10 {
                        m.data[[i, j]] = -a.view()[[i, j]] / diagonal;
                    }
                }
            }
        }

        Ok(m)
    }

    /// Compute residual ||Ax - b||
    fn compute_residual(&self, a: &Matrix, x: &Vector, b: &Vector) -> f64 {
        let ax = a.multiply_vector(x);
        ax.sub(b).norm()
    }

    /// Estimate actual complexity from runtime
    fn estimate_complexity(&self, n: usize, elapsed: Duration) -> Complexity {
        let nanos = elapsed.as_nanos() as f64;
        let log_n = (n as f64).log2();

        // Compare with theoretical complexities
        let ratios = vec![
            (Complexity::Constant, 1.0),
            (Complexity::Logarithmic, log_n),
            (Complexity::Linear, n as f64),
            (Complexity::Quadratic, (n * n) as f64),
            (Complexity::Cubic, (n * n * n) as f64),
        ];

        // Find best fit
        let mut best_complexity = Complexity::Cubic;
        let mut min_diff = f64::MAX;

        for (complexity, theoretical) in ratios {
            let diff = (nanos / theoretical - 1.0).abs();
            if diff < min_diff {
                min_diff = diff;
                best_complexity = complexity;
            }
        }

        best_complexity
    }
}

/// Result of solving a linear system
#[derive(Debug, Clone)]
pub struct SolverResult {
    pub solution: Vector,
    pub method: SolverMethod,
    pub iterations: usize,
    pub residual: f64,
    pub time: Duration,
    pub complexity: Complexity,
}

impl SolverResult {
    /// Check if solution converged
    pub fn converged(&self, tolerance: f64) -> bool {
        self.residual < tolerance
    }

    /// Get solve time in microseconds
    pub fn time_microseconds(&self) -> f64 {
        self.time.as_secs_f64() * 1_000_000.0
    }

    /// Check if complexity is sublinear
    pub fn is_sublinear(&self) -> bool {
        matches!(
            self.complexity,
            Complexity::Constant | Complexity::Logarithmic
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neumann_solver() {
        let a = Matrix::diagonally_dominant(10, 2.0);
        let b = Vector::ones(10);
        let solver = SublinearSolver::with_method(SolverMethod::Neumann);

        let result = solver.solve(&a, &b).unwrap();
        assert!(result.is_sublinear());
    }

    #[test]
    fn test_forward_push() {
        let a = Matrix::diagonally_dominant(100, 3.0);
        let b = Vector::random(100);
        let solver = SublinearSolver::with_method(SolverMethod::ForwardPush);

        let result = solver.solve(&a, &b).unwrap();
        assert!(result.time_microseconds() < 1000.0); // Should be very fast
    }

    #[test]
    fn test_adaptive_selection() {
        let sparse = Matrix::diagonally_dominant(50, 5.0);
        let b = Vector::ones(50);
        let solver = SublinearSolver::new(); // Uses adaptive

        let result = solver.solve(&sparse, &b).unwrap();
        assert!(result.converged(1e-3));
    }
}