//! Poincaré Ball Model Operations for Hyperbolic Geometry
//!
//! This module implements core operations in the Poincaré ball model of hyperbolic space,
//! providing mathematically correct implementations with numerical stability guarantees.

/// Small epsilon for numerical stability
const EPS: f32 = 1e-7;

/// Compute the squared Euclidean norm of a vector
#[inline]
fn norm_squared(x: &[f32]) -> f32 {
    x.iter().map(|&v| v * v).sum()
}

/// Compute the Euclidean norm of a vector
#[inline]
fn norm(x: &[f32]) -> f32 {
    norm_squared(x).sqrt()
}

/// Compute Poincaré distance between two points in hyperbolic space
pub fn poincare_distance(u: &[f32], v: &[f32], c: f32) -> f32 {
    let c = c.abs();
    let sqrt_c = c.sqrt();

    let diff: Vec<f32> = u.iter().zip(v).map(|(a, b)| a - b).collect();
    let norm_diff_sq = norm_squared(&diff);
    let norm_u_sq = norm_squared(u);
    let norm_v_sq = norm_squared(v);

    let lambda_u = 1.0 - c * norm_u_sq;
    let lambda_v = 1.0 - c * norm_v_sq;

    let numerator = 2.0 * c * norm_diff_sq;
    let denominator = lambda_u * lambda_v;

    let arg = 1.0 + numerator / denominator.max(EPS);
    (1.0 / sqrt_c) * arg.max(1.0).acosh()
}

/// Möbius addition in Poincaré ball
pub fn mobius_add(u: &[f32], v: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs();
    let norm_u_sq = norm_squared(u);
    let norm_v_sq = norm_squared(v);
    let dot_uv: f32 = u.iter().zip(v).map(|(a, b)| a * b).sum();

    let coef_u = 1.0 + 2.0 * c * dot_uv + c * norm_v_sq;
    let coef_v = 1.0 - c * norm_u_sq;
    let denom = 1.0 + 2.0 * c * dot_uv + c * c * norm_u_sq * norm_v_sq;

    let result: Vec<f32> = u
        .iter()
        .zip(v)
        .map(|(ui, vi)| (coef_u * ui + coef_v * vi) / denom.max(EPS))
        .collect();

    project_to_ball(&result, c, EPS)
}

/// Möbius scalar multiplication
pub fn mobius_scalar_mult(r: f32, v: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs();
    let sqrt_c = c.sqrt();
    let norm_v = norm(v);

    if norm_v < EPS {
        return v.to_vec();
    }

    let arctanh_arg = (sqrt_c * norm_v).min(1.0 - EPS);
    let scale = (1.0 / sqrt_c) * (r * arctanh_arg.atanh()).tanh() / norm_v;

    v.iter().map(|&vi| scale * vi).collect()
}

/// Exponential map: maps tangent vector v at point p to hyperbolic space
pub fn exp_map(v: &[f32], p: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs();
    let sqrt_c = c.sqrt();

    let norm_p_sq = norm_squared(p);
    let lambda_p = 1.0 / (1.0 - c * norm_p_sq).max(EPS);

    let norm_v = norm(v);
    let norm_v_p = lambda_p * norm_v;

    if norm_v < EPS {
        return p.to_vec();
    }

    let coef = (sqrt_c * norm_v_p / 2.0).tanh() / (sqrt_c * norm_v_p);
    let transported: Vec<f32> = v.iter().map(|&vi| coef * vi).collect();

    mobius_add(p, &transported, c)
}

/// Logarithmic map: maps point y to tangent space at point p
pub fn log_map(y: &[f32], p: &[f32], c: f32) -> Vec<f32> {
    let c = c.abs();
    let sqrt_c = c.sqrt();

    let neg_p: Vec<f32> = p.iter().map(|&pi| -pi).collect();
    let diff = mobius_add(&neg_p, y, c);
    let norm_diff = norm(&diff);

    if norm_diff < EPS {
        return vec![0.0; y.len()];
    }

    let norm_p_sq = norm_squared(p);
    let lambda_p = 1.0 / (1.0 - c * norm_p_sq).max(EPS);

    let arctanh_arg = (sqrt_c * norm_diff).min(1.0 - EPS);
    let coef = (2.0 / (sqrt_c * lambda_p)) * arctanh_arg.atanh() / norm_diff;

    diff.iter().map(|&di| coef * di).collect()
}

/// Project point to Poincaré ball
pub fn project_to_ball(x: &[f32], c: f32, eps: f32) -> Vec<f32> {
    let c = c.abs();
    let norm_x = norm(x);
    let max_norm = (1.0 / c.sqrt()) - eps;

    if norm_x < max_norm {
        x.to_vec()
    } else {
        let scale = max_norm / norm_x.max(EPS);
        x.iter().map(|&xi| scale * xi).collect()
    }
}

/// Compute the Fréchet mean (centroid) of points in hyperbolic space
pub fn frechet_mean(
    points: &[&[f32]],
    weights: Option<&[f32]>,
    c: f32,
    max_iter: usize,
    tol: f32,
) -> Vec<f32> {
    let dim = points[0].len();
    let c = c.abs();

    let uniform_weights: Vec<f32>;
    let w = if let Some(weights) = weights {
        weights
    } else {
        uniform_weights = vec![1.0 / points.len() as f32; points.len()];
        &uniform_weights
    };

    let mut mean = vec![0.0; dim];
    for (point, &weight) in points.iter().zip(w) {
        for (i, &val) in point.iter().enumerate() {
            mean[i] += weight * val;
        }
    }
    mean = project_to_ball(&mean, c, EPS);

    let learning_rate = 0.1;
    for _ in 0..max_iter {
        let mut grad = vec![0.0; dim];
        for (point, &weight) in points.iter().zip(w) {
            let log_map_result = log_map(point, &mean, c);
            for (i, &val) in log_map_result.iter().enumerate() {
                grad[i] += weight * val;
            }
        }

        if norm(&grad) < tol {
            break;
        }

        let update: Vec<f32> = grad.iter().map(|&g| learning_rate * g).collect();
        mean = exp_map(&update, &mean, c);
    }

    project_to_ball(&mean, c, EPS)
}
