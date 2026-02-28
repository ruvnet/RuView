//! Product manifold implementation

use super::config::ProductManifoldConfig;
use crate::error::{MathError, Result};
use crate::spherical::SphericalSpace;
use crate::utils::{dot, norm, EPS};

/// Product manifold: M = E^e × H^h × S^s
#[derive(Debug, Clone)]
pub struct ProductManifold {
    config: ProductManifoldConfig,
    spherical: Option<SphericalSpace>,
}

impl ProductManifold {
    /// Create a new product manifold
    ///
    /// # Arguments
    /// * `euclidean_dim` - Dimension of Euclidean component
    /// * `hyperbolic_dim` - Dimension of hyperbolic component (Poincaré ball)
    /// * `spherical_dim` - Dimension of spherical component
    pub fn new(euclidean_dim: usize, hyperbolic_dim: usize, spherical_dim: usize) -> Self {
        let config = ProductManifoldConfig::new(euclidean_dim, hyperbolic_dim, spherical_dim);
        let spherical = if spherical_dim > 0 {
            Some(SphericalSpace::new(spherical_dim))
        } else {
            None
        };

        Self { config, spherical }
    }

    /// Create from configuration
    pub fn from_config(config: ProductManifoldConfig) -> Self {
        let spherical = if config.spherical_dim > 0 {
            Some(SphericalSpace::new(config.spherical_dim))
        } else {
            None
        };

        Self { config, spherical }
    }

    /// Get configuration
    pub fn config(&self) -> &ProductManifoldConfig {
        &self.config
    }

    /// Total dimension
    pub fn dim(&self) -> usize {
        self.config.total_dim()
    }

    /// Extract Euclidean component from point
    pub fn euclidean_component<'a>(&self, point: &'a [f64]) -> &'a [f64] {
        let (e_range, _, _) = self.config.component_ranges();
        &point[e_range]
    }

    /// Extract hyperbolic component from point
    pub fn hyperbolic_component<'a>(&self, point: &'a [f64]) -> &'a [f64] {
        let (_, h_range, _) = self.config.component_ranges();
        &point[h_range]
    }

    /// Extract spherical component from point
    pub fn spherical_component<'a>(&self, point: &'a [f64]) -> &'a [f64] {
        let (_, _, s_range) = self.config.component_ranges();
        &point[s_range]
    }

    /// Project point onto the product manifold
    ///
    /// - Euclidean: no projection needed
    /// - Hyperbolic: project into Poincaré ball
    /// - Spherical: normalize to unit sphere
    pub fn project(&self, point: &[f64]) -> Result<Vec<f64>> {
        if point.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), point.len()));
        }

        let mut result = point.to_vec();
        let (_e_range, h_range, s_range) = self.config.component_ranges();

        // Euclidean: no projection needed (kept as-is)
        // Hyperbolic: project to Poincaré ball (||x|| < 1)
        if !h_range.is_empty() {
            let h_part = &mut result[h_range.clone()];
            let h_norm: f64 = h_part.iter().map(|&x| x * x).sum::<f64>().sqrt();

            if h_norm >= 1.0 - EPS {
                let scale = (1.0 - EPS) / h_norm;
                for x in h_part.iter_mut() {
                    *x *= scale;
                }
            }
        }

        // Spherical: normalize to unit sphere
        if !s_range.is_empty() {
            let s_part = &mut result[s_range.clone()];
            let s_norm: f64 = s_part.iter().map(|&x| x * x).sum::<f64>().sqrt();

            if s_norm > EPS {
                for x in s_part.iter_mut() {
                    *x /= s_norm;
                }
            } else {
                // Set to north pole
                s_part[0] = 1.0;
                for x in s_part[1..].iter_mut() {
                    *x = 0.0;
                }
            }
        }

        Ok(result)
    }

    /// Compute distance in product manifold
    ///
    /// d(x, y)² = w_e d_E(x_e, y_e)² + w_h d_H(x_h, y_h)² + w_s d_S(x_s, y_s)²
    #[inline]
    pub fn distance(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        if x.len() != self.dim() || y.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), x.len()));
        }

        let (w_e, w_h, w_s) = self.config.component_weights;
        let (e_range, h_range, s_range) = self.config.component_ranges();

        let mut dist_sq = 0.0;

        // Euclidean distance with SIMD-friendly accumulation
        if !e_range.is_empty() && w_e > 0.0 {
            let d_e = self.euclidean_distance_sq(&x[e_range.clone()], &y[e_range.clone()]);
            dist_sq += w_e * d_e;
        }

        // Hyperbolic (Poincaré) distance
        if !h_range.is_empty() && w_h > 0.0 {
            let x_h = &x[h_range.clone()];
            let y_h = &y[h_range.clone()];
            let d_h = self.poincare_distance(x_h, y_h)?;
            dist_sq += w_h * d_h * d_h;
        }

        // Spherical distance
        if !s_range.is_empty() && w_s > 0.0 {
            let x_s = &x[s_range.clone()];
            let y_s = &y[s_range.clone()];
            let d_s = self.spherical_distance(x_s, y_s)?;
            dist_sq += w_s * d_s * d_s;
        }

        Ok(dist_sq.sqrt())
    }

    /// SIMD-friendly squared Euclidean distance using 4-way unrolled accumulator
    #[inline(always)]
    fn euclidean_distance_sq(&self, x: &[f64], y: &[f64]) -> f64 {
        let len = x.len();
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f64;
        let mut sum1 = 0.0f64;
        let mut sum2 = 0.0f64;
        let mut sum3 = 0.0f64;

        // Process 4 elements at a time for SIMD vectorization
        for i in 0..chunks {
            let base = i * 4;
            let d0 = x[base] - y[base];
            let d1 = x[base + 1] - y[base + 1];
            let d2 = x[base + 2] - y[base + 2];
            let d3 = x[base + 3] - y[base + 3];
            sum0 += d0 * d0;
            sum1 += d1 * d1;
            sum2 += d2 * d2;
            sum3 += d3 * d3;
        }

        // Handle remainder
        let base = chunks * 4;
        for i in 0..remainder {
            let d = x[base + i] - y[base + i];
            sum0 += d * d;
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Poincaré ball distance
    ///
    /// d(x, y) = arcosh(1 + 2 ||x - y||² / ((1 - ||x||²)(1 - ||y||²)))
    ///
    /// Optimized with SIMD-friendly 4-way accumulator for computing norms
    #[inline]
    fn poincare_distance(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        let len = x.len();
        let chunks = len / 4;
        let remainder = len % 4;

        // Compute all three values in one pass for better cache utilization
        let mut x_norm_sq = 0.0f64;
        let mut y_norm_sq = 0.0f64;
        let mut diff_sq = 0.0f64;

        // 4-way unrolled for SIMD
        for i in 0..chunks {
            let base = i * 4;

            let x0 = x[base];
            let x1 = x[base + 1];
            let x2 = x[base + 2];
            let x3 = x[base + 3];

            let y0 = y[base];
            let y1 = y[base + 1];
            let y2 = y[base + 2];
            let y3 = y[base + 3];

            x_norm_sq += x0 * x0 + x1 * x1 + x2 * x2 + x3 * x3;
            y_norm_sq += y0 * y0 + y1 * y1 + y2 * y2 + y3 * y3;

            let d0 = x0 - y0;
            let d1 = x1 - y1;
            let d2 = x2 - y2;
            let d3 = x3 - y3;
            diff_sq += d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3;
        }

        // Handle remainder
        let base = chunks * 4;
        for i in 0..remainder {
            let xi = x[base + i];
            let yi = y[base + i];
            x_norm_sq += xi * xi;
            y_norm_sq += yi * yi;
            let d = xi - yi;
            diff_sq += d * d;
        }

        let denom = (1.0 - x_norm_sq).max(EPS) * (1.0 - y_norm_sq).max(EPS);
        let arg = 1.0 + 2.0 * diff_sq / denom;

        // Apply curvature scaling
        let c = (-self.config.hyperbolic_curvature).sqrt();
        Ok(arg.max(1.0).acosh() / c)
    }

    /// Spherical distance (geodesic)
    fn spherical_distance(&self, x: &[f64], y: &[f64]) -> Result<f64> {
        let cos_angle = dot(x, y).clamp(-1.0, 1.0);
        let c = self.config.spherical_curvature.sqrt();
        Ok(cos_angle.acos() / c)
    }

    /// Exponential map at point x with tangent vector v
    pub fn exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim() || v.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), x.len()));
        }

        let mut result = vec![0.0; self.dim()];
        let (e_range, h_range, s_range) = self.config.component_ranges();

        // Euclidean: exp_x(v) = x + v
        for i in e_range.clone() {
            result[i] = x[i] + v[i];
        }

        // Hyperbolic (Poincaré) exp map
        if !h_range.is_empty() {
            let x_h = &x[h_range.clone()];
            let v_h = &v[h_range.clone()];
            let exp_h = self.poincare_exp_map(x_h, v_h)?;
            for (i, val) in h_range.clone().zip(exp_h.iter()) {
                result[i] = *val;
            }
        }

        // Spherical exp map
        if !s_range.is_empty() {
            let x_s = &x[s_range.clone()];
            let v_s = &v[s_range.clone()];
            let exp_s = self.spherical_exp_map(x_s, v_s)?;
            for (i, val) in s_range.clone().zip(exp_s.iter()) {
                result[i] = *val;
            }
        }

        self.project(&result)
    }

    /// Poincaré ball exponential map
    fn poincare_exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>> {
        let c = -self.config.hyperbolic_curvature;
        let sqrt_c = c.sqrt();

        let x_norm_sq: f64 = x.iter().map(|&xi| xi * xi).sum();
        let v_norm: f64 = v.iter().map(|&vi| vi * vi).sum::<f64>().sqrt();

        if v_norm < EPS {
            return Ok(x.to_vec());
        }

        let lambda_x = 2.0 / (1.0 - c * x_norm_sq).max(EPS);
        let norm_v = lambda_x * v_norm;

        let t = (sqrt_c * norm_v).tanh() / (sqrt_c * v_norm);

        // Möbius addition: x ⊕_c (t * v)
        let tv: Vec<f64> = v.iter().map(|&vi| t * vi).collect();
        self.mobius_add(x, &tv, c)
    }

    /// Möbius addition in Poincaré ball
    fn mobius_add(&self, x: &[f64], y: &[f64], c: f64) -> Result<Vec<f64>> {
        let x_norm_sq: f64 = x.iter().map(|&xi| xi * xi).sum();
        let y_norm_sq: f64 = y.iter().map(|&yi| yi * yi).sum();
        let xy_dot: f64 = x.iter().zip(y.iter()).map(|(&xi, &yi)| xi * yi).sum();

        let num_coef = 1.0 + 2.0 * c * xy_dot + c * y_norm_sq;
        let denom = 1.0 + 2.0 * c * xy_dot + c * c * x_norm_sq * y_norm_sq;

        if denom.abs() < EPS {
            return Ok(x.to_vec());
        }

        let y_coef = 1.0 - c * x_norm_sq;

        let result: Vec<f64> = x
            .iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| (num_coef * xi + y_coef * yi) / denom)
            .collect();

        Ok(result)
    }

    /// Spherical exponential map
    fn spherical_exp_map(&self, x: &[f64], v: &[f64]) -> Result<Vec<f64>> {
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

        // Normalize to sphere
        let n = norm(&result);
        if n > EPS {
            Ok(result.iter().map(|&r| r / n).collect())
        } else {
            Ok(x.to_vec())
        }
    }

    /// Logarithmic map at point x toward point y
    pub fn log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>> {
        if x.len() != self.dim() || y.len() != self.dim() {
            return Err(MathError::dimension_mismatch(self.dim(), x.len()));
        }

        let mut result = vec![0.0; self.dim()];
        let (e_range, h_range, s_range) = self.config.component_ranges();

        // Euclidean: log_x(y) = y - x
        for i in e_range.clone() {
            result[i] = y[i] - x[i];
        }

        // Hyperbolic log map
        if !h_range.is_empty() {
            let x_h = &x[h_range.clone()];
            let y_h = &y[h_range.clone()];
            let log_h = self.poincare_log_map(x_h, y_h)?;
            for (i, val) in h_range.clone().zip(log_h.iter()) {
                result[i] = *val;
            }
        }

        // Spherical log map
        if !s_range.is_empty() {
            let x_s = &x[s_range.clone()];
            let y_s = &y[s_range.clone()];
            let log_s = self.spherical_log_map(x_s, y_s)?;
            for (i, val) in s_range.clone().zip(log_s.iter()) {
                result[i] = *val;
            }
        }

        Ok(result)
    }

    /// Poincaré ball logarithmic map
    fn poincare_log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>> {
        let c = -self.config.hyperbolic_curvature;

        // -x ⊕_c y
        let neg_x: Vec<f64> = x.iter().map(|&xi| -xi).collect();
        let diff = self.mobius_add(&neg_x, y, c)?;

        let diff_norm: f64 = diff.iter().map(|&d| d * d).sum::<f64>().sqrt();

        if diff_norm < EPS {
            return Ok(vec![0.0; x.len()]);
        }

        let x_norm_sq: f64 = x.iter().map(|&xi| xi * xi).sum();
        let lambda_x = 2.0 / (1.0 - c * x_norm_sq).max(EPS);

        let sqrt_c = c.sqrt();
        let arctanh_arg = (sqrt_c * diff_norm).min(1.0 - EPS);
        let scale = (2.0 / (lambda_x * sqrt_c)) * arctanh_arg.atanh() / diff_norm;

        Ok(diff.iter().map(|&d| scale * d).collect())
    }

    /// Spherical logarithmic map
    fn spherical_log_map(&self, x: &[f64], y: &[f64]) -> Result<Vec<f64>> {
        let cos_theta = dot(x, y).clamp(-1.0, 1.0);
        let theta = cos_theta.acos();

        if theta < EPS {
            return Ok(vec![0.0; x.len()]);
        }

        if (theta - std::f64::consts::PI).abs() < EPS {
            return Err(MathError::numerical_instability("Antipodal points"));
        }

        let scale = theta / theta.sin();

        Ok(x.iter()
            .zip(y.iter())
            .map(|(&xi, &yi)| scale * (yi - cos_theta * xi))
            .collect())
    }

    /// Compute Fréchet mean on product manifold
    pub fn frechet_mean(&self, points: &[Vec<f64>], weights: Option<&[f64]>) -> Result<Vec<f64>> {
        if points.is_empty() {
            return Err(MathError::empty_input("points"));
        }

        let n = points.len();
        let uniform = 1.0 / n as f64;
        let weights: Vec<f64> = match weights {
            Some(w) => {
                let sum: f64 = w.iter().sum();
                w.iter().map(|&wi| wi / sum).collect()
            }
            None => vec![uniform; n],
        };

        // Initialize with weighted Euclidean mean
        let mut mean = vec![0.0; self.dim()];
        for (p, &w) in points.iter().zip(weights.iter()) {
            for (mi, &pi) in mean.iter_mut().zip(p.iter()) {
                *mi += w * pi;
            }
        }
        mean = self.project(&mean)?;

        // Iterative refinement
        for _ in 0..100 {
            let mut gradient = vec![0.0; self.dim()];

            for (p, &w) in points.iter().zip(weights.iter()) {
                if let Ok(log_v) = self.log_map(&mean, p) {
                    for (gi, &li) in gradient.iter_mut().zip(log_v.iter()) {
                        *gi += w * li;
                    }
                }
            }

            let grad_norm = norm(&gradient);
            if grad_norm < 1e-8 {
                break;
            }

            // Step along geodesic (learning rate = 1.0)
            mean = self.exp_map(&mean, &gradient)?;
        }

        Ok(mean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_product_manifold_creation() {
        let manifold = ProductManifold::new(32, 16, 8);

        assert_eq!(manifold.dim(), 56);
        assert_eq!(manifold.config.euclidean_dim, 32);
        assert_eq!(manifold.config.hyperbolic_dim, 16);
        assert_eq!(manifold.config.spherical_dim, 8);
    }

    #[test]
    fn test_projection() {
        let manifold = ProductManifold::new(2, 2, 3);

        // Point with hyperbolic component outside ball and unnormalized spherical
        let point = vec![1.0, 2.0, 2.0, 0.0, 3.0, 4.0, 0.0];

        let projected = manifold.project(&point).unwrap();

        // Check hyperbolic is in ball
        let h = manifold.hyperbolic_component(&projected);
        let h_norm: f64 = h.iter().map(|&x| x * x).sum::<f64>().sqrt();
        assert!(h_norm < 1.0);

        // Check spherical is normalized
        let s = manifold.spherical_component(&projected);
        let s_norm: f64 = s.iter().map(|&x| x * x).sum::<f64>().sqrt();
        assert!((s_norm - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_only_distance() {
        let manifold = ProductManifold::new(3, 0, 0);

        let x = vec![0.0, 0.0, 0.0];
        let y = vec![3.0, 4.0, 0.0];

        let dist = manifold.distance(&x, &y).unwrap();
        assert!((dist - 5.0).abs() < 1e-10);
    }

    #[test]
    fn test_product_distance() {
        let manifold = ProductManifold::new(2, 2, 3);

        let x = manifold
            .project(&vec![0.0, 0.0, 0.1, 0.0, 1.0, 0.0, 0.0])
            .unwrap();
        let y = manifold
            .project(&vec![1.0, 1.0, 0.0, 0.1, 0.0, 1.0, 0.0])
            .unwrap();

        let dist = manifold.distance(&x, &y).unwrap();
        assert!(dist > 0.0);
    }

    #[test]
    fn test_exp_log_inverse() {
        let manifold = ProductManifold::new(2, 0, 0); // Euclidean only for simplicity

        let x = vec![1.0, 2.0];
        let y = vec![3.0, 4.0];

        let v = manifold.log_map(&x, &y).unwrap();
        let y_recovered = manifold.exp_map(&x, &v).unwrap();

        for (yi, yr) in y.iter().zip(y_recovered.iter()) {
            assert!((yi - yr).abs() < 1e-6);
        }
    }
}
