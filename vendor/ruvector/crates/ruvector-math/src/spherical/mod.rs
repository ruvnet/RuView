//! Spherical Geometry
//!
//! Operations on the n-sphere S^n = {x ∈ R^{n+1} : ||x|| = 1}
//!
//! ## Use Cases in Vector Search
//!
//! - **Cyclical patterns**: Time-of-day, day-of-week, seasonal data
//! - **Directional data**: Wind directions, compass bearings
//! - **Normalized embeddings**: Common in NLP (unit-normalized word vectors)
//! - **Angular similarity**: Natural for cosine similarity
//!
//! ## Key Operations
//!
//! - Geodesic distance: d(x, y) = arccos(⟨x, y⟩)
//! - Exponential map: Move from x in direction v
//! - Logarithmic map: Find direction from x to y
//! - Fréchet mean: Spherical centroid

use crate::error::{MathError, Result};
use crate::utils::{dot, norm, normalize, EPS};

/// Configuration for spherical operations
#[derive(Debug, Clone)]
pub struct SphericalConfig {
    /// Maximum iterations for iterative algorithms
    pub max_iterations: usize,
    /// Convergence threshold
    pub threshold: f64,
}

impl Default for SphericalConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            threshold: 1e-8,
        }
    }
}

/// Spherical space operations
#[derive(Debug, Clone)]
pub struct SphericalSpace {
    /// Dimension of the sphere (ambient dimension - 1)
    dim: usize,
    /// Configuration
    config: SphericalConfig,
}

impl SphericalSpace {
    /// Create a new spherical space S^{n-1} embedded in R^n
    ///
    /// # Arguments
    /// * `ambient_dim` - Dimension of ambient Euclidean space
    pub fn new(ambient_dim: usize) -> Self {
        Self {
            dim: ambient_dim.max(1),
            config: SphericalConfig::default(),
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: SphericalConfig) -> Self {
        self.config = config;
        self
    }

    /// Get ambient dimension
    pub fn ambient_dim(&self) -> usize {
        self.dim
    }

    /// Get intrinsic dimension (ambient_dim - 1)
    pub fn intrinsic_dim(&self) -> usize {
        self.dim.saturating_sub(1)
    }

    /// Project a point onto the sphere
    pub fn project(&self, point: &[f64]) -> Result<Vec<f64>> {
        if point.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, point.len()));
        }

        let n = norm(point);
        if n < EPS {
            // Return north pole for zero vector
            let mut result = vec![0.0; self.dim];
            result[0] = 1.0;
            return Ok(result);
        }

        Ok(normalize(point))
    }

    /// Check if point is on the sphere
    pub fn is_on_sphere(&self, point: &[f64]) -> bool {
        if point.len() != self.dim {
            return false;
        }
        let n = norm(point);
        (n - 1.0).abs() < 1e-6
    }

    /// Geodesic distance on the sphere: d(x, y) = arccos(⟨x, y⟩)
    ///
    /// This is the great-circle distance.
    pub fn distance(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        if x.len() != self.dim || y.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, x.len()));
        }

        let cos_angle = dot(x, y).clamp(-1.0, 1.0);
        Ok(cos_angle.acos())
    }

    /// Squared geodesic distance (useful for optimization)
    pub fn squared_distance(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        let d = self.distance(x, y)?;
        Ok(d * d)
    }

    /// Exponential map: exp_x(v) - move from x in direction v
    ///
    /// exp_x(v) = cos(||v||) x + sin(||v||) (v / ||v||)
    pub fn exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim || v.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, x.len()));
        }

        let v_norm = norm(v);

        if v_norm < EPS {
            return Ok(x.to_vec());
        }

        let cos_t = v_norm.cos();
        let sin_t = v_norm.sin();

        let result: Vec<f64> = x
            .iter()
            .zip(v.iter())
            .map(|(&xi, &vi)| cos_t * xi + sin_t * vi / v_norm)
            .collect();

        // Ensure on sphere
        Ok(normalize(&result))
    }

    /// Logarithmic map: log_x(y) - tangent vector at x pointing toward y
    ///
    /// log_x(y) = (θ / sin(θ)) (y - cos(θ) x)
    /// where θ = d(x, y) = arccos(⟨x, y⟩)
    pub fn log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim || y.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, x.len()));
        }

        let cos_theta = dot(x, y).clamp(-1.0, 1.0);
        let theta = cos_theta.acos();

        if theta < EPS {
            // Points are the same
            return Ok(vec![0.0; self.dim]);
        }

        if (theta - std::f64::consts::PI).abs() < EPS {
            // Points are antipodal - log map is not well-defined
            return Err(MathError::numerical_instability(
                "Antipodal points have undefined log map",
            ));
        }

        let scale = theta / theta.sin();

        let result: Vec<f64> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| scale * (yi - cos_theta * xi))
            .collect();

        Ok(result)
    }

    /// Parallel transport vector v from x to y
    ///
    /// Transports tangent vector at x along geodesic to y
    pub fn parallel_transport(&self, x: &[f64], y: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim || y.len() != self.dim || v.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, x.len()));
        }

        let cos_theta = dot(x, y).clamp(-1.0, 1.0);

        if (cos_theta - 1.0).abs() < EPS {
            // Same point, no transport needed
            return Ok(v.to_vec());
        }

        let theta = cos_theta.acos();

        // Direction from x to y (unit tangent)
        let u: Vec<f64> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| yi - cos_theta * xi)
            .collect();
        let u = normalize(&u);

        // Component of v along u
        let v_u = dot(v, &u);

        // Transport formula
        let result: Vec<f64> = (0..self.dim)
            .map(|i| {
                let v_perp = v[i] - v_u * u[i] - dot(v, x) * x[i];
                v_perp + v_u * (-theta.sin() * x[i] + theta.cos() * u[i])
                    - dot(v, x) * (theta.cos() * x[i] + theta.sin() * u[i])
            })
            .collect();

        Ok(result)
    }

    /// Fréchet mean on the sphere (spherical centroid)
    ///
    /// Minimizes: Σᵢ wᵢ d(m, xᵢ)²
    pub fn frechet_mean(&self, points: &[Vec<f64>], weights: Option<&[f64]>) -> Result<Vec<f64>> {
        if points.is_empty() {
            return Err(MathError::empty_input("points"));
        }

        let n = points.len();
        let uniform_weight = 1.0 / n as f64;
        let weights: Vec<f64> = match weights {
            Some(w) => {
                let sum: f64 = w.iter().sum();
                w.iter().map(|&wi| wi / sum).collect()
            }
            None => vec![uniform_weight; n],
        };

        // Initialize with weighted Euclidean mean, then project
        let mut mean: Vec<f64> = vec![0.0; self.dim];
        for (p, &w) in points.iter().zip(weights.iter()) {
            for (mi, &pi) in mean.iter_mut().zip(p.iter()) {
                *mi += w * pi;
            }
        }
        mean = self.project(&mean)?;

        // Iterative refinement (Riemannian gradient descent)
        for _ in 0..self.config.max_iterations {
            // Compute Riemannian gradient: Σ wᵢ log_{mean}(xᵢ)
            let mut gradient = vec![0.0; self.dim];

            for (p, &w) in points.iter().zip(weights.iter()) {
                if let Ok(log_v) = self.log_map(&mean, p) {
                    for (gi, &li) in gradient.iter_mut().zip(log_v.iter()) {
                        *gi += w * li;
                    }
                }
            }

            let grad_norm = norm(&gradient);
            if grad_norm < self.config.threshold {
                break;
            }

            // Step along geodesic
            mean = self.exp_map(&mean, &gradient)?;
        }

        Ok(mean)
    }

    /// Geodesic interpolation: point at fraction t along geodesic from x to y
    ///
    /// γ(t) = sin((1-t)θ)/sin(θ) x + sin(tθ)/sin(θ) y
    pub fn geodesic(&self, x: &[f64], y: &[f64], t: f64) -> Result<Vec<f64>> {
        if x.len() != self.dim || y.len() != self.dim {
            return Err(MathError::dimension_mismatch(self.dim, x.len()));
        }

        let t = t.clamp(0.0, 1.0);

        let cos_theta = dot(x, y).clamp(-1.0, 1.0);
        let theta = cos_theta.acos();

        if theta < EPS {
            return Ok(x.to_vec());
        }

        let sin_theta = theta.sin();
        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;

        let result: Vec<f64> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| a * xi + b * yi)
            .collect();

        // Ensure on sphere
        Ok(normalize(&result))
    }

    /// Sample uniformly from the sphere
    pub fn sample_uniform(&self, rng: &mut impl rand::Rng) -> Vec<f64> {
        use rand_distr::{Distribution, StandardNormal};

        let point: Vec<f64> = (0..self.dim).map(|_| StandardNormal.sample(rng)).collect();

        normalize(&point)
    }

    /// Von Mises-Fisher mean direction MLE
    ///
    /// Computes the mean direction (mode of vMF distribution)
    pub fn mean_direction(&self, points: &[Vec<f64>]) -> Result<Vec<f64>> {
        if points.is_empty() {
            return Err(MathError::empty_input("points"));
        }

        let mut sum = vec![0.0; self.dim];
        for p in points {
            if p.len() != self.dim {
                return Err(MathError::dimension_mismatch(self.dim, p.len()));
            }
            for (si, &pi) in sum.iter_mut().zip(p.iter()) {
                *si += pi;
            }
        }

        Ok(normalize(&sum))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_onto_sphere() {
        let sphere = SphericalSpace::new(3);

        let point = vec![3.0, 4.0, 0.0];
        let projected = sphere.project(&point).unwrap();

        let norm: f64 = projected.iter().map(|&x| x * x).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_geodesic_distance() {
        let sphere = SphericalSpace::new(3);

        // Orthogonal unit vectors
        let x = vec![1.0, 0.0, 0.0];
        let y = vec![0.0, 1.0, 0.0];

        let dist = sphere.distance(&x, &y).unwrap();
        let expected = std::f64::consts::PI / 2.0;

        assert!((dist - expected).abs() < 1e-10);
    }

    #[test]
    fn test_exp_log_inverse() {
        let sphere = SphericalSpace::new(3);

        let x = vec![1.0, 0.0, 0.0];
        let y = sphere.project(&vec![1.0, 1.0, 0.0]).unwrap();

        // log then exp should return to y
        let v = sphere.log_map(&x, &y).unwrap();
        let y_recovered = sphere.exp_map(&x, &v).unwrap();

        for (yi, &yr) in y.iter().zip(y_recovered.iter()) {
            assert!((yi - yr).abs() < 1e-6, "Exp-log inverse failed");
        }
    }

    #[test]
    fn test_geodesic_interpolation() {
        let sphere = SphericalSpace::new(3);

        let x = vec![1.0, 0.0, 0.0];
        let y = vec![0.0, 1.0, 0.0];

        // Midpoint
        let mid = sphere.geodesic(&x, &y, 0.5).unwrap();

        // Should be on sphere
        let norm: f64 = mid.iter().map(|&m| m * m).sum::<f64>().sqrt();
        assert!((norm - 1.0).abs() < 1e-10);

        // Should be equidistant
        let d_x = sphere.distance(&x, &mid).unwrap();
        let d_y = sphere.distance(&mid, &y).unwrap();
        assert!((d_x - d_y).abs() < 1e-10);
    }

    #[test]
    fn test_frechet_mean() {
        let sphere = SphericalSpace::new(3);

        // Points near north pole
        let points = vec![
            vec![0.9, 0.1, 0.0],
            vec![0.9, -0.1, 0.0],
            vec![0.9, 0.0, 0.1],
            vec![0.9, 0.0, -0.1],
        ];

        let points: Vec<Vec<f64>> = points
            .into_iter()
            .map(|p| sphere.project(&p).unwrap())
            .collect();

        let mean = sphere.frechet_mean(&points, None).unwrap();

        // Mean should be close to (1, 0, 0)
        assert!(mean[0] > 0.95);
    }
}
