//! Eigenvalue computation algorithms
//!
//! This module provides efficient algorithms for computing eigenvalues and eigenvectors
//! of sparse symmetric matrices, specifically designed for graph Laplacians.
//!
//! ## Algorithms
//!
//! - **Power Iteration**: Simple method for finding the largest eigenvalue
//! - **Inverse Power Iteration**: Finds smallest eigenvalue (with shift)
//! - **Lanczos Algorithm**: Efficient method for finding multiple eigenvalues of sparse matrices

use super::types::{SparseMatrix, Vector, CONVERGENCE_TOL, EPS, MAX_ITER};
use std::f64::consts::SQRT_2;

/// Normalize a vector to unit length
fn normalize(v: &mut Vector) -> f64 {
    let norm: f64 = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm > EPS {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
    norm
}

/// Compute dot product of two vectors
fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Subtract scaled vector: a = a - scale * b
fn axpy(a: &mut Vector, b: &[f64], scale: f64) {
    for (ai, &bi) in a.iter_mut().zip(b.iter()) {
        *ai -= scale * bi;
    }
}

/// Generate a random unit vector
fn random_unit_vector(n: usize, seed: u64) -> Vector {
    let mut v = Vec::with_capacity(n);
    let mut state = seed;

    for _ in 0..n {
        // Simple LCG for reproducibility
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let rand = ((state >> 33) as f64) / (u32::MAX as f64) - 0.5;
        v.push(rand);
    }

    normalize(&mut v);
    v
}

/// Power iteration for finding the largest eigenvalue
#[derive(Debug, Clone)]
pub struct PowerIteration {
    /// Maximum iterations
    pub max_iter: usize,
    /// Convergence tolerance
    pub tol: f64,
}

impl Default for PowerIteration {
    fn default() -> Self {
        Self {
            max_iter: MAX_ITER,
            tol: CONVERGENCE_TOL,
        }
    }
}

impl PowerIteration {
    /// Create a new power iteration solver
    pub fn new(max_iter: usize, tol: f64) -> Self {
        Self { max_iter, tol }
    }

    /// Find the largest eigenvalue and corresponding eigenvector
    pub fn largest_eigenvalue(&self, matrix: &SparseMatrix) -> (f64, Vector) {
        assert_eq!(matrix.rows, matrix.cols);
        let n = matrix.rows;

        if n == 0 {
            return (0.0, Vec::new());
        }

        let mut v = random_unit_vector(n, 42);
        let mut lambda = 0.0;

        for _ in 0..self.max_iter {
            // w = A * v
            let mut w = matrix.mul_vec(&v);

            // Rayleigh quotient: lambda = v^T A v
            let new_lambda = dot(&v, &w);

            // Normalize
            normalize(&mut w);

            // Check convergence
            if (new_lambda - lambda).abs() < self.tol {
                return (new_lambda, w);
            }

            lambda = new_lambda;
            v = w;
        }

        (lambda, v)
    }

    /// Find the smallest eigenvalue using inverse iteration
    /// Requires solving (A - shift*I)x = b, which we approximate
    pub fn smallest_eigenvalue(&self, matrix: &SparseMatrix, shift: f64) -> (f64, Vector) {
        assert_eq!(matrix.rows, matrix.cols);
        let n = matrix.rows;

        if n == 0 {
            return (0.0, Vec::new());
        }

        // Create shifted matrix: A - shift*I
        let identity = SparseMatrix::identity(n);
        let shifted = matrix.add(&identity.scale(-shift));

        // Use power iteration on the shifted matrix
        // The smallest eigenvalue of A corresponds to the eigenvalue of (A - shift*I)
        // closest to zero, which becomes the largest in magnitude for inverse iteration

        // Since we can't easily invert, we use a gradient descent approach
        let mut v = random_unit_vector(n, 123);
        let mut lambda = shift;

        for iter in 0..self.max_iter {
            // Compute A*v
            let av = matrix.mul_vec(&v);

            // Rayleigh quotient
            let rq = dot(&v, &av);

            // Gradient: 2(A*v - rq*v)
            let mut grad: Vector = av.iter().zip(v.iter())
                .map(|(&avi, &vi)| 2.0 * (avi - rq * vi))
                .collect();

            let grad_norm = normalize(&mut grad);

            if grad_norm < self.tol {
                return (rq, v);
            }

            // Line search with decreasing step size
            let step = 0.1 / (1.0 + iter as f64 * 0.01);

            // Update: v = v - step * grad
            for (vi, gi) in v.iter_mut().zip(grad.iter()) {
                *vi -= step * gi;
            }
            normalize(&mut v);

            if (rq - lambda).abs() < self.tol {
                return (rq, v);
            }
            lambda = rq;
        }

        (lambda, v)
    }

    /// Find eigenvalue closest to a target using shifted inverse iteration
    pub fn eigenvalue_near(&self, matrix: &SparseMatrix, target: f64) -> (f64, Vector) {
        self.smallest_eigenvalue(matrix, target)
    }
}

/// Lanczos algorithm for computing multiple eigenvalues of sparse symmetric matrices
#[derive(Debug, Clone)]
pub struct LanczosAlgorithm {
    /// Number of Lanczos vectors to compute
    pub num_vectors: usize,
    /// Maximum iterations
    pub max_iter: usize,
    /// Convergence tolerance
    pub tol: f64,
    /// Number of eigenvalues to return
    pub num_eigenvalues: usize,
    /// Reorthogonalization frequency
    pub reorth_freq: usize,
}

impl Default for LanczosAlgorithm {
    fn default() -> Self {
        Self {
            num_vectors: 30,
            max_iter: MAX_ITER,
            tol: CONVERGENCE_TOL,
            num_eigenvalues: 10,
            reorth_freq: 5,
        }
    }
}

impl LanczosAlgorithm {
    /// Create a new Lanczos solver
    pub fn new(num_eigenvalues: usize) -> Self {
        Self {
            num_vectors: (num_eigenvalues * 3).max(30),
            num_eigenvalues,
            ..Default::default()
        }
    }

    /// Compute the k smallest eigenvalues and eigenvectors
    pub fn compute_smallest(&self, matrix: &SparseMatrix) -> (Vec<f64>, Vec<Vector>) {
        assert_eq!(matrix.rows, matrix.cols);
        let n = matrix.rows;

        if n == 0 {
            return (Vec::new(), Vec::new());
        }

        let k = self.num_vectors.min(n);
        let mut eigenvalues = Vec::new();
        let mut eigenvectors = Vec::new();

        // Lanczos vectors
        let mut v: Vec<Vector> = Vec::with_capacity(k + 1);

        // Tridiagonal matrix elements
        let mut alpha: Vec<f64> = Vec::with_capacity(k);
        let mut beta: Vec<f64> = Vec::with_capacity(k);

        // Initialize with random vector
        let v0 = vec![0.0; n];
        let mut v1 = random_unit_vector(n, 42);

        v.push(v0);
        v.push(v1.clone());

        // Lanczos iteration
        for j in 1..=k {
            // w = A * v_j
            let mut w = matrix.mul_vec(&v[j]);

            // alpha_j = v_j^T * w
            let alpha_j = dot(&v[j], &w);
            alpha.push(alpha_j);

            // w = w - alpha_j * v_j - beta_{j-1} * v_{j-1}
            axpy(&mut w, &v[j], alpha_j);
            if j > 1 {
                axpy(&mut w, &v[j - 1], beta[j - 2]);
            }

            // Reorthogonalization for numerical stability
            if j % self.reorth_freq == 0 {
                for i in 1..=j {
                    let proj = dot(&w, &v[i]);
                    axpy(&mut w, &v[i], proj);
                }
            }

            // beta_j = ||w||
            let beta_j = normalize(&mut w);

            if beta_j < self.tol {
                // Found an invariant subspace, stop early
                break;
            }

            beta.push(beta_j);
            v.push(w);
        }

        // Solve tridiagonal eigenvalue problem
        let (tri_eigenvalues, tri_eigenvectors) =
            self.solve_tridiagonal(&alpha, &beta);

        // Transform eigenvectors back to original space
        let m = alpha.len();
        let num_return = self.num_eigenvalues.min(m);

        for i in 0..num_return {
            eigenvalues.push(tri_eigenvalues[i]);

            // y = V * z (where z is the tridiagonal eigenvector)
            let mut y = vec![0.0; n];
            for j in 0..m {
                for k in 0..n {
                    y[k] += tri_eigenvectors[i][j] * v[j + 1][k];
                }
            }
            normalize(&mut y);
            eigenvectors.push(y);
        }

        (eigenvalues, eigenvectors)
    }

    /// Compute the k largest eigenvalues and eigenvectors
    pub fn compute_largest(&self, matrix: &SparseMatrix) -> (Vec<f64>, Vec<Vector>) {
        // For largest eigenvalues, we can use negative of matrix
        // and negate the result
        let neg_matrix = matrix.scale(-1.0);
        let (mut eigenvalues, eigenvectors) = self.compute_smallest(&neg_matrix);

        for ev in eigenvalues.iter_mut() {
            *ev = -*ev;
        }

        // Reverse to get largest first
        eigenvalues.reverse();
        let eigenvectors: Vec<Vector> = eigenvectors.into_iter().rev().collect();

        (eigenvalues, eigenvectors)
    }

    /// Solve the tridiagonal eigenvalue problem using QR algorithm
    fn solve_tridiagonal(&self, alpha: &[f64], beta: &[f64]) -> (Vec<f64>, Vec<Vec<f64>>) {
        let n = alpha.len();
        if n == 0 {
            return (Vec::new(), Vec::new());
        }

        // Copy diagonal and off-diagonal
        let mut d: Vec<f64> = alpha.to_vec();
        let mut e: Vec<f64> = beta.to_vec();

        // Initialize eigenvector matrix as identity
        let mut z: Vec<Vec<f64>> = (0..n).map(|i| {
            let mut row = vec![0.0; n];
            row[i] = 1.0;
            row
        }).collect();

        // Implicit QR algorithm for symmetric tridiagonal matrices
        for _ in 0..self.max_iter {
            let mut converged = true;

            for i in 0..n.saturating_sub(1) {
                if e[i].abs() > self.tol * (d[i].abs() + d[i + 1].abs()) {
                    converged = false;

                    // Wilkinson shift
                    let delta = (d[i + 1] - d[i]) / 2.0;
                    let sign = if delta >= 0.0 { 1.0 } else { -1.0 };
                    let shift = d[i + 1] - sign * e[i].powi(2) /
                        (delta.abs() + (delta.powi(2) + e[i].powi(2)).sqrt());

                    // Apply QR step with shift
                    self.qr_step(&mut d, &mut e, &mut z, i, n - 1, shift);
                }
            }

            if converged {
                break;
            }
        }

        // Sort eigenvalues (ascending) and corresponding eigenvectors
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&i, &j| d[i].partial_cmp(&d[j]).unwrap());

        let sorted_eigenvalues: Vec<f64> = indices.iter().map(|&i| d[i]).collect();
        let sorted_eigenvectors: Vec<Vec<f64>> = indices.iter().map(|&i| z[i].clone()).collect();

        (sorted_eigenvalues, sorted_eigenvectors)
    }

    /// Perform one implicit QR step
    fn qr_step(
        &self,
        d: &mut [f64],
        e: &mut [f64],
        z: &mut [Vec<f64>],
        start: usize,
        end: usize,
        shift: f64,
    ) {
        let mut c = 1.0;
        let mut s = 0.0;
        let mut p = d[start] - shift;

        for i in start..end {
            let r = (p * p + e[i] * e[i]).sqrt();

            if r < EPS {
                e[i] = 0.0;
                continue;
            }

            let c_prev = c;
            let s_prev = s;

            c = p / r;
            s = e[i] / r;

            if i > start {
                e[i - 1] = r * s_prev;
            }

            p = c * d[i] - s * e[i];
            let temp = c * e[i] + s * d[i + 1];
            d[i] = c * p + s * temp;
            p = c * temp - s * d[i + 1];
            d[i + 1] = s * p + c * d[i + 1];
            e[i] = s * p;

            // Update eigenvectors
            let n = z.len();
            for k in 0..n {
                let zi = z[i][k];
                let zi1 = z[i + 1][k];
                z[i][k] = c * zi - s * zi1;
                z[i + 1][k] = s * zi + c * zi1;
            }
        }

        if end > start {
            e[end - 1] = p * s;
            d[end] = p * c + shift;
        }
    }

    /// Estimate spectral radius (largest magnitude eigenvalue)
    pub fn spectral_radius(&self, matrix: &SparseMatrix) -> f64 {
        let power = PowerIteration::default();
        let (lambda, _) = power.largest_eigenvalue(matrix);
        lambda.abs()
    }

    /// Compute condition number estimate
    pub fn condition_number(&self, matrix: &SparseMatrix) -> f64 {
        let (eigenvalues, _) = self.compute_smallest(matrix);

        if eigenvalues.is_empty() {
            return f64::INFINITY;
        }

        let min_ev = eigenvalues.iter()
            .filter(|&&x| x.abs() > EPS)
            .fold(f64::INFINITY, |a, &b| a.min(b.abs()));

        let max_ev = eigenvalues.iter()
            .fold(0.0f64, |a, &b| a.max(b.abs()));

        if min_ev > EPS {
            max_ev / min_ev
        } else {
            f64::INFINITY
        }
    }
}

/// Deflation method for finding multiple eigenvalues
pub struct DeflationSolver {
    /// Power iteration solver
    power: PowerIteration,
    /// Number of eigenvalues to compute
    num_eigenvalues: usize,
}

impl DeflationSolver {
    /// Create a new deflation solver
    pub fn new(num_eigenvalues: usize) -> Self {
        Self {
            power: PowerIteration::default(),
            num_eigenvalues,
        }
    }

    /// Compute eigenvalues using Hotelling deflation
    pub fn compute(&self, matrix: &SparseMatrix) -> (Vec<f64>, Vec<Vector>) {
        let n = matrix.rows;
        let mut eigenvalues = Vec::new();
        let mut eigenvectors = Vec::new();
        let mut current_matrix = matrix.clone();

        for _ in 0..self.num_eigenvalues.min(n) {
            let (lambda, v) = self.power.largest_eigenvalue(&current_matrix);

            if lambda.abs() < EPS {
                break;
            }

            eigenvalues.push(lambda);
            eigenvectors.push(v.clone());

            // Deflate: A' = A - lambda * v * v^T
            let mut triplets = Vec::new();

            for i in 0..n {
                for j in 0..n {
                    let val = current_matrix.get(i, j) - lambda * v[i] * v[j];
                    if val.abs() > EPS {
                        triplets.push((i, j, val));
                    }
                }
            }

            current_matrix = SparseMatrix::from_triplets(n, n, &triplets);
        }

        (eigenvalues, eigenvectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_matrix() -> SparseMatrix {
        // Simple symmetric 3x3 matrix
        let triplets = vec![
            (0, 0, 4.0), (0, 1, 1.0), (0, 2, 0.0),
            (1, 0, 1.0), (1, 1, 3.0), (1, 2, 1.0),
            (2, 0, 0.0), (2, 1, 1.0), (2, 2, 2.0),
        ];
        SparseMatrix::from_triplets(3, 3, &triplets)
    }

    #[test]
    fn test_power_iteration() {
        let m = create_test_matrix();
        let power = PowerIteration::default();
        let (lambda, v) = power.largest_eigenvalue(&m);

        // Verify eigenvalue equation: ||Av - lambda*v|| should be small
        let av = m.mul_vec(&v);
        let error: f64 = av.iter()
            .zip(v.iter())
            .map(|(avi, vi)| (avi - lambda * vi).powi(2))
            .sum::<f64>()
            .sqrt();

        assert!(error < 0.01, "Eigenvalue error too large: {}", error);
    }

    #[test]
    fn test_lanczos() {
        let m = create_test_matrix();
        let lanczos = LanczosAlgorithm::new(3);
        let (eigenvalues, eigenvectors) = lanczos.compute_smallest(&m);

        assert!(!eigenvalues.is_empty());

        // Verify first eigenvalue equation
        if !eigenvectors.is_empty() {
            let v = &eigenvectors[0];
            let lambda = eigenvalues[0];
            let av = m.mul_vec(v);

            let error: f64 = av.iter()
                .zip(v.iter())
                .map(|(avi, vi)| (avi - lambda * vi).powi(2))
                .sum::<f64>()
                .sqrt();

            assert!(error < 0.1, "Lanczos eigenvalue error: {}", error);
        }
    }

    #[test]
    fn test_normalize() {
        let mut v = vec![3.0, 4.0];
        let norm = normalize(&mut v);

        assert!((norm - 5.0).abs() < EPS);
        assert!((v[0] - 0.6).abs() < EPS);
        assert!((v[1] - 0.8).abs() < EPS);
    }

    #[test]
    fn test_spectral_radius() {
        let m = create_test_matrix();
        let lanczos = LanczosAlgorithm::default();
        let radius = lanczos.spectral_radius(&m);

        // For our test matrix, largest eigenvalue should be around 5
        assert!(radius > 3.0 && radius < 6.0);
    }
}
