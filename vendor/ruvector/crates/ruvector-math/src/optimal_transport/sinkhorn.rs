//! Log-Stabilized Sinkhorn Algorithm
//!
//! The Sinkhorn algorithm computes the entropic-regularized optimal transport:
//!
//! min_{γ ∈ Π(a,b)} ⟨γ, C⟩ - ε H(γ)
//!
//! where H(γ) = -Σ γ_ij log(γ_ij) is the entropy and ε is the regularization.
//!
//! ## Log-Stabilization
//!
//! We work in log-domain to prevent numerical overflow/underflow:
//! - Store log(u) and log(v) instead of u, v
//! - Use log-sum-exp for stable normalization
//!
//! ## Complexity
//!
//! - O(n² × iterations) for dense cost matrix
//! - Typically converges in 50-200 iterations
//! - ~1000x faster than linear programming for exact OT

use crate::error::{MathError, Result};
use crate::utils::{log_sum_exp, EPS, LOG_MIN};

/// Result of Sinkhorn algorithm
#[derive(Debug, Clone)]
pub struct TransportPlan {
    /// Transport plan matrix γ[i,j] (n × m)
    pub plan: Vec<Vec<f64>>,
    /// Total transport cost
    pub cost: f64,
    /// Number of iterations to convergence
    pub iterations: usize,
    /// Final marginal error (||Pγ - a||₁ + ||γᵀ1 - b||₁)
    pub marginal_error: f64,
    /// Whether the algorithm converged
    pub converged: bool,
}

/// Log-stabilized Sinkhorn solver for entropic optimal transport
#[derive(Debug, Clone)]
pub struct SinkhornSolver {
    /// Regularization parameter ε
    regularization: f64,
    /// Maximum iterations
    max_iterations: usize,
    /// Convergence threshold
    threshold: f64,
}

impl SinkhornSolver {
    /// Create a new Sinkhorn solver
    ///
    /// # Arguments
    /// * `regularization` - Entropy regularization ε (0.01-0.1 typical)
    /// * `max_iterations` - Maximum Sinkhorn iterations (100-1000 typical)
    pub fn new(regularization: f64, max_iterations: usize) -> Self {
        Self {
            regularization: regularization.max(1e-6),
            max_iterations: max_iterations.max(1),
            threshold: 1e-6,
        }
    }

    /// Set convergence threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.max(1e-12);
        self
    }

    /// Compute the cost matrix for squared Euclidean distance
    /// Uses SIMD-friendly 4-way unrolled accumulator for better performance
    #[inline]
    pub fn compute_cost_matrix(source: &[Vec<f64>], target: &[Vec<f64>]) -> Vec<Vec<f64>> {
        source
            .iter()
            .map(|s| {
                target
                    .iter()
                    .map(|t| Self::squared_euclidean(s, t))
                    .collect()
            })
            .collect()
    }

    /// SIMD-friendly squared Euclidean distance
    #[inline(always)]
    fn squared_euclidean(a: &[f64], b: &[f64]) -> f64 {
        let len = a.len();
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f64;
        let mut sum1 = 0.0f64;
        let mut sum2 = 0.0f64;
        let mut sum3 = 0.0f64;

        for i in 0..chunks {
            let base = i * 4;
            let d0 = a[base] - b[base];
            let d1 = a[base + 1] - b[base + 1];
            let d2 = a[base + 2] - b[base + 2];
            let d3 = a[base + 3] - b[base + 3];
            sum0 += d0 * d0;
            sum1 += d1 * d1;
            sum2 += d2 * d2;
            sum3 += d3 * d3;
        }

        let base = chunks * 4;
        for i in 0..remainder {
            let d = a[base + i] - b[base + i];
            sum0 += d * d;
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Solve optimal transport using log-stabilized Sinkhorn
    ///
    /// # Arguments
    /// * `cost_matrix` - C[i,j] = cost to move from source[i] to target[j]
    /// * `source_weights` - Marginal distribution a (sum to 1)
    /// * `target_weights` - Marginal distribution b (sum to 1)
    pub fn solve(
        &self,
        cost_matrix: &[Vec<f64>],
        source_weights: &[f64],
        target_weights: &[f64],
    ) -> Result<TransportPlan> {
        let n = source_weights.len();
        let m = target_weights.len();

        if n == 0 || m == 0 {
            return Err(MathError::empty_input("weights"));
        }

        if cost_matrix.len() != n || cost_matrix.iter().any(|row| row.len() != m) {
            return Err(MathError::dimension_mismatch(n, cost_matrix.len()));
        }

        // Normalize weights
        let sum_a: f64 = source_weights.iter().sum();
        let sum_b: f64 = target_weights.iter().sum();
        let a: Vec<f64> = source_weights.iter().map(|&w| w / sum_a).collect();
        let b: Vec<f64> = target_weights.iter().map(|&w| w / sum_b).collect();

        // Initialize log-domain Gibbs kernel: K = exp(-C/ε)
        // Store log(K) = -C/ε
        let log_k: Vec<Vec<f64>> = cost_matrix
            .iter()
            .map(|row| row.iter().map(|&c| -c / self.regularization).collect())
            .collect();

        // Initialize log scaling vectors
        let mut log_u = vec![0.0; n];
        let mut log_v = vec![0.0; m];

        let log_a: Vec<f64> = a.iter().map(|&ai| ai.ln().max(LOG_MIN)).collect();
        let log_b: Vec<f64> = b.iter().map(|&bi| bi.ln().max(LOG_MIN)).collect();

        let mut converged = false;
        let mut iterations = 0;
        let mut marginal_error = f64::INFINITY;

        // Pre-allocate buffers for log-sum-exp computation (reduces allocations per iteration)
        let mut log_terms_row = vec![0.0; m];
        let mut log_terms_col = vec![0.0; n];

        // Sinkhorn iterations in log domain
        for iter in 0..self.max_iterations {
            iterations = iter + 1;

            // Update log_u: log_u = log_a - log_sum_exp_j(log_v[j] + log_K[i,j])
            let mut max_u_change: f64 = 0.0;
            for i in 0..n {
                let old_log_u = log_u[i];
                // Compute into pre-allocated buffer
                for j in 0..m {
                    log_terms_row[j] = log_v[j] + log_k[i][j];
                }
                let lse = log_sum_exp(&log_terms_row);
                log_u[i] = log_a[i] - lse;
                max_u_change = max_u_change.max((log_u[i] - old_log_u).abs());
            }

            // Update log_v: log_v = log_b - log_sum_exp_i(log_u[i] + log_K[i,j])
            let mut max_v_change: f64 = 0.0;
            for j in 0..m {
                let old_log_v = log_v[j];
                // Compute into pre-allocated buffer
                for i in 0..n {
                    log_terms_col[i] = log_u[i] + log_k[i][j];
                }
                let lse = log_sum_exp(&log_terms_col);
                log_v[j] = log_b[j] - lse;
                max_v_change = max_v_change.max((log_v[j] - old_log_v).abs());
            }

            // Check convergence
            let max_change = max_u_change.max(max_v_change);

            // Compute marginal error every 10 iterations
            if iter % 10 == 0 || max_change < self.threshold {
                marginal_error = self.compute_marginal_error(&log_u, &log_v, &log_k, &a, &b);

                if max_change < self.threshold && marginal_error < self.threshold * 10.0 {
                    converged = true;
                    break;
                }
            }
        }

        // Compute transport plan: γ[i,j] = exp(log_u[i] + log_K[i,j] + log_v[j])
        let plan: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..m)
                    .map(|j| {
                        let log_gamma = log_u[i] + log_k[i][j] + log_v[j];
                        log_gamma.exp().max(0.0)
                    })
                    .collect()
            })
            .collect();

        // Compute transport cost: ⟨γ, C⟩
        let cost = plan
            .iter()
            .zip(cost_matrix.iter())
            .map(|(gamma_row, cost_row)| {
                gamma_row
                    .iter()
                    .zip(cost_row.iter())
                    .map(|(&g, &c)| g * c)
                    .sum::<f64>()
            })
            .sum();

        Ok(TransportPlan {
            plan,
            cost,
            iterations,
            marginal_error,
            converged,
        })
    }

    /// Compute marginal constraint error
    fn compute_marginal_error(
        &self,
        log_u: &[f64],
        log_v: &[f64],
        log_k: &[Vec<f64>],
        a: &[f64],
        b: &[f64],
    ) -> f64 {
        let n = log_u.len();
        let m = log_v.len();

        // Compute row sums (γ1 should equal a)
        let mut row_error = 0.0;
        for i in 0..n {
            let log_row_sum = log_sum_exp(
                &(0..m)
                    .map(|j| log_u[i] + log_k[i][j] + log_v[j])
                    .collect::<Vec<_>>(),
            );
            row_error += (log_row_sum.exp() - a[i]).abs();
        }

        // Compute column sums (γᵀ1 should equal b)
        let mut col_error = 0.0;
        for j in 0..m {
            let log_col_sum = log_sum_exp(
                &(0..n)
                    .map(|i| log_u[i] + log_k[i][j] + log_v[j])
                    .collect::<Vec<_>>(),
            );
            col_error += (log_col_sum.exp() - b[j]).abs();
        }

        row_error + col_error
    }

    /// Compute Sinkhorn distance (optimal transport cost) between point clouds
    pub fn distance(&self, source: &[Vec<f64>], target: &[Vec<f64>]) -> Result<f64> {
        let cost_matrix = Self::compute_cost_matrix(source, target);

        // Uniform weights
        let n = source.len();
        let m = target.len();
        let source_weights = vec![1.0 / n as f64; n];
        let target_weights = vec![1.0 / m as f64; m];

        let result = self.solve(&cost_matrix, &source_weights, &target_weights)?;
        Ok(result.cost)
    }

    /// Compute Wasserstein barycenter of multiple distributions
    ///
    /// Returns the barycenter (mean distribution) in transport space
    pub fn barycenter(
        &self,
        distributions: &[&[Vec<f64>]],
        weights: Option<&[f64]>,
        support_size: usize,
        dim: usize,
    ) -> Result<Vec<Vec<f64>>> {
        if distributions.is_empty() {
            return Err(MathError::empty_input("distributions"));
        }

        let k = distributions.len();
        let barycenter_weights = match weights {
            Some(w) => {
                let sum: f64 = w.iter().sum();
                w.iter().map(|&wi| wi / sum).collect()
            }
            None => vec![1.0 / k as f64; k],
        };

        // Initialize barycenter as mean of first distribution
        let mut barycenter: Vec<Vec<f64>> = (0..support_size)
            .map(|i| {
                let t = i as f64 / (support_size - 1).max(1) as f64;
                vec![t; dim]
            })
            .collect();

        // Fixed-point iteration to find barycenter
        for _outer in 0..20 {
            // For each input distribution, compute transport to barycenter
            let mut displacements = vec![vec![0.0; dim]; support_size];

            for (dist_idx, &distribution) in distributions.iter().enumerate() {
                let cost_matrix = Self::compute_cost_matrix(distribution, &barycenter);

                let n = distribution.len();
                let source_w = vec![1.0 / n as f64; n];
                let target_w = vec![1.0 / support_size as f64; support_size];

                if let Ok(plan) = self.solve(&cost_matrix, &source_w, &target_w) {
                    // Compute displacement from plan
                    for j in 0..support_size {
                        for i in 0..n {
                            let weight = plan.plan[i][j] * support_size as f64;
                            for d in 0..dim {
                                displacements[j][d] += barycenter_weights[dist_idx]
                                    * weight
                                    * (distribution[i][d] - barycenter[j][d]);
                            }
                        }
                    }
                }
            }

            // Update barycenter
            let mut max_update: f64 = 0.0;
            for j in 0..support_size {
                for d in 0..dim {
                    let delta = displacements[j][d] * 0.5; // Step size
                    barycenter[j][d] += delta;
                    max_update = max_update.max(delta.abs());
                }
            }

            if max_update < EPS {
                break;
            }
        }

        Ok(barycenter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sinkhorn_identity() {
        let solver = SinkhornSolver::new(0.1, 100);

        let source = vec![vec![0.0, 0.0], vec![1.0, 1.0]];
        let target = vec![vec![0.0, 0.0], vec![1.0, 1.0]];

        let cost = solver.distance(&source, &target).unwrap();
        assert!(cost < 0.1, "Identity should have near-zero cost: {}", cost);
    }

    #[test]
    fn test_sinkhorn_translation() {
        let solver = SinkhornSolver::new(0.05, 200);

        let source = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];

        // Translate by (1, 0)
        let target: Vec<Vec<f64>> = source.iter().map(|p| vec![p[0] + 1.0, p[1]]).collect();

        let cost = solver.distance(&source, &target).unwrap();

        // Expected cost for unit translation: each point moves distance 1
        // With squared Euclidean: cost ≈ 1.0
        assert!(
            cost > 0.5 && cost < 2.0,
            "Translation cost should be ~1.0: {}",
            cost
        );
    }

    #[test]
    fn test_sinkhorn_convergence() {
        let solver = SinkhornSolver::new(0.1, 100).with_threshold(1e-6);

        let cost_matrix = vec![
            vec![0.0, 1.0, 2.0],
            vec![1.0, 0.0, 1.0],
            vec![2.0, 1.0, 0.0],
        ];

        let a = vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];
        let b = vec![1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0];

        let result = solver.solve(&cost_matrix, &a, &b).unwrap();

        assert!(result.converged, "Should converge");
        assert!(
            result.marginal_error < 0.01,
            "Marginal error too high: {}",
            result.marginal_error
        );
    }

    #[test]
    fn test_transport_plan_marginals() {
        let solver = SinkhornSolver::new(0.1, 100);

        let cost_matrix = vec![vec![0.0, 1.0], vec![1.0, 0.0]];

        let a = vec![0.3, 0.7];
        let b = vec![0.6, 0.4];

        let result = solver.solve(&cost_matrix, &a, &b).unwrap();

        // Check row marginals
        for (i, &ai) in a.iter().enumerate() {
            let row_sum: f64 = result.plan[i].iter().sum();
            assert!(
                (row_sum - ai).abs() < 0.05,
                "Row {} sum {} != {}",
                i,
                row_sum,
                ai
            );
        }

        // Check column marginals
        for (j, &bj) in b.iter().enumerate() {
            let col_sum: f64 = result.plan.iter().map(|row| row[j]).sum();
            assert!(
                (col_sum - bj).abs() < 0.05,
                "Col {} sum {} != {}",
                j,
                col_sum,
                bj
            );
        }
    }
}
