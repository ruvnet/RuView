// Poincaré Ball Model Implementation
// Implements conformal model of hyperbolic space

use crate::hyperbolic::{EPSILON, MAX_NORM};
use simsimd::SpatialSimilarity;

/// Poincaré ball model for hyperbolic space
pub struct PoincareBall {
    /// Curvature of the hyperbolic space (typically -1.0)
    pub curvature: f32,
}

impl PoincareBall {
    /// Create a new Poincaré ball with specified curvature
    pub fn new(curvature: f32) -> Self {
        assert!(curvature < 0.0, "Curvature must be negative");
        Self { curvature }
    }

    /// Compute squared norm of a vector
    #[inline]
    fn norm_squared(&self, x: &[f32]) -> f32 {
        (f32::dot(x, x).unwrap_or(0.0) as f32).max(0.0)
    }

    /// Compute norm of a vector
    #[inline]
    fn norm(&self, x: &[f32]) -> f32 {
        self.norm_squared(x).sqrt()
    }

    /// Project vector to within the Poincaré ball
    pub fn project(&self, x: &[f32]) -> Vec<f32> {
        let norm = self.norm(x);
        if norm < MAX_NORM {
            x.to_vec()
        } else {
            // Scale to MAX_NORM to stay within ball
            let scale = MAX_NORM / (norm + EPSILON);
            x.iter().map(|&v| v * scale).collect()
        }
    }

    /// Compute Poincaré distance between two points
    /// d(x, y) = acosh(1 + 2 * ||x - y||² / ((1 - ||x||²)(1 - ||y||²)))
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        assert_eq!(x.len(), y.len(), "Vectors must have same dimension");

        let x_norm_sq = self.norm_squared(x);
        let y_norm_sq = self.norm_squared(y);

        // Compute ||x - y||²
        let diff: Vec<f32> = x.iter().zip(y.iter()).map(|(&a, &b)| a - b).collect();
        let diff_norm_sq = self.norm_squared(&diff);

        // Compute conformal factors
        let x_factor = 1.0 - x_norm_sq;
        let y_factor = 1.0 - y_norm_sq;

        // Prevent division by zero
        if x_factor <= EPSILON || y_factor <= EPSILON {
            return f32::INFINITY;
        }

        // d(x, y) = acosh(1 + 2 * ||x - y||² / ((1 - ||x||²)(1 - ||y||²)))
        let numerator = 2.0 * diff_norm_sq;
        let denominator = x_factor * y_factor;
        let ratio = numerator / (denominator + EPSILON);

        let arg = 1.0 + ratio;
        let distance = arg.acosh();

        // Scale by curvature
        let k = self.curvature.abs().sqrt();
        distance / k
    }

    /// Möbius addition: x ⊕ y
    /// Formula: (1 + 2⟨x,y⟩ + ||y||²)x + (1 - ||x||²)y / (1 + 2⟨x,y⟩ + ||x||²||y||²)
    pub fn mobius_add(&self, x: &[f32], y: &[f32]) -> Vec<f32> {
        assert_eq!(x.len(), y.len(), "Vectors must have same dimension");

        let x_norm_sq = self.norm_squared(x);
        let y_norm_sq = self.norm_squared(y);
        let xy_dot = f32::dot(x, y).unwrap_or(0.0) as f32;

        let numerator_x_coeff = 1.0f32 + 2.0f32 * xy_dot + y_norm_sq;
        let numerator_y_coeff = 1.0f32 - x_norm_sq;
        let denominator = 1.0f32 + 2.0f32 * xy_dot + x_norm_sq * y_norm_sq + EPSILON;

        let result: Vec<f32> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| (numerator_x_coeff * xi + numerator_y_coeff * yi) / denominator)
            .collect();

        self.project(&result)
    }

    /// Exponential map: exp_x(v) maps tangent vector v at point x to the manifold
    /// Uses approximation for numerical stability
    pub fn exp_map(&self, base: &[f32], tangent: &[f32]) -> Vec<f32> {
        assert_eq!(
            base.len(),
            tangent.len(),
            "Vectors must have same dimension"
        );

        let tangent_norm = self.norm(tangent);
        if tangent_norm < EPSILON {
            return base.to_vec();
        }

        let k = self.curvature.abs().sqrt();
        let lambda_base = 2.0 / (1.0 - self.norm_squared(base) + EPSILON);

        let coeff = (k * lambda_base * tangent_norm / 2.0).tanh() / (k * tangent_norm + EPSILON);

        let scaled_tangent: Vec<f32> = tangent.iter().map(|&v| v * coeff).collect();

        self.mobius_add(base, &scaled_tangent)
    }

    /// Logarithmic map: log_x(y) maps point y to tangent space at point x
    pub fn log_map(&self, base: &[f32], target: &[f32]) -> Vec<f32> {
        assert_eq!(base.len(), target.len(), "Vectors must have same dimension");

        // Compute -x ⊕ y
        let neg_base: Vec<f32> = base.iter().map(|&v| -v).collect();
        let diff = self.mobius_add(&neg_base, target);

        let diff_norm = self.norm(&diff);
        if diff_norm < EPSILON {
            return vec![0.0; base.len()];
        }

        let k = self.curvature.abs().sqrt();
        let lambda_base = 2.0 / (1.0 - self.norm_squared(base) + EPSILON);

        let coeff =
            2.0 / (k * lambda_base + EPSILON) * (k * diff_norm).atanh() / (diff_norm + EPSILON);

        diff.iter().map(|&v| v * coeff).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f32 = 1e-4;

    #[test]
    fn test_poincare_ball_creation() {
        let ball = PoincareBall::new(-1.0);
        assert_eq!(ball.curvature, -1.0);
    }

    #[test]
    #[should_panic(expected = "Curvature must be negative")]
    fn test_poincare_positive_curvature_panics() {
        let _ball = PoincareBall::new(1.0);
    }

    #[test]
    fn test_project_within_ball() {
        let ball = PoincareBall::new(-1.0);
        let x = vec![0.5, 0.5];
        let projected = ball.project(&x);
        assert_eq!(projected, x);
    }

    #[test]
    fn test_project_outside_ball() {
        let ball = PoincareBall::new(-1.0);
        let x = vec![1.5, 1.5]; // Norm > 1
        let projected = ball.project(&x);
        let norm = ball.norm(&projected);
        assert!(norm <= MAX_NORM);
    }

    #[test]
    fn test_distance_origin() {
        let ball = PoincareBall::new(-1.0);
        let origin = vec![0.0, 0.0];
        let point = vec![0.5, 0.0];
        let dist = ball.distance(&origin, &point);
        assert!(dist > 0.0);
        assert!(dist < f32::INFINITY);
    }

    #[test]
    fn test_distance_symmetric() {
        let ball = PoincareBall::new(-1.0);
        let x = vec![0.3, 0.4];
        let y = vec![0.1, 0.2];
        let d1 = ball.distance(&x, &y);
        let d2 = ball.distance(&y, &x);
        assert!((d1 - d2).abs() < TOL);
    }

    #[test]
    fn test_distance_same_point() {
        let ball = PoincareBall::new(-1.0);
        let x = vec![0.3, 0.4];
        let dist = ball.distance(&x, &x);
        assert!(dist < TOL);
    }

    #[test]
    fn test_mobius_add_identity() {
        let ball = PoincareBall::new(-1.0);
        let x = vec![0.3, 0.4];
        let origin = vec![0.0, 0.0];
        let result = ball.mobius_add(&x, &origin);
        for i in 0..x.len() {
            assert!((result[i] - x[i]).abs() < TOL);
        }
    }

    #[test]
    fn test_exp_map_zero_tangent() {
        let ball = PoincareBall::new(-1.0);
        let base = vec![0.3, 0.4];
        let tangent = vec![0.0, 0.0];
        let result = ball.exp_map(&base, &tangent);
        assert_eq!(result, base);
    }

    #[test]
    fn test_log_exp_inverse() {
        let ball = PoincareBall::new(-1.0);
        let base = vec![0.2, 0.3];
        let tangent = vec![0.1, 0.1];

        let point = ball.exp_map(&base, &tangent);
        let recovered = ball.log_map(&base, &point);

        for i in 0..tangent.len() {
            assert!((recovered[i] - tangent[i]).abs() < TOL);
        }
    }

    #[test]
    fn test_log_map_same_point() {
        let ball = PoincareBall::new(-1.0);
        let base = vec![0.3, 0.4];
        let result = ball.log_map(&base, &base);
        for &v in &result {
            assert!(v.abs() < TOL);
        }
    }

    #[test]
    fn test_curvature_scaling() {
        let ball1 = PoincareBall::new(-1.0);
        let ball2 = PoincareBall::new(-4.0);
        let x = vec![0.3, 0.4];
        let y = vec![0.1, 0.2];

        let d1 = ball1.distance(&x, &y);
        let d2 = ball2.distance(&x, &y);

        // Higher curvature magnitude should give shorter distances
        assert!(d2 < d1);
    }
}
