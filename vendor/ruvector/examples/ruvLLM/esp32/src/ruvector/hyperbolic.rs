//! Hyperbolic Embeddings for RuvLLM ESP32
//!
//! Implements hyperbolic geometry distance metrics optimized for microcontrollers.
//! Hyperbolic spaces are ideal for hierarchical data (taxonomies, knowledge graphs)
//! as they naturally represent tree-like structures with exponentially growing space.
//!
//! # Models
//!
//! ## Poincaré Ball Model
//! - Points in unit ball: ||x|| < 1
//! - Conformal (preserves angles)
//! - Distance: d(x,y) = arcosh(1 + 2||x-y||² / ((1-||x||²)(1-||y||²)))
//!
//! ## Lorentz (Hyperboloid) Model
//! - Points on hyperboloid: -x₀² + x₁² + ... + xₙ² = -1, x₀ > 0
//! - More numerically stable
//! - Distance: d(x,y) = arcosh(-⟨x,y⟩_L)

use heapless::Vec as HVec;
use libm::{acoshf, sqrtf};

/// Scale factor for INT8 to float conversion
const POINCARE_SCALE: f32 = 127.0 / 0.787;

/// Default curvature of hyperbolic space
const DEFAULT_CURVATURE: f32 = -1.0;

/// Hyperbolic embedding configuration
#[derive(Debug, Clone, Copy)]
pub struct HyperbolicConfig {
    /// Curvature of the hyperbolic space (negative value)
    pub curvature: f32,
    /// Dimension of the embedding
    pub dim: usize,
    /// Epsilon for numerical stability
    pub eps: f32,
}

impl Default for HyperbolicConfig {
    fn default() -> Self {
        Self {
            curvature: DEFAULT_CURVATURE,
            dim: 32,
            eps: 1e-5,
        }
    }
}

/// Poincaré distance between two INT8 vectors
pub fn poincare_distance_i8(a: &[i8], b: &[i8]) -> i32 {
    let c = 1.0; // |curvature|
    let scale = 1.0 / POINCARE_SCALE;

    let mut norm_a_sq: f32 = 0.0;
    let mut norm_b_sq: f32 = 0.0;
    let mut diff_sq: f32 = 0.0;

    for (x, y) in a.iter().zip(b.iter()) {
        let xf = (*x as f32) * scale;
        let yf = (*y as f32) * scale;
        norm_a_sq += xf * xf;
        norm_b_sq += yf * yf;
        diff_sq += (xf - yf) * (xf - yf);
    }

    // Clamp norms to stay inside ball
    let max_norm = 1.0 - 1e-5;
    norm_a_sq = norm_a_sq.min(max_norm * max_norm);
    norm_b_sq = norm_b_sq.min(max_norm * max_norm);

    let numerator = 2.0 * c * diff_sq;
    let denom_a = 1.0 - c * norm_a_sq;
    let denom_b = 1.0 - c * norm_b_sq;
    let denominator = denom_a * denom_b;

    if denominator < 1e-10 {
        return i32::MAX / 2;
    }

    let arg = (1.0 + numerator / denominator).max(1.0);
    let dist = acoshf(arg);

    (dist * 1000.0) as i32
}

/// Lorentz distance from spatial coordinates
pub fn lorentz_distance_spatial_i8(a: &[i8], b: &[i8]) -> i32 {
    let scale = 1.0 / POINCARE_SCALE;
    let k = 1.0; // 1/|c| for c = -1

    let mut norm_a_sq: f32 = 0.0;
    let mut norm_b_sq: f32 = 0.0;
    let mut spatial_dot: f32 = 0.0;

    for (x, y) in a.iter().zip(b.iter()) {
        let xf = (*x as f32) * scale;
        let yf = (*y as f32) * scale;
        norm_a_sq += xf * xf;
        norm_b_sq += yf * yf;
        spatial_dot += xf * yf;
    }

    // Compute timelike components: x₀ = √(k + ||x||²)
    let t_a = sqrtf(k + norm_a_sq);
    let t_b = sqrtf(k + norm_b_sq);

    // Lorentz inner product: -t_a*t_b + spatial_dot
    let inner = -t_a * t_b + spatial_dot;
    let arg = (-inner).max(1.0);
    let dist = acoshf(arg);

    (dist * 1000.0) as i32
}

/// Convert Euclidean INT8 vector to Poincaré ball
pub fn to_poincare_i8(euclidean: &[i8]) -> HVec<i8, 64> {
    let mut result: HVec<i8, 64> = HVec::new();

    let mut norm_sq: f32 = 0.0;
    for x in euclidean {
        let xf = *x as f32;
        norm_sq += xf * xf;
    }
    let norm = sqrtf(norm_sq);

    if norm < 1e-6 {
        for _ in 0..euclidean.len() {
            let _ = result.push(0);
        }
        return result;
    }

    let scale = (norm / (2.0 * POINCARE_SCALE)).tanh() * POINCARE_SCALE / norm;

    for x in euclidean {
        let mapped = ((*x as f32) * scale).clamp(-127.0, 127.0) as i8;
        let _ = result.push(mapped);
    }

    result
}

/// Convert Euclidean INT8 vector to Lorentz hyperboloid
pub fn to_lorentz_i8(spatial: &[i8]) -> HVec<i8, 65> {
    let mut result: HVec<i8, 65> = HVec::new();
    let scale = 1.0 / POINCARE_SCALE;

    let mut norm_sq: f32 = 0.0;
    for x in spatial {
        let xf = (*x as f32) * scale;
        norm_sq += xf * xf;
    }

    let t = sqrtf(1.0 + norm_sq);
    let t_scaled = (t * 127.0).clamp(-127.0, 127.0) as i8;
    let _ = result.push(t_scaled);

    for x in spatial {
        let _ = result.push(*x);
    }

    result
}

/// Hyperbolic midpoint between two points (Poincaré ball)
pub fn hyperbolic_midpoint(a: &[i8], b: &[i8]) -> HVec<i8, 64> {
    let scale = 1.0 / POINCARE_SCALE;
    let mut result: HVec<i8, 64> = HVec::new();

    // Simple approximation: weighted average scaled back
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = (*x as f32) * scale;
        let yf = (*y as f32) * scale;
        let mid = (xf + yf) * 0.5;
        let mapped = (mid * POINCARE_SCALE).clamp(-127.0, 127.0) as i8;
        let _ = result.push(mapped);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_distance_zero() {
        let a = [0i8, 0, 0, 0];
        let b = [0i8, 0, 0, 0];
        let dist = poincare_distance_i8(&a, &b);
        assert!(dist < 10, "Distance at origin should be ~0, got {}", dist);
    }

    #[test]
    fn test_poincare_distance_symmetric() {
        let a = [10i8, 20, 30, 40];
        let b = [50i8, 60, 70, 80];
        let d1 = poincare_distance_i8(&a, &b);
        let d2 = poincare_distance_i8(&b, &a);
        assert_eq!(d1, d2, "Distance should be symmetric");
    }

    #[test]
    fn test_poincare_distance_triangle_inequality() {
        let a = [10i8, 0, 0, 0];
        let b = [0i8, 10, 0, 0];
        let c = [0i8, 0, 10, 0];
        let ab = poincare_distance_i8(&a, &b);
        let bc = poincare_distance_i8(&b, &c);
        let ac = poincare_distance_i8(&a, &c);
        assert!(ac <= ab + bc + 1, "Triangle inequality violated");
    }

    #[test]
    fn test_lorentz_distance_spatial() {
        let a = [10i8, 20, 30];
        let b = [60i8, 70, 80];
        let dist = lorentz_distance_spatial_i8(&a, &b);
        assert!(dist >= 0, "Distance should be non-negative, got {}", dist);
        let zero_dist = lorentz_distance_spatial_i8(&a, &a);
        assert!(zero_dist < 10, "Same point distance should be ~0, got {}", zero_dist);
    }

    #[test]
    fn test_lorentz_distance_symmetric() {
        let a = [10i8, 20, 30];
        let b = [50i8, 60, 70];
        let d1 = lorentz_distance_spatial_i8(&a, &b);
        let d2 = lorentz_distance_spatial_i8(&b, &a);
        assert_eq!(d1, d2, "Lorentz distance should be symmetric");
    }

    #[test]
    fn test_to_poincare_origin() {
        let euclidean = [0i8, 0, 0, 0];
        let poincare = to_poincare_i8(&euclidean);
        for x in poincare.iter() {
            assert_eq!(*x, 0, "Origin should map to origin");
        }
    }

    #[test]
    fn test_to_lorentz() {
        let spatial = [50i8, 50, 50];
        let lorentz = to_lorentz_i8(&spatial);
        assert!(lorentz[0] > 0, "Timelike component should be positive");
        assert_eq!(lorentz.len(), spatial.len() + 1, "Should add timelike component");
    }

    #[test]
    fn test_hyperbolic_midpoint() {
        let a = [20i8, 0, 0, 0];
        let b = [-20i8, 0, 0, 0];
        let mid = hyperbolic_midpoint(&a, &b);
        let norm: i32 = mid.iter().map(|&x| (x as i32).abs()).sum();
        assert!(norm < 50, "Midpoint of symmetric points should be near origin");
    }

    #[test]
    fn test_boundary_behavior() {
        let center = [0i8, 0, 0, 0];
        let near_boundary = [120i8, 0, 0, 0];
        let dist = poincare_distance_i8(&center, &near_boundary);
        assert!(dist > 500, "Distance to boundary should be large");
    }
}
