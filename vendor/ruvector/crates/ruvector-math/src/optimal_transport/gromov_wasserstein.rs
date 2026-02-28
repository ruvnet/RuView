//! Gromov-Wasserstein Distance
//!
//! Gromov-Wasserstein (GW) distance compares the *structure* of two metric spaces,
//! not requiring them to share a common embedding space.
//!
//! ## Definition
//!
//! GW(X, Y) = min_{γ ∈ Π(μ,ν)} Σᵢⱼₖₗ |d_X(xᵢ, xₖ) - d_Y(yⱼ, yₗ)|² γᵢⱼ γₖₗ
//!
//! This measures how well the pairwise distances in X match those in Y.
//!
//! ## Use Cases
//!
//! - Cross-lingual word embeddings (different embedding spaces)
//! - Graph matching (comparing graph structures)
//! - Shape matching (comparing point cloud structures)
//! - Multi-modal alignment (different feature spaces)
//!
//! ## Algorithm
//!
//! Uses Frank-Wolfe (conditional gradient) with entropic regularization:
//! 1. Initialize transport plan (identity or Sinkhorn)
//! 2. Compute gradient of GW objective
//! 3. Solve linearized problem via Sinkhorn
//! 4. Line search and update
//! 5. Repeat until convergence

use super::SinkhornSolver;
use crate::error::{MathError, Result};
use crate::utils::EPS;

/// Gromov-Wasserstein distance calculator
#[derive(Debug, Clone)]
pub struct GromovWasserstein {
    /// Regularization for inner Sinkhorn
    regularization: f64,
    /// Maximum outer iterations
    max_iterations: usize,
    /// Convergence threshold
    threshold: f64,
    /// Inner Sinkhorn iterations
    inner_iterations: usize,
}

impl GromovWasserstein {
    /// Create a new Gromov-Wasserstein calculator
    ///
    /// # Arguments
    /// * `regularization` - Entropy regularization (0.01-0.1 typical)
    pub fn new(regularization: f64) -> Self {
        Self {
            regularization: regularization.max(1e-6),
            max_iterations: 100,
            threshold: 1e-5,
            inner_iterations: 50,
        }
    }

    /// Set maximum iterations
    pub fn with_max_iterations(mut self, max_iter: usize) -> Self {
        self.max_iterations = max_iter.max(1);
        self
    }

    /// Set convergence threshold
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.threshold = threshold.max(1e-12);
        self
    }

    /// Compute pairwise distance matrix
    fn distance_matrix(points: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let n = points.len();
        let mut dist = vec![vec![0.0; n]; n];

        for i in 0..n {
            for j in (i + 1)..n {
                let d: f64 = points[i]
                    .iter()
                    .zip(points[j].iter())
                    .map(|(&a, &b)| (a - b).powi(2))
                    .sum::<f64>()
                    .sqrt();
                dist[i][j] = d;
                dist[j][i] = d;
            }
        }

        dist
    }

    /// Compute squared distance loss tensor contraction
    /// L(γ) = Σᵢⱼₖₗ (D_X[i,k] - D_Y[j,l])² γᵢⱼ γₖₗ
    ///      = ⟨h₁(D_X) ⊗ h₂(D_Y), γ ⊗ γ⟩ - 2⟨D_X γ D_Y^T, γ⟩
    ///
    /// where h₁(a) = a², h₂(b) = b², for squared loss
    fn compute_gw_loss(dist_x: &[Vec<f64>], dist_y: &[Vec<f64>], gamma: &[Vec<f64>]) -> f64 {
        let n = dist_x.len();
        let m = dist_y.len();

        // Term 1: Σᵢₖ D_X[i,k]² (Σⱼ γᵢⱼ)(Σₗ γₖₗ) = Σᵢₖ D_X[i,k]² pᵢ pₖ
        let p: Vec<f64> = gamma.iter().map(|row| row.iter().sum()).collect();
        let term1: f64 = (0..n)
            .map(|i| {
                (0..n)
                    .map(|k| dist_x[i][k].powi(2) * p[i] * p[k])
                    .sum::<f64>()
            })
            .sum();

        // Term 2: Σⱼₗ D_Y[j,l]² (Σᵢ γᵢⱼ)(Σₖ γₖₗ) = Σⱼₗ D_Y[j,l]² qⱼ qₗ
        let q: Vec<f64> = (0..m)
            .map(|j| gamma.iter().map(|row| row[j]).sum())
            .collect();
        let term2: f64 = (0..m)
            .map(|j| {
                (0..m)
                    .map(|l| dist_y[j][l].powi(2) * q[j] * q[l])
                    .sum::<f64>()
            })
            .sum();

        // Term 3: 2 * Σᵢⱼₖₗ D_X[i,k] D_Y[j,l] γᵢⱼ γₖₗ = 2 * trace(D_X γ D_Y^T γ^T)
        // = 2 * Σᵢⱼ (D_X γ)ᵢⱼ (γ D_Y^T)ᵢⱼ
        let dx_gamma: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..m)
                    .map(|j| (0..n).map(|k| dist_x[i][k] * gamma[k][j]).sum())
                    .collect()
            })
            .collect();

        let gamma_dy: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..m)
                    .map(|j| (0..m).map(|l| gamma[i][l] * dist_y[l][j]).sum())
                    .collect()
            })
            .collect();

        let term3: f64 = 2.0
            * (0..n)
                .map(|i| (0..m).map(|j| dx_gamma[i][j] * gamma_dy[i][j]).sum::<f64>())
                .sum::<f64>();

        term1 + term2 - term3
    }

    /// Compute gradient of GW loss w.r.t. gamma
    /// ∇_γ L = 2 * (h₁(D_X) p 1^T + 1 q^T h₂(D_Y) - 2 D_X γ D_Y^T)
    fn compute_gradient(
        dist_x: &[Vec<f64>],
        dist_y: &[Vec<f64>],
        gamma: &[Vec<f64>],
    ) -> Vec<Vec<f64>> {
        let n = dist_x.len();
        let m = dist_y.len();

        // Marginals
        let p: Vec<f64> = gamma.iter().map(|row| row.iter().sum()).collect();
        let q: Vec<f64> = (0..m)
            .map(|j| gamma.iter().map(|row| row[j]).sum())
            .collect();

        // D_X² p 1^T term
        let dx2_p: Vec<f64> = (0..n)
            .map(|i| (0..n).map(|k| dist_x[i][k].powi(2) * p[k]).sum())
            .collect();

        // 1 q^T D_Y² term
        let dy2_q: Vec<f64> = (0..m)
            .map(|j| (0..m).map(|l| dist_y[j][l].powi(2) * q[l]).sum())
            .collect();

        // D_X γ D_Y^T
        let dx_gamma_dy: Vec<Vec<f64>> = (0..n)
            .map(|i| {
                (0..m)
                    .map(|j| {
                        (0..n)
                            .map(|k| {
                                (0..m)
                                    .map(|l| dist_x[i][k] * gamma[k][l] * dist_y[l][j])
                                    .sum::<f64>()
                            })
                            .sum()
                    })
                    .collect()
            })
            .collect();

        // Gradient = 2 * (dx2_p 1^T + 1 dy2_q^T - 2 * D_X γ D_Y^T)
        (0..n)
            .map(|i| {
                (0..m)
                    .map(|j| 2.0 * (dx2_p[i] + dy2_q[j] - 2.0 * dx_gamma_dy[i][j]))
                    .collect()
            })
            .collect()
    }

    /// Solve Gromov-Wasserstein using Frank-Wolfe
    pub fn solve(
        &self,
        source: &[Vec<f64>],
        target: &[Vec<f64>],
    ) -> Result<GromovWassersteinResult> {
        if source.is_empty() || target.is_empty() {
            return Err(MathError::empty_input("points"));
        }

        let n = source.len();
        let m = target.len();

        // Compute distance matrices
        let dist_x = Self::distance_matrix(source);
        let dist_y = Self::distance_matrix(target);

        // Initialize with independent coupling
        let mut gamma: Vec<Vec<f64>> = (0..n).map(|_| vec![1.0 / (n * m) as f64; m]).collect();

        let sinkhorn = SinkhornSolver::new(self.regularization, self.inner_iterations);
        let source_weights = vec![1.0 / n as f64; n];
        let target_weights = vec![1.0 / m as f64; m];

        let mut loss = Self::compute_gw_loss(&dist_x, &dist_y, &gamma);
        let mut converged = false;

        for _iter in 0..self.max_iterations {
            // Compute gradient (cost matrix for linearized problem)
            let gradient = Self::compute_gradient(&dist_x, &dist_y, &gamma);

            // Solve linearized problem with Sinkhorn
            let linear_result = sinkhorn.solve(&gradient, &source_weights, &target_weights)?;
            let direction = linear_result.plan;

            // Line search
            let mut best_alpha = 0.0;
            let mut best_loss = loss;

            for k in 1..=10 {
                let alpha = k as f64 / 10.0;

                // gamma_new = (1 - alpha) * gamma + alpha * direction
                let gamma_new: Vec<Vec<f64>> = (0..n)
                    .map(|i| {
                        (0..m)
                            .map(|j| (1.0 - alpha) * gamma[i][j] + alpha * direction[i][j])
                            .collect()
                    })
                    .collect();

                let new_loss = Self::compute_gw_loss(&dist_x, &dist_y, &gamma_new);

                if new_loss < best_loss {
                    best_alpha = alpha;
                    best_loss = new_loss;
                }
            }

            // Update gamma
            if best_alpha > 0.0 {
                for i in 0..n {
                    for j in 0..m {
                        gamma[i][j] =
                            (1.0 - best_alpha) * gamma[i][j] + best_alpha * direction[i][j];
                    }
                }
            }

            // Check convergence
            let loss_change = (loss - best_loss).abs() / (loss.abs() + EPS);
            loss = best_loss;

            if loss_change < self.threshold {
                converged = true;
                break;
            }
        }

        Ok(GromovWassersteinResult {
            transport_plan: gamma,
            loss,
            converged,
        })
    }

    /// Compute GW distance between two point clouds
    pub fn distance(&self, source: &[Vec<f64>], target: &[Vec<f64>]) -> Result<f64> {
        let result = self.solve(source, target)?;
        Ok(result.loss.sqrt())
    }
}

/// Result of Gromov-Wasserstein computation
#[derive(Debug, Clone)]
pub struct GromovWassersteinResult {
    /// Optimal transport plan
    pub transport_plan: Vec<Vec<f64>>,
    /// GW loss value
    pub loss: f64,
    /// Whether algorithm converged
    pub converged: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gw_identical() {
        let gw = GromovWasserstein::new(0.1);

        let points = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        let dist = gw.distance(&points, &points).unwrap();
        // GW with entropic regularization won't be exactly 0 for identical structures
        assert!(
            dist < 1.0,
            "Identical structures should have low GW: {}",
            dist
        );
    }

    #[test]
    fn test_gw_scaled() {
        let gw = GromovWasserstein::new(0.1);

        let source = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.0, 1.0]];

        // Scale by 2 - structure is preserved!
        let target: Vec<Vec<f64>> = source
            .iter()
            .map(|p| vec![p[0] * 2.0, p[1] * 2.0])
            .collect();

        let dist = gw.distance(&source, &target).unwrap();

        // GW is NOT invariant to scaling (distances change)
        // But relative structure is preserved
        assert!(dist > 0.0, "Scaled structure should have some GW distance");
    }

    #[test]
    fn test_gw_different_structures() {
        let gw = GromovWasserstein::new(0.1);

        // Triangle
        let triangle = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![0.5, 0.866]];

        // Line
        let line = vec![vec![0.0, 0.0], vec![1.0, 0.0], vec![2.0, 0.0]];

        let dist = gw.distance(&triangle, &line).unwrap();

        // Different structures should have larger GW distance
        assert!(
            dist > 0.1,
            "Different structures should have high GW: {}",
            dist
        );
    }

    #[test]
    fn test_distance_matrix() {
        let points = vec![vec![0.0, 0.0], vec![3.0, 4.0]];
        let dist = GromovWasserstein::distance_matrix(&points);

        assert!((dist[0][1] - 5.0).abs() < 1e-10);
        assert!((dist[1][0] - 5.0).abs() < 1e-10);
        assert!(dist[0][0].abs() < 1e-10);
    }
}
