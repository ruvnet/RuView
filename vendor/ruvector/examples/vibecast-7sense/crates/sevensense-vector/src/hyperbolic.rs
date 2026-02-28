//! Hyperbolic geometry operations for hierarchical embeddings.
//!
//! This module implements operations in the Poincare ball model of hyperbolic space,
//! which is particularly useful for representing hierarchical relationships
//! in embeddings (e.g., taxonomy trees, part-whole relationships).
//!
//! ## Poincare Ball Model
//!
//! The Poincare ball is the open unit ball B^n = {x in R^n : ||x|| < 1}
//! equipped with the Riemannian metric:
//!
//! g_x = (2 / (1 - ||x||^2))^2 * I
//!
//! This metric causes distances to grow exponentially near the boundary,
//! making it ideal for tree-like structures.
//!
//! ## Key Operations
//!
//! - `exp_map`: Project from tangent space to hyperbolic space
//! - `log_map`: Project from hyperbolic space to tangent space
//! - `mobius_add`: Gyrovector addition (parallel transport)
//! - `poincare_distance`: Geodesic distance on the manifold

#![allow(dead_code)]  // Hyperbolic geometry utilities for future use

/// Default curvature for the Poincare ball model.
/// Negative curvature corresponds to hyperbolic space.
pub const DEFAULT_CURVATURE: f32 = -1.0;

/// Epsilon for numerical stability.
const EPS: f32 = 1e-7;

/// Maximum norm to prevent points from reaching the boundary.
const MAX_NORM: f32 = 1.0 - 1e-5;

/// Compute the Poincare distance between two points in the Poincare ball.
///
/// The geodesic distance in the Poincare ball model is:
///
/// d(u, v) = (1/sqrt(-c)) * arcosh(1 + 2 * ||u - v||^2 / ((1 - ||u||^2) * (1 - ||v||^2)))
///
/// where c is the (negative) curvature.
///
/// # Arguments
/// * `u` - First point in the Poincare ball
/// * `v` - Second point in the Poincare ball
/// * `curvature` - Curvature of the space (negative for hyperbolic)
///
/// # Returns
/// The geodesic distance between u and v.
pub fn poincare_distance(u: &[f32], v: &[f32], curvature: f32) -> f32 {
    debug_assert_eq!(u.len(), v.len(), "Vector length mismatch");
    debug_assert!(curvature < 0.0, "Curvature must be negative for hyperbolic space");

    let sqrt_c = (-curvature).sqrt();

    let norm_u_sq = squared_norm(u);
    let norm_v_sq = squared_norm(v);

    // Clamp norms to ensure they're inside the ball
    let norm_u_sq = norm_u_sq.min(MAX_NORM * MAX_NORM);
    let norm_v_sq = norm_v_sq.min(MAX_NORM * MAX_NORM);

    let diff_sq = squared_distance(u, v);

    let denominator = (1.0 - norm_u_sq) * (1.0 - norm_v_sq);
    let argument = 1.0 + 2.0 * diff_sq / (denominator + EPS);

    // arcosh(x) = ln(x + sqrt(x^2 - 1))
    let arcosh_val = (argument + (argument * argument - 1.0).max(0.0).sqrt()).ln();

    arcosh_val / sqrt_c
}

/// Exponential map: project from tangent space at origin to the Poincare ball.
///
/// Maps a Euclidean vector v from the tangent space T_0 B^n at the origin
/// to a point on the Poincare ball.
///
/// exp_0(v) = tanh(sqrt(-c) * ||v|| / 2) * v / (sqrt(-c) * ||v||)
///
/// # Arguments
/// * `v` - Vector in tangent space
/// * `curvature` - Curvature of the space (negative for hyperbolic)
///
/// # Returns
/// Point in the Poincare ball.
pub fn exp_map(v: &[f32], curvature: f32) -> Vec<f32> {
    let sqrt_c = (-curvature).sqrt();
    let norm_v = l2_norm(v);

    if norm_v < EPS {
        return vec![0.0; v.len()];
    }

    let scale = (sqrt_c * norm_v / 2.0).tanh() / (sqrt_c * norm_v);

    v.iter().map(|&x| x * scale).collect()
}

/// Exponential map from an arbitrary base point.
///
/// exp_x(v) = mobius_add(x, exp_0(v), c)
///
/// # Arguments
/// * `x` - Base point in the Poincare ball
/// * `v` - Vector in tangent space at x
/// * `curvature` - Curvature of the space
pub fn exp_map_at(x: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    let exp_v = exp_map(v, curvature);
    mobius_add(x, &exp_v, curvature)
}

/// Logarithmic map: project from Poincare ball to tangent space at origin.
///
/// Inverse of the exponential map.
///
/// log_0(y) = (2 / sqrt(-c)) * arctanh(sqrt(-c) * ||y||) * y / ||y||
///
/// # Arguments
/// * `y` - Point in the Poincare ball
/// * `curvature` - Curvature of the space (negative for hyperbolic)
///
/// # Returns
/// Vector in tangent space at origin.
pub fn log_map(y: &[f32], curvature: f32) -> Vec<f32> {
    let sqrt_c = (-curvature).sqrt();
    let norm_y = l2_norm(y).min(MAX_NORM);

    if norm_y < EPS {
        return vec![0.0; y.len()];
    }

    let scale = (2.0 / sqrt_c) * (sqrt_c * norm_y).atanh() / norm_y;

    y.iter().map(|&x| x * scale).collect()
}

/// Logarithmic map from an arbitrary base point.
///
/// log_x(y) = log_0(mobius_add(-x, y, c))
///
/// # Arguments
/// * `x` - Base point in the Poincare ball
/// * `y` - Target point in the Poincare ball
/// * `curvature` - Curvature of the space
pub fn log_map_at(x: &[f32], y: &[f32], curvature: f32) -> Vec<f32> {
    let neg_x: Vec<f32> = x.iter().map(|&v| -v).collect();
    let diff = mobius_add(&neg_x, y, curvature);
    log_map(&diff, curvature)
}

/// Mobius addition (gyrovector addition).
///
/// The Mobius addition is the binary operation in the Poincare ball
/// that generalizes vector addition. It can be seen as parallel transport
/// followed by addition.
///
/// u ⊕ v = ((1 + 2c<u,v> + c||v||^2)u + (1 - c||u||^2)v) /
///         (1 + 2c<u,v> + c^2||u||^2||v||^2)
///
/// # Arguments
/// * `u` - First point in the Poincare ball
/// * `v` - Second point in the Poincare ball
/// * `curvature` - Curvature of the space (negative for hyperbolic)
///
/// # Returns
/// Result of Mobius addition u ⊕ v.
pub fn mobius_add(u: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    debug_assert_eq!(u.len(), v.len(), "Vector length mismatch");

    let c = -curvature;
    let norm_u_sq = squared_norm(u);
    let norm_v_sq = squared_norm(v);
    let dot_uv = dot_product(u, v);

    let numerator_u_coef = 1.0 + 2.0 * c * dot_uv + c * norm_v_sq;
    let numerator_v_coef = 1.0 - c * norm_u_sq;
    let denominator = 1.0 + 2.0 * c * dot_uv + c * c * norm_u_sq * norm_v_sq;

    let mut result = Vec::with_capacity(u.len());
    for i in 0..u.len() {
        let value = (numerator_u_coef * u[i] + numerator_v_coef * v[i]) / (denominator + EPS);
        result.push(value);
    }

    // Project back into the ball if needed
    project_to_ball(&mut result);
    result
}

/// Mobius scalar multiplication.
///
/// r ⊗ x = (1/sqrt(c)) * tanh(r * arctanh(sqrt(c) * ||x||)) * x / ||x||
///
/// # Arguments
/// * `r` - Scalar multiplier
/// * `x` - Point in the Poincare ball
/// * `curvature` - Curvature of the space
pub fn mobius_scalar_mul(r: f32, x: &[f32], curvature: f32) -> Vec<f32> {
    let sqrt_c = (-curvature).sqrt();
    let norm_x = l2_norm(x).min(MAX_NORM);

    if norm_x < EPS {
        return vec![0.0; x.len()];
    }

    let scale = (r * (sqrt_c * norm_x).atanh()).tanh() / (sqrt_c * norm_x);

    x.iter().map(|&v| v * scale).collect()
}

/// Compute the hyperbolic midpoint of two points.
///
/// The midpoint is the point on the geodesic between u and v
/// that is equidistant from both.
pub fn hyperbolic_midpoint(u: &[f32], v: &[f32], curvature: f32) -> Vec<f32> {
    // log_u(v) gives direction and distance to v from u
    let log_v = log_map_at(u, v, curvature);

    // Scale by 0.5 to get halfway
    let half_log: Vec<f32> = log_v.iter().map(|&x| x * 0.5).collect();

    // Map back to the ball
    exp_map_at(u, &half_log, curvature)
}

/// Compute the hyperbolic centroid of multiple points.
///
/// This is the Einstein (Frechet) mean in hyperbolic space.
pub fn hyperbolic_centroid(points: &[&[f32]], curvature: f32) -> Option<Vec<f32>> {
    if points.is_empty() {
        return None;
    }

    let dim = points[0].len();

    // Start with the Euclidean centroid projected onto the ball
    let mut centroid = vec![0.0; dim];
    for point in points {
        for (i, &v) in point.iter().enumerate() {
            centroid[i] += v;
        }
    }
    for x in centroid.iter_mut() {
        *x /= points.len() as f32;
    }
    project_to_ball(&mut centroid);

    // Iteratively refine using gradient descent
    // (simplified version - could use Riemannian gradient descent for better accuracy)
    for _ in 0..10 {
        let mut grad = vec![0.0; dim];

        for point in points {
            let log_p = log_map_at(&centroid, point, curvature);
            for (i, &v) in log_p.iter().enumerate() {
                grad[i] += v;
            }
        }

        // Average gradient
        for x in grad.iter_mut() {
            *x /= points.len() as f32;
        }

        // Update centroid
        centroid = exp_map_at(&centroid, &grad, curvature);
    }

    Some(centroid)
}

/// Convert a Euclidean embedding to a Poincare ball embedding.
///
/// Uses the exponential map at the origin.
pub fn euclidean_to_poincare(euclidean: &[f32], curvature: f32) -> Vec<f32> {
    exp_map(euclidean, curvature)
}

/// Convert a Poincare ball embedding to Euclidean space.
///
/// Uses the logarithmic map at the origin.
pub fn poincare_to_euclidean(poincare: &[f32], curvature: f32) -> Vec<f32> {
    log_map(poincare, curvature)
}

/// Project a point into the Poincare ball if it lies outside.
fn project_to_ball(v: &mut [f32]) {
    let norm = l2_norm(v);
    if norm >= MAX_NORM {
        let scale = MAX_NORM / norm;
        for x in v.iter_mut() {
            *x *= scale;
        }
    }
}

/// Compute the squared L2 norm of a vector.
#[inline]
fn squared_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum()
}

/// Compute the L2 norm of a vector.
#[inline]
fn l2_norm(v: &[f32]) -> f32 {
    squared_norm(v).sqrt()
}

/// Compute the squared Euclidean distance between two vectors.
#[inline]
fn squared_distance(u: &[f32], v: &[f32]) -> f32 {
    u.iter()
        .zip(v.iter())
        .map(|(a, b)| {
            let diff = a - b;
            diff * diff
        })
        .sum()
}

/// Compute the dot product of two vectors.
#[inline]
fn dot_product(u: &[f32], v: &[f32]) -> f32 {
    u.iter().zip(v.iter()).map(|(a, b)| a * b).sum()
}

/// Conformal factor at a point (metric scaling).
///
/// The conformal factor lambda(x) = 2 / (1 - ||x||^2)
/// determines how much distances are scaled at point x.
pub fn conformal_factor(x: &[f32]) -> f32 {
    let norm_sq = squared_norm(x).min(MAX_NORM * MAX_NORM);
    2.0 / (1.0 - norm_sq)
}

/// Check if a point is inside the Poincare ball.
pub fn is_in_ball(x: &[f32]) -> bool {
    squared_norm(x) < 1.0
}

/// Compute hyperbolic angle between vectors in tangent space.
pub fn hyperbolic_angle(u: &[f32], v: &[f32]) -> f32 {
    let norm_u = l2_norm(u);
    let norm_v = l2_norm(v);

    if norm_u < EPS || norm_v < EPS {
        return 0.0;
    }

    let cos_angle = dot_product(u, v) / (norm_u * norm_v);
    cos_angle.clamp(-1.0, 1.0).acos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_poincare_distance_same_point() {
        let u = vec![0.1, 0.2, 0.3];
        let dist = poincare_distance(&u, &u, DEFAULT_CURVATURE);
        assert_relative_eq!(dist, 0.0, epsilon = 1e-5);
    }

    #[test]
    fn test_poincare_distance_origin() {
        let origin = vec![0.0, 0.0, 0.0];
        let v = vec![0.5, 0.0, 0.0];
        let dist = poincare_distance(&origin, &v, DEFAULT_CURVATURE);
        assert!(dist > 0.0);
    }

    #[test]
    fn test_exp_log_inverse() {
        let v = vec![0.5, 0.3, 0.1];
        let exp_v = exp_map(&v, DEFAULT_CURVATURE);
        let log_exp_v = log_map(&exp_v, DEFAULT_CURVATURE);

        for (a, b) in v.iter().zip(log_exp_v.iter()) {
            assert_relative_eq!(a, b, epsilon = 1e-4);
        }
    }

    #[test]
    fn test_mobius_add_zero() {
        let u = vec![0.1, 0.2, 0.3];
        let zero = vec![0.0, 0.0, 0.0];

        let result = mobius_add(&u, &zero, DEFAULT_CURVATURE);
        for (a, b) in u.iter().zip(result.iter()) {
            assert_relative_eq!(a, b, epsilon = 1e-5);
        }
    }

    #[test]
    fn test_mobius_add_stays_in_ball() {
        let u = vec![0.8, 0.0, 0.0];
        let v = vec![0.0, 0.8, 0.0];

        let result = mobius_add(&u, &v, DEFAULT_CURVATURE);
        let norm = l2_norm(&result);
        assert!(norm < 1.0);
    }

    #[test]
    fn test_hyperbolic_midpoint() {
        let u = vec![0.1, 0.0, 0.0];
        let v = vec![0.5, 0.0, 0.0];

        let mid = hyperbolic_midpoint(&u, &v, DEFAULT_CURVATURE);

        // Midpoint should be between u and v
        assert!(mid[0] > u[0] && mid[0] < v[0]);

        // Distances should be approximately equal
        let dist_u = poincare_distance(&u, &mid, DEFAULT_CURVATURE);
        let dist_v = poincare_distance(&v, &mid, DEFAULT_CURVATURE);
        assert_relative_eq!(dist_u, dist_v, epsilon = 1e-3);
    }

    #[test]
    fn test_euclidean_poincare_conversion() {
        let euclidean = vec![0.3, 0.2, 0.1];

        let poincare = euclidean_to_poincare(&euclidean, DEFAULT_CURVATURE);
        assert!(is_in_ball(&poincare));

        let back = poincare_to_euclidean(&poincare, DEFAULT_CURVATURE);
        for (a, b) in euclidean.iter().zip(back.iter()) {
            assert_relative_eq!(a, b, epsilon = 1e-4);
        }
    }

    #[test]
    fn test_conformal_factor() {
        let origin = vec![0.0, 0.0, 0.0];
        assert_relative_eq!(conformal_factor(&origin), 2.0, epsilon = 1e-5);

        // Near boundary, factor should be large
        let near_boundary = vec![0.99, 0.0, 0.0];
        assert!(conformal_factor(&near_boundary) > 10.0);
    }

    #[test]
    fn test_is_in_ball() {
        assert!(is_in_ball(&[0.0, 0.0, 0.0]));
        assert!(is_in_ball(&[0.5, 0.5, 0.0]));
        assert!(!is_in_ball(&[1.0, 0.0, 0.0]));
        assert!(!is_in_ball(&[0.6, 0.6, 0.6])); // norm > 1
    }
}
