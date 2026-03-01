//! Fisher Information Metric
//!
//! The Fisher metric on the probability simplex:
//! F = diag(p) - p*p^T
//!
//! This gives the natural geometry for probability distributions.

use serde::{Deserialize, Serialize};

/// Fisher metric configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FisherConfig {
    /// Regularization epsilon for numerical stability
    pub eps: f32,
    /// Maximum CG iterations
    pub max_iters: usize,
    /// Convergence threshold
    pub tol: f32,
}

impl Default for FisherConfig {
    fn default() -> Self {
        Self {
            eps: 1e-8,
            max_iters: 10,
            tol: 1e-6,
        }
    }
}

/// Fisher metric operations
#[derive(Debug, Clone)]
pub struct FisherMetric {
    config: FisherConfig,
}

impl FisherMetric {
    /// Create new Fisher metric
    pub fn new(config: FisherConfig) -> Self {
        Self { config }
    }

    /// Apply Fisher matrix to vector: F*v = diag(p)*v - p*(p^T*v)
    /// This is O(n) instead of O(n^2)
    #[inline]
    pub fn apply(&self, probs: &[f32], v: &[f32]) -> Vec<f32> {
        let n = probs.len().min(v.len()); // Security: bounds check

        if n == 0 {
            return vec![];
        }

        // Compute p^T * v
        let pv = Self::dot_simd(probs, v);

        // F*v = diag(p)*v - p*(p^T*v)
        let mut result = vec![0.0f32; n];
        for i in 0..n {
            result[i] = probs[i] * v[i] - probs[i] * pv;
        }

        result
    }

    /// Apply inverse Fisher (approximately) using diagonal preconditioning
    /// F^{-1} ≈ diag(1/p) for small perturbations
    #[inline]
    pub fn apply_inverse_approx(&self, probs: &[f32], v: &[f32]) -> Vec<f32> {
        let n = probs.len().min(v.len()); // Security: bounds check

        if n == 0 {
            return vec![];
        }

        let mut result = vec![0.0f32; n];

        for i in 0..n {
            let p = probs[i].max(self.config.eps);
            result[i] = v[i] / p;
        }

        // Project to sum-zero (tangent space of simplex)
        let mean: f32 = result.iter().sum::<f32>() / n as f32;
        for i in 0..n {
            result[i] -= mean;
        }

        result
    }

    /// Solve F*x = b using conjugate gradient
    /// Returns x such that probs[i]*x[i] - probs[i]*sum(probs[j]*x[j]) ≈ b[i]
    pub fn solve_cg(&self, probs: &[f32], b: &[f32]) -> Vec<f32> {
        let n = probs.len().min(b.len()); // Security: bounds check

        if n == 0 {
            return vec![];
        }

        // Project b to sum-zero (must be in tangent space)
        let mut b_proj = b[..n].to_vec();
        let b_mean: f32 = b_proj.iter().sum::<f32>() / n as f32;
        for i in 0..n {
            b_proj[i] -= b_mean;
        }

        // CG iteration
        let mut x = vec![0.0f32; n];
        let mut r = b_proj.clone();
        let mut d = r.clone();

        let mut rtr = Self::dot_simd(&r, &r);
        if rtr < self.config.tol {
            return x;
        }

        for _ in 0..self.config.max_iters {
            let fd = self.apply(probs, &d);
            let dfd = Self::dot_simd(&d, &fd).max(self.config.eps);
            let alpha = rtr / dfd;

            for i in 0..n {
                x[i] += alpha * d[i];
                r[i] -= alpha * fd[i];
            }

            let rtr_new = Self::dot_simd(&r, &r);
            if rtr_new < self.config.tol {
                break;
            }

            let beta = rtr_new / rtr.max(self.config.eps); // Security: prevent division by zero
            for i in 0..n {
                d[i] = r[i] + beta * d[i];
            }

            rtr = rtr_new;
        }

        x
    }

    /// Compute Fisher-Rao distance between two probability distributions
    /// d_FR(p, q) = 2 * arccos(sum(sqrt(p_i * q_i)))
    pub fn fisher_rao_distance(&self, p: &[f32], q: &[f32]) -> f32 {
        let n = p.len().min(q.len());
        let mut bhattacharyya = 0.0f32;

        for i in 0..n {
            let pi = p[i].max(self.config.eps);
            let qi = q[i].max(self.config.eps);
            bhattacharyya += (pi * qi).sqrt();
        }

        // Clamp for numerical stability
        let cos_half = bhattacharyya.clamp(0.0, 1.0);
        2.0 * cos_half.acos()
    }

    /// SIMD-friendly dot product
    #[inline(always)]
    fn dot_simd(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len().min(b.len());
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f32;
        let mut sum1 = 0.0f32;
        let mut sum2 = 0.0f32;
        let mut sum3 = 0.0f32;

        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] * b[base];
            sum1 += a[base + 1] * b[base + 1];
            sum2 += a[base + 2] * b[base + 2];
            sum3 += a[base + 3] * b[base + 3];
        }

        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] * b[base + i];
        }

        sum0 + sum1 + sum2 + sum3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fisher_apply() {
        let fisher = FisherMetric::new(FisherConfig::default());

        // Uniform distribution
        let p = vec![0.25, 0.25, 0.25, 0.25];
        let v = vec![1.0, 0.0, 0.0, -1.0];

        let fv = fisher.apply(&p, &v);

        // F*v should be in tangent space (sum to ~0)
        let sum: f32 = fv.iter().sum();
        assert!(sum.abs() < 1e-5);
    }

    #[test]
    fn test_fisher_cg_solve() {
        let fisher = FisherMetric::new(FisherConfig::default());

        let p = vec![0.4, 0.3, 0.2, 0.1];
        let b = vec![0.1, -0.05, -0.02, -0.03]; // sum-zero

        let x = fisher.solve_cg(&p, &b);

        // F*x should approximately equal b
        let fx = fisher.apply(&p, &x);

        for i in 0..4 {
            assert!((fx[i] - b[i]).abs() < 0.1);
        }
    }

    #[test]
    fn test_fisher_rao_distance() {
        let fisher = FisherMetric::new(FisherConfig::default());

        let p = vec![0.5, 0.5];
        let q = vec![0.5, 0.5];

        // Same distribution = 0 distance
        let d = fisher.fisher_rao_distance(&p, &q);
        assert!(d.abs() < 1e-5);

        // Different distributions
        let q2 = vec![0.9, 0.1];
        let d2 = fisher.fisher_rao_distance(&p, &q2);
        assert!(d2 > 0.0);
    }
}
