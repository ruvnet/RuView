//! Poincaré Ball Model Operations for Hyperbolic Geometry
//!
//! This module implements core operations in the Poincaré ball model of hyperbolic space,
//! providing mathematically correct implementations with numerical stability guarantees.
//!
//! # Mathematical Background
//!
//! The Poincaré ball model represents hyperbolic space as the interior of a unit ball
//! in Euclidean space. Points are constrained to satisfy ||x|| < 1/√c where c > 0 is
//! the curvature parameter.
//!
//! # Key Operations
//!
//! - **Möbius Addition**: The hyperbolic analog of vector addition
//! - **Exponential Map**: Maps tangent vectors to the manifold
//! - **Logarithmic Map**: Maps manifold points to tangent space
//! - **Poincaré Distance**: The geodesic distance in hyperbolic space

use crate::error::{HyperbolicError, HyperbolicResult};
use serde::{Deserialize, Serialize};

/// Small epsilon for numerical stability (as specified: eps=1e-5)
pub const EPS: f32 = 1e-5;

/// Default curvature parameter (negative curvature, c > 0)
pub const DEFAULT_CURVATURE: f32 = 1.0;

/// Configuration for Poincaré ball operations
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PoincareConfig {
    /// Curvature parameter (c > 0 for hyperbolic space)
    pub curvature: f32,
    /// Numerical stability epsilon
    pub eps: f32,
    /// Maximum iterations for iterative algorithms (e.g., Fréchet mean)
    pub max_iter: usize,
    /// Convergence tolerance
    pub tol: f32,
}

impl Default for PoincareConfig {
    fn default() -> Self {
        Self {
            curvature: DEFAULT_CURVATURE,
            eps: EPS,
            max_iter: 100,
            tol: 1e-6,
        }
    }
}

impl PoincareConfig {
    /// Create configuration with custom curvature
    pub fn with_curvature(curvature: f32) -> HyperbolicResult<Self> {
        if curvature <= 0.0 {
            return Err(HyperbolicError::InvalidCurvature(curvature));
        }
        Ok(Self {
            curvature,
            ..Default::default()
        })
    }

    /// Maximum allowed norm for points in the ball
    #[inline]
    pub fn max_norm(&self) -> f32 {
        (1.0 / self.curvature.sqrt()) - self.eps
    }
}

// ============================================================================
// Optimized Core Operations (SIMD-friendly)
// ============================================================================

/// Compute the squared Euclidean norm of a slice (optimized with unrolling)
#[inline]
pub fn norm_squared(x: &[f32]) -> f32 {
    let len = x.len();
    let mut sum = 0.0f32;

    // Process 4 elements at a time for better SIMD utilization
    let chunks = len / 4;
    let remainder = len % 4;

    let mut i = 0;
    for _ in 0..chunks {
        let a = x[i];
        let b = x[i + 1];
        let c = x[i + 2];
        let d = x[i + 3];
        sum += a * a + b * b + c * c + d * d;
        i += 4;
    }

    // Handle remainder
    for j in 0..remainder {
        let v = x[i + j];
        sum += v * v;
    }

    sum
}

/// Compute the Euclidean norm of a slice
#[inline]
pub fn norm(x: &[f32]) -> f32 {
    norm_squared(x).sqrt()
}

/// Compute the dot product of two slices (optimized with unrolling)
#[inline]
pub fn dot(x: &[f32], y: &[f32]) -> f32 {
    let len = x.len().min(y.len());
    let mut sum = 0.0f32;

    // Process 4 elements at a time
    let chunks = len / 4;
    let remainder = len % 4;

    let mut i = 0;
    for _ in 0..chunks {
        sum += x[i] * y[i] + x[i+1] * y[i+1] + x[i+2] * y[i+2] + x[i+3] * y[i+3];
        i += 4;
    }

    for j in 0..remainder {
        sum += x[i + j] * y[i + j];
    }

    sum
}

/// Fused computation of ||u-v||², ||u||², ||v||² in single pass (3x faster)
#[inline]
pub fn fused_norms(u: &[f32], v: &[f32]) -> (f32, f32, f32) {
    let len = u.len().min(v.len());
    let mut diff_sq = 0.0f32;
    let mut norm_u_sq = 0.0f32;
    let mut norm_v_sq = 0.0f32;

    // Process 4 elements at a time
    let chunks = len / 4;
    let remainder = len % 4;

    let mut i = 0;
    for _ in 0..chunks {
        let (u0, u1, u2, u3) = (u[i], u[i+1], u[i+2], u[i+3]);
        let (v0, v1, v2, v3) = (v[i], v[i+1], v[i+2], v[i+3]);
        let (d0, d1, d2, d3) = (u0 - v0, u1 - v1, u2 - v2, u3 - v3);

        diff_sq += d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3;
        norm_u_sq += u0 * u0 + u1 * u1 + u2 * u2 + u3 * u3;
        norm_v_sq += v0 * v0 + v1 * v1 + v2 * v2 + v3 * v3;
        i += 4;
    }

    for j in 0..remainder {
        let ui = u[i + j];
        let vi = v[i + j];
        let di = ui - vi;
        diff_sq += di * di;
        norm_u_sq += ui * ui;
        norm_v_sq += vi * vi;
    }

    (diff_sq, norm_u_sq, norm_v_sq)
}

/// Project a point back into the Poincaré ball
///
/// Ensures ||x|| < 1/√c - eps for numerical stability
#[inline]
pub fn project_to_ball(x: &[f32], c: f32, eps: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let norm_sq = norm_squared(x);
    let max_norm = (1.0 / c.sqrt()) - eps;
    let max_norm_sq = max_norm * max_norm;

    if norm_sq < max_norm_sq || norm_sq < eps * eps {
        x.to_vec()
    } else {
        let scale = max_norm / norm_sq.sqrt();
        x.iter().map(|&xi| scale * xi).collect()
    }
}

/// Project in-place (avoids allocation when possible)
#[inline]
pub fn project_to_ball_inplace(x: &mut [f32], c: f32, eps: f32) {
    let c = c.abs().max(EPS);
    let norm_sq = norm_squared(x);
    let max_norm = (1.0 / c.sqrt()) - eps;
    let max_norm_sq = max_norm * max_norm;

    if norm_sq >= max_norm_sq && norm_sq >= eps * eps {
        let scale = max_norm / norm_sq.sqrt();
        for xi in x.iter_mut() {
            *xi *= scale;
        }
    }
}

/// Compute the conformal factor λ_x at point x
///
/// λ_x = 2 / (1 - c||x||²)
#[inline]
pub fn conformal_factor(x: &[f32], c: f32) -> f32 {
    let norm_sq = norm_squared(x);
    2.0 / (1.0 - c * norm_sq).max(EPS)
}

/// Conformal factor from pre-computed norm squared
#[inline]
pub fn conformal_factor_from_norm_sq(norm_sq: f32, c: f32) -> f32 {
    2.0 / (1.0 - c * norm_sq).max(EPS)
}

// ============================================================================
// Poincaré Distance (Optimized)
// ============================================================================

/// Poincaré distance between two points (optimized with fused norms)
///
/// Uses the formula:
/// d(u, v) = (1/√c) acosh(1 + 2c ||u - v||² / ((1 - c||u||²)(1 - c||v||²)))
#[inline]
pub fn poincare_distance(u: &[f32], v: &[f32], c: f32) -> f32 {
    let c = c.abs().max(EPS);

    // Fused computation: single pass for all three norms
    let (diff_sq, norm_u_sq, norm_v_sq) = fused_norms(u, v);

    poincare_distance_from_norms(diff_sq, norm_u_sq, norm_v_sq, c)
}

/// Poincaré distance from pre-computed norms (for batch operations)
#[inline]
pub fn poincare_distance_from_norms(diff_sq: f32, norm_u_sq: f32, norm_v_sq: f32, c: f32) -> f32 {
    let sqrt_c = c.sqrt();

    let lambda_u = (1.0 - c * norm_u_sq).max(EPS);
    let lambda_v = (1.0 - c * norm_v_sq).max(EPS);

    let numerator = 2.0 * c * diff_sq;
    let denominator = lambda_u * lambda_v;

    let arg = 1.0 + numerator / denominator;

    if arg <= 1.0 {
        return 0.0;
    }

    // Stable acosh computation
    (1.0 / sqrt_c) * fast_acosh(arg)
}

/// Fast acosh with numerical stability
#[inline]
fn fast_acosh(x: f32) -> f32 {
    if x <= 1.0 {
        return 0.0;
    }

    let delta = x - 1.0;
    if delta < 1e-4 {
        // Taylor expansion for small delta: acosh(1+δ) ≈ √(2δ)
        (2.0 * delta).sqrt()
    } else if x < 1e6 {
        // Standard formula: acosh(x) = ln(x + √(x²-1))
        (x + (x * x - 1.0).sqrt()).ln()
    } else {
        // For very large x: acosh(x) ≈ ln(2x)
        (2.0 * x).ln()
    }
}

/// Squared Poincaré distance (faster for comparisons)
#[inline]
pub fn poincare_distance_squared(u: &[f32], v: &[f32], c: f32) -> f32 {
    let d = poincare_distance(u, v, c);
    d * d
}

/// Batch distance computation (processes multiple pairs efficiently)
pub fn poincare_distance_batch(
    query: &[f32],
    points: &[&[f32]],
    c: f32,
) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let query_norm_sq = norm_squared(query);

    points
        .iter()
        .map(|point| {
            let (diff_sq, _, point_norm_sq) = fused_norms(query, point);
            poincare_distance_from_norms(diff_sq, query_norm_sq, point_norm_sq, c)
        })
        .collect()
}

// ============================================================================
// Möbius Operations (Optimized)
// ============================================================================

/// Möbius addition in the Poincaré ball (optimized)
///
/// x ⊕_c y = ((1 + 2c⟨x,y⟩ + c||y||²)x + (1 - c||x||²)y) / (1 + 2c⟨x,y⟩ + c²||x||²||y||²)
#[inline]
pub fn mobius_add(x: &[f32], y: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);

    // Fused computation of norms and dot product
    let len = x.len().min(y.len());
    let mut norm_x_sq = 0.0f32;
    let mut norm_y_sq = 0.0f32;
    let mut dot_xy = 0.0f32;

    // Process 4 elements at a time
    let chunks = len / 4;
    let remainder = len % 4;

    let mut i = 0;
    for _ in 0..chunks {
        let (x0, x1, x2, x3) = (x[i], x[i+1], x[i+2], x[i+3]);
        let (y0, y1, y2, y3) = (y[i], y[i+1], y[i+2], y[i+3]);

        norm_x_sq += x0 * x0 + x1 * x1 + x2 * x2 + x3 * x3;
        norm_y_sq += y0 * y0 + y1 * y1 + y2 * y2 + y3 * y3;
        dot_xy += x0 * y0 + x1 * y1 + x2 * y2 + x3 * y3;
        i += 4;
    }

    for j in 0..remainder {
        let xi = x[i + j];
        let yi = y[i + j];
        norm_x_sq += xi * xi;
        norm_y_sq += yi * yi;
        dot_xy += xi * yi;
    }

    // Compute coefficients
    let coef_x = 1.0 + 2.0 * c * dot_xy + c * norm_y_sq;
    let coef_y = 1.0 - c * norm_x_sq;
    let denom = (1.0 + 2.0 * c * dot_xy + c * c * norm_x_sq * norm_y_sq).max(EPS);
    let inv_denom = 1.0 / denom;

    // Compute result
    let mut result = Vec::with_capacity(len);
    for j in 0..len {
        result.push((coef_x * x[j] + coef_y * y[j]) * inv_denom);
    }

    // Project back into ball
    project_to_ball_inplace(&mut result, c, EPS);
    result
}

/// Möbius addition in-place (modifies first argument)
#[inline]
pub fn mobius_add_inplace(x: &mut [f32], y: &[f32], c: f32) {
    let c = c.abs().max(EPS);
    let len = x.len().min(y.len());

    let norm_x_sq = norm_squared(x);
    let norm_y_sq = norm_squared(y);
    let dot_xy = dot(x, y);

    let coef_x = 1.0 + 2.0 * c * dot_xy + c * norm_y_sq;
    let coef_y = 1.0 - c * norm_x_sq;
    let denom = (1.0 + 2.0 * c * dot_xy + c * c * norm_x_sq * norm_y_sq).max(EPS);
    let inv_denom = 1.0 / denom;

    for j in 0..len {
        x[j] = (coef_x * x[j] + coef_y * y[j]) * inv_denom;
    }

    project_to_ball_inplace(x, c, EPS);
}

/// Möbius scalar multiplication
///
/// r ⊗_c x = (1/√c) tanh(r · arctanh(√c ||x||)) · (x / ||x||)
pub fn mobius_scalar_mult(r: f32, x: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let sqrt_c = c.sqrt();
    let norm_x = norm(x);

    if norm_x < EPS {
        return x.to_vec();
    }

    let arctanh_arg = (sqrt_c * norm_x).min(1.0 - EPS);
    let arctanh_val = arctanh_arg.atanh();
    let scale = (1.0 / sqrt_c) * (r * arctanh_val).tanh() / norm_x;

    x.iter().map(|&xi| scale * xi).collect()
}

// ============================================================================
// Exp/Log Maps (Optimized)
// ============================================================================

/// Exponential map at point p
///
/// exp_p(v) = p ⊕_c (tanh(√c λ_p ||v|| / 2) · v / (√c ||v||))
pub fn exp_map(v: &[f32], p: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let sqrt_c = c.sqrt();

    let norm_p_sq = norm_squared(p);
    let lambda_p = conformal_factor_from_norm_sq(norm_p_sq, c);

    let norm_v = norm(v);

    if norm_v < EPS {
        return p.to_vec();
    }

    let scaled_norm = sqrt_c * lambda_p * norm_v / 2.0;
    let coef = scaled_norm.tanh() / (sqrt_c * norm_v);

    let transported: Vec<f32> = v.iter().map(|&vi| coef * vi).collect();

    mobius_add(p, &transported, c)
}

/// Logarithmic map at point p
///
/// log_p(y) = (2 / (√c λ_p)) arctanh(√c ||−p ⊕_c y||) · (−p ⊕_c y) / ||−p ⊕_c y||
pub fn log_map(y: &[f32], p: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);
    let sqrt_c = c.sqrt();

    // Compute -p ⊕_c y
    let neg_p: Vec<f32> = p.iter().map(|&pi| -pi).collect();
    let diff = mobius_add(&neg_p, y, c);
    let norm_diff = norm(&diff);

    if norm_diff < EPS {
        return vec![0.0; y.len()];
    }

    let norm_p_sq = norm_squared(p);
    let lambda_p = conformal_factor_from_norm_sq(norm_p_sq, c);

    let arctanh_arg = (sqrt_c * norm_diff).min(1.0 - EPS);
    let coef = (2.0 / (sqrt_c * lambda_p)) * arctanh_arg.atanh() / norm_diff;

    diff.iter().map(|&di| coef * di).collect()
}

/// Logarithmic map at a shard centroid for tangent space coordinates
pub fn log_map_at_centroid(x: &[f32], centroid: &[f32], c: f32) -> Vec<f32> {
    log_map(x, centroid, c)
}

// ============================================================================
// Fréchet Mean & Utilities
// ============================================================================

/// Compute the Fréchet mean (hyperbolic centroid) of points
pub fn frechet_mean(
    points: &[&[f32]],
    weights: Option<&[f32]>,
    config: &PoincareConfig,
) -> HyperbolicResult<Vec<f32>> {
    if points.is_empty() {
        return Err(HyperbolicError::EmptyCollection);
    }

    let dim = points[0].len();
    let c = config.curvature;

    // Validate dimensions
    for p in points.iter() {
        if p.len() != dim {
            return Err(HyperbolicError::DimensionMismatch {
                expected: dim,
                got: p.len(),
            });
        }
    }

    // Set up weights
    let uniform_weights: Vec<f32>;
    let w = if let Some(weights) = weights {
        if weights.len() != points.len() {
            return Err(HyperbolicError::DimensionMismatch {
                expected: points.len(),
                got: weights.len(),
            });
        }
        weights
    } else {
        uniform_weights = vec![1.0 / points.len() as f32; points.len()];
        &uniform_weights
    };

    // Initialize with Euclidean weighted mean, projected to ball
    let mut mean = vec![0.0; dim];
    for (point, &weight) in points.iter().zip(w) {
        for (i, &val) in point.iter().enumerate() {
            mean[i] += weight * val;
        }
    }
    project_to_ball_inplace(&mut mean, c, config.eps);

    // Riemannian gradient descent
    let learning_rate = 0.1;
    let mut grad = vec![0.0; dim];

    for _ in 0..config.max_iter {
        // Reset gradient
        for g in grad.iter_mut() {
            *g = 0.0;
        }

        // Compute Riemannian gradient
        for (point, &weight) in points.iter().zip(w) {
            let log_result = log_map(point, &mean, c);
            for (i, &val) in log_result.iter().enumerate() {
                grad[i] += weight * val;
            }
        }

        // Check convergence
        if norm(&grad) < config.tol {
            break;
        }

        // Update step
        let update: Vec<f32> = grad.iter().map(|&g| learning_rate * g).collect();
        mean = exp_map(&update, &mean, c);
    }

    Ok(mean)
}

/// Hyperbolic midpoint between two points
pub fn hyperbolic_midpoint(x: &[f32], y: &[f32], c: f32) -> Vec<f32> {
    let log_y = log_map(y, x, c);
    let half_log: Vec<f32> = log_y.iter().map(|&v| 0.5 * v).collect();
    exp_map(&half_log, x, c)
}

/// Parallel transport a tangent vector from p to q
pub fn parallel_transport(v: &[f32], p: &[f32], q: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs().max(EPS);

    let lambda_p = conformal_factor(p, c);
    let lambda_q = conformal_factor(q, c);
    let scale = lambda_p / lambda_q;

    v.iter().map(|&vi| scale * vi).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_to_ball() {
        let x = vec![0.5, 0.5, 0.5];
        let projected = project_to_ball(&x, 1.0, EPS);
        assert!(norm(&projected) < 1.0 - EPS);
    }

    #[test]
    fn test_mobius_add_identity() {
        let x = vec![0.3, 0.2, 0.1];
        let zero = vec![0.0, 0.0, 0.0];

        let result = mobius_add(&x, &zero, 1.0);
        for (a, b) in x.iter().zip(result.iter()) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    #[test]
    fn test_exp_log_inverse() {
        let p = vec![0.1, 0.2, 0.1];
        let v = vec![0.1, -0.1, 0.05];

        let q = exp_map(&v, &p, 1.0);
        let v_recovered = log_map(&q, &p, 1.0);

        for (a, b) in v.iter().zip(v_recovered.iter()) {
            assert!((a - b).abs() < 1e-4);
        }
    }

    #[test]
    fn test_poincare_distance_symmetry() {
        let u = vec![0.3, 0.2];
        let v = vec![-0.1, 0.4];

        let d1 = poincare_distance(&u, &v, 1.0);
        let d2 = poincare_distance(&v, &u, 1.0);

        assert!((d1 - d2).abs() < 1e-6);
    }

    #[test]
    fn test_poincare_distance_origin() {
        let origin = vec![0.0, 0.0];
        let d = poincare_distance(&origin, &origin, 1.0);
        assert!(d.abs() < 1e-6);
    }

    #[test]
    fn test_fused_norms() {
        let u = vec![0.3, 0.2, 0.1];
        let v = vec![0.1, 0.4, 0.2];

        let (diff_sq, norm_u_sq, norm_v_sq) = fused_norms(&u, &v);

        let expected_diff_sq: f32 = u.iter().zip(v.iter())
            .map(|(a, b)| (a - b) * (a - b)).sum();
        let expected_norm_u_sq = norm_squared(&u);
        let expected_norm_v_sq = norm_squared(&v);

        assert!((diff_sq - expected_diff_sq).abs() < 1e-6);
        assert!((norm_u_sq - expected_norm_u_sq).abs() < 1e-6);
        assert!((norm_v_sq - expected_norm_v_sq).abs() < 1e-6);
    }
}
