// Lorentz Hyperboloid Model Implementation
// Implements isometric model of hyperbolic space

use crate::hyperbolic::EPSILON;
use simsimd::SpatialSimilarity;

/// Lorentz/Hyperboloid model for hyperbolic space
/// Points live on the hyperboloid: -x₀² + x₁² + ... + xₙ² = -1/K
pub struct LorentzModel {
    /// Curvature of the hyperbolic space (typically -1.0)
    pub curvature: f32,
}

impl LorentzModel {
    /// Create a new Lorentz model with specified curvature
    pub fn new(curvature: f32) -> Self {
        assert!(curvature < 0.0, "Curvature must be negative");
        Self { curvature }
    }

    /// Minkowski inner product: -x₀y₀ + x₁y₁ + ... + xₙyₙ
    pub fn minkowski_dot(&self, x: &[f32], y: &[f32]) -> f32 {
        assert_eq!(x.len(), y.len(), "Vectors must have same dimension");
        assert!(x.len() >= 2, "Need at least 2 dimensions for Lorentz model");

        let time_part = -x[0] * y[0];
        let spatial_part = if x.len() > 1 {
            f32::dot(&x[1..], &y[1..]).unwrap_or(0.0) as f32
        } else {
            0.0f32
        };

        time_part + spatial_part
    }

    /// Compute Lorentz distance between two points
    /// d(x, y) = acosh(-⟨x, y⟩_L)
    pub fn distance(&self, x: &[f32], y: &[f32]) -> f32 {
        let inner = -self.minkowski_dot(x, y);

        // Clamp to avoid numerical errors in acosh
        let arg = inner.max(1.0);
        let distance = arg.acosh();

        // Scale by curvature
        let k = self.curvature.abs().sqrt();
        distance / k
    }

    /// Convert from Poincaré ball coordinates to Lorentz hyperboloid
    /// x → (1 + ||x||², 2x₁, 2x₂, ..., 2xₙ) / (1 - ||x||²)
    pub fn from_poincare(&self, x: &[f32]) -> Vec<f32> {
        let norm_sq = f32::dot(x, x).unwrap_or(0.0) as f32;
        let norm_sq = norm_sq.max(0.0);
        let denominator = 1.0f32 - norm_sq + EPSILON;

        if denominator <= EPSILON {
            // Point at infinity, return large time coordinate
            let mut result = vec![0.0f32; x.len() + 1];
            result[0] = 1e6f32; // Large time coordinate
            return result;
        }

        let time_coord = (1.0f32 + norm_sq) / denominator;
        let spatial_scale = 2.0f32 / denominator;

        let mut result: Vec<f32> = Vec::with_capacity(x.len() + 1);
        result.push(time_coord);
        for &xi in x {
            result.push(xi * spatial_scale);
        }

        result
    }

    /// Convert from Lorentz hyperboloid to Poincaré ball coordinates
    /// (x₀, x₁, ..., xₙ) → (x₁, ..., xₙ) / (x₀ + 1)
    pub fn to_poincare(&self, x: &[f32]) -> Vec<f32> {
        assert!(x.len() >= 2, "Need at least 2 dimensions for Lorentz model");

        let time_coord = x[0];
        let denominator = time_coord + 1.0 + EPSILON;

        if denominator <= EPSILON {
            // Point at infinity, return origin
            return vec![0.0; x.len() - 1];
        }

        x[1..].iter().map(|&xi| xi / denominator).collect()
    }

    /// Verify that a point lies on the hyperboloid
    /// Should satisfy: -x₀² + x₁² + ... + xₙ² = -1/K
    pub fn is_on_hyperboloid(&self, x: &[f32]) -> bool {
        let k = self.curvature.abs();
        let expected = -1.0 / k;
        let actual = self.minkowski_dot(x, x);
        (actual - expected).abs() < EPSILON * 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hyperbolic::poincare::PoincareBall;

    const TOL: f32 = 1e-3;

    #[test]
    fn test_lorentz_creation() {
        let model = LorentzModel::new(-1.0);
        assert_eq!(model.curvature, -1.0);
    }

    #[test]
    #[should_panic(expected = "Curvature must be negative")]
    fn test_lorentz_positive_curvature_panics() {
        let _model = LorentzModel::new(1.0);
    }

    #[test]
    fn test_minkowski_dot() {
        let model = LorentzModel::new(-1.0);
        let x = vec![2.0, 1.0, 1.0];
        let y = vec![3.0, 2.0, 1.0];

        // -2*3 + 1*2 + 1*1 = -6 + 2 + 1 = -3
        let result = model.minkowski_dot(&x, &y);
        assert!((result - (-3.0)).abs() < TOL);
    }

    #[test]
    fn test_minkowski_dot_self() {
        let model = LorentzModel::new(-1.0);
        let x = vec![1.5, 1.0, 0.5];

        // -1.5² + 1.0² + 0.5² = -2.25 + 1.0 + 0.25 = -1.0
        let result = model.minkowski_dot(&x, &x);
        assert!((result - (-1.0)).abs() < TOL);
    }

    #[test]
    fn test_distance_same_point() {
        let model = LorentzModel::new(-1.0);
        let x = vec![1.5, 1.0, 0.5];
        let dist = model.distance(&x, &x);
        assert!(dist < TOL);
    }

    #[test]
    fn test_distance_different_points() {
        let model = LorentzModel::new(-1.0);
        let x = vec![1.5, 1.0, 0.5];
        let y = vec![2.0, 1.5, 0.5];
        let dist = model.distance(&x, &y);
        assert!(dist > 0.0);
        assert!(dist < f32::INFINITY);
    }

    #[test]
    fn test_distance_symmetric() {
        let model = LorentzModel::new(-1.0);
        let x = vec![1.5, 1.0, 0.5];
        let y = vec![2.0, 1.5, 0.5];
        let d1 = model.distance(&x, &y);
        let d2 = model.distance(&y, &x);
        assert!((d1 - d2).abs() < TOL);
    }

    #[test]
    fn test_poincare_conversion_origin() {
        let model = LorentzModel::new(-1.0);
        let poincare_origin = vec![0.0, 0.0];
        let lorentz = model.from_poincare(&poincare_origin);

        // Origin should map to (1, 0, 0)
        assert!((lorentz[0] - 1.0).abs() < TOL);
        assert!(lorentz[1].abs() < TOL);
        assert!(lorentz[2].abs() < TOL);

        assert!(model.is_on_hyperboloid(&lorentz));
    }

    #[test]
    fn test_poincare_conversion_roundtrip() {
        let model = LorentzModel::new(-1.0);
        let original = vec![0.3, 0.4];

        let lorentz = model.from_poincare(&original);
        assert!(model.is_on_hyperboloid(&lorentz));

        let recovered = model.to_poincare(&lorentz);

        for i in 0..original.len() {
            assert!((recovered[i] - original[i]).abs() < TOL);
        }
    }

    #[test]
    fn test_from_poincare_on_hyperboloid() {
        let model = LorentzModel::new(-1.0);
        let points = vec![
            vec![0.0, 0.0],
            vec![0.3, 0.4],
            vec![0.5, 0.0],
            vec![0.2, 0.7],
        ];

        for point in points {
            let lorentz = model.from_poincare(&point);
            assert!(
                model.is_on_hyperboloid(&lorentz),
                "Point {:?} -> {:?} not on hyperboloid",
                point,
                lorentz
            );
        }
    }

    #[test]
    fn test_distance_consistency_with_poincare() {
        let lorentz_model = LorentzModel::new(-1.0);
        let poincare_ball = PoincareBall::new(-1.0);

        let p1 = vec![0.2, 0.3];
        let p2 = vec![0.4, 0.1];

        let l1 = lorentz_model.from_poincare(&p1);
        let l2 = lorentz_model.from_poincare(&p2);

        let lorentz_dist = lorentz_model.distance(&l1, &l2);
        let poincare_dist = poincare_ball.distance(&p1, &p2);

        // Distances should be approximately equal
        assert!(
            (lorentz_dist - poincare_dist).abs() < TOL,
            "Lorentz: {}, Poincaré: {}",
            lorentz_dist,
            poincare_dist
        );
    }

    #[test]
    fn test_curvature_scaling() {
        let model1 = LorentzModel::new(-1.0);
        let model2 = LorentzModel::new(-4.0);

        let x = vec![1.5, 1.0, 0.5];
        let y = vec![2.0, 1.5, 0.5];

        let d1 = model1.distance(&x, &y);
        let d2 = model2.distance(&x, &y);

        // Higher curvature magnitude should give shorter distances
        assert!(d2 < d1);
    }
}
