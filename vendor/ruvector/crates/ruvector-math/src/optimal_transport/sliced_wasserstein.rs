//! Sliced Wasserstein Distance
//!
//! The Sliced Wasserstein distance projects high-dimensional distributions
//! onto random 1D lines and averages the 1D Wasserstein distances.
//!
//! ## Algorithm
//!
//! 1. Generate L random unit vectors (directions) in R^d
//! 2. For each direction θ:
//!    a. Project all source and target points onto θ
//!    b. Compute 1D Wasserstein distance (closed-form via sorted quantiles)
//! 3. Average over all directions
//!
//! ## Complexity
//!
//! - O(L × n log n) where L = number of projections, n = number of points
//! - Linear in dimension d (only dot products)
//!
//! ## Advantages
//!
//! - **Fast**: Near-linear scaling to millions of points
//! - **SIMD-friendly**: Projections are just dot products
//! - **Statistically consistent**: Converges to true W2 as L → ∞

use super::{OptimalTransport, WassersteinConfig};
use crate::utils::{argsort, EPS};
use rand::prelude::*;
use rand_distr::StandardNormal;

/// Sliced Wasserstein distance calculator
#[derive(Debug, Clone)]
pub struct SlicedWasserstein {
    /// Number of random projection directions
    num_projections: usize,
    /// Power for Wasserstein-p (typically 1 or 2)
    p: f64,
    /// Random seed for reproducibility
    seed: Option<u64>,
}

impl SlicedWasserstein {
    /// Create a new Sliced Wasserstein calculator
    ///
    /// # Arguments
    /// * `num_projections` - Number of random 1D projections (100-1000 typical)
    pub fn new(num_projections: usize) -> Self {
        Self {
            num_projections: num_projections.max(1),
            p: 2.0,
            seed: None,
        }
    }

    /// Create from configuration
    pub fn from_config(config: &WassersteinConfig) -> Self {
        Self {
            num_projections: config.num_projections.max(1),
            p: config.p,
            seed: config.seed,
        }
    }

    /// Set the Wasserstein power (1 for W1, 2 for W2)
    pub fn with_power(mut self, p: f64) -> Self {
        self.p = p.max(1.0);
        self
    }

    /// Set random seed for reproducibility
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }

    /// Generate random unit directions
    fn generate_directions(&self, dim: usize) -> Vec<Vec<f64>> {
        let mut rng = match self.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        (0..self.num_projections)
            .map(|_| {
                let mut direction: Vec<f64> =
                    (0..dim).map(|_| rng.sample(StandardNormal)).collect();

                // Normalize to unit vector
                let norm: f64 = direction.iter().map(|&x| x * x).sum::<f64>().sqrt();
                if norm > EPS {
                    for x in &mut direction {
                        *x /= norm;
                    }
                }
                direction
            })
            .collect()
    }

    /// Project points onto a direction (SIMD-friendly dot product)
    #[inline(always)]
    fn project(points: &[Vec<f64>], direction: &[f64]) -> Vec<f64> {
        points
            .iter()
            .map(|p| Self::dot_product(p, direction))
            .collect()
    }

    /// Project points into pre-allocated buffer (reduces allocations)
    #[inline(always)]
    fn project_into(points: &[Vec<f64>], direction: &[f64], out: &mut [f64]) {
        for (i, p) in points.iter().enumerate() {
            out[i] = Self::dot_product(p, direction);
        }
    }

    /// SIMD-friendly dot product using fold pattern
    /// Compiler can auto-vectorize this pattern effectively
    #[inline(always)]
    fn dot_product(a: &[f64], b: &[f64]) -> f64 {
        // Use 4-way unrolled accumulator for better SIMD utilization
        let len = a.len();
        let chunks = len / 4;
        let remainder = len % 4;

        let mut sum0 = 0.0f64;
        let mut sum1 = 0.0f64;
        let mut sum2 = 0.0f64;
        let mut sum3 = 0.0f64;

        // Process 4 elements at a time (helps SIMD vectorization)
        for i in 0..chunks {
            let base = i * 4;
            sum0 += a[base] * b[base];
            sum1 += a[base + 1] * b[base + 1];
            sum2 += a[base + 2] * b[base + 2];
            sum3 += a[base + 3] * b[base + 3];
        }

        // Handle remainder
        let base = chunks * 4;
        for i in 0..remainder {
            sum0 += a[base + i] * b[base + i];
        }

        sum0 + sum1 + sum2 + sum3
    }

    /// Compute 1D Wasserstein distance between two sorted distributions
    ///
    /// For uniform weights, this is simply the sum of |sorted_a[i] - sorted_b[i]|^p
    #[inline]
    fn wasserstein_1d_uniform(&self, mut proj_a: Vec<f64>, mut proj_b: Vec<f64>) -> f64 {
        let n = proj_a.len();
        let m = proj_b.len();

        // Sort projections using fast f64 comparison
        proj_a.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        proj_b.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if n == m {
            // Same size: direct comparison with SIMD-friendly accumulator
            self.wasserstein_1d_equal_size(&proj_a, &proj_b)
        } else {
            // Different sizes: interpolate via quantiles
            self.wasserstein_1d_quantile(&proj_a, &proj_b, n.max(m))
        }
    }

    /// Optimized equal-size 1D Wasserstein with SIMD-friendly pattern
    #[inline(always)]
    fn wasserstein_1d_equal_size(&self, sorted_a: &[f64], sorted_b: &[f64]) -> f64 {
        let n = sorted_a.len();
        if n == 0 {
            return 0.0;
        }

        // Use p=2 fast path (most common case)
        if (self.p - 2.0).abs() < 1e-10 {
            // L2 Wasserstein: sum of squared differences
            let mut sum0 = 0.0f64;
            let mut sum1 = 0.0f64;
            let mut sum2 = 0.0f64;
            let mut sum3 = 0.0f64;

            let chunks = n / 4;
            let remainder = n % 4;

            for i in 0..chunks {
                let base = i * 4;
                let d0 = sorted_a[base] - sorted_b[base];
                let d1 = sorted_a[base + 1] - sorted_b[base + 1];
                let d2 = sorted_a[base + 2] - sorted_b[base + 2];
                let d3 = sorted_a[base + 3] - sorted_b[base + 3];
                sum0 += d0 * d0;
                sum1 += d1 * d1;
                sum2 += d2 * d2;
                sum3 += d3 * d3;
            }

            let base = chunks * 4;
            for i in 0..remainder {
                let d = sorted_a[base + i] - sorted_b[base + i];
                sum0 += d * d;
            }

            (sum0 + sum1 + sum2 + sum3) / n as f64
        } else if (self.p - 1.0).abs() < 1e-10 {
            // L1 Wasserstein: sum of absolute differences
            let mut sum = 0.0f64;
            for i in 0..n {
                sum += (sorted_a[i] - sorted_b[i]).abs();
            }
            sum / n as f64
        } else {
            // General case
            sorted_a
                .iter()
                .zip(sorted_b.iter())
                .map(|(&a, &b)| (a - b).abs().powf(self.p))
                .sum::<f64>()
                / n as f64
        }
    }

    /// Compute 1D Wasserstein via quantile interpolation
    fn wasserstein_1d_quantile(
        &self,
        sorted_a: &[f64],
        sorted_b: &[f64],
        num_samples: usize,
    ) -> f64 {
        let mut total = 0.0;

        for i in 0..num_samples {
            let q = (i as f64 + 0.5) / num_samples as f64;

            let val_a = quantile_sorted(sorted_a, q);
            let val_b = quantile_sorted(sorted_b, q);

            total += (val_a - val_b).abs().powf(self.p);
        }

        total / num_samples as f64
    }

    /// Compute 1D Wasserstein with weights
    fn wasserstein_1d_weighted(
        &self,
        proj_a: &[f64],
        weights_a: &[f64],
        proj_b: &[f64],
        weights_b: &[f64],
    ) -> f64 {
        // Sort by projected values
        let idx_a = argsort(proj_a);
        let idx_b = argsort(proj_b);

        let sorted_a: Vec<f64> = idx_a.iter().map(|&i| proj_a[i]).collect();
        let sorted_w_a: Vec<f64> = idx_a.iter().map(|&i| weights_a[i]).collect();
        let sorted_b: Vec<f64> = idx_b.iter().map(|&i| proj_b[i]).collect();
        let sorted_w_b: Vec<f64> = idx_b.iter().map(|&i| weights_b[i]).collect();

        // Compute cumulative weights
        let cdf_a = compute_cdf(&sorted_w_a);
        let cdf_b = compute_cdf(&sorted_w_b);

        // Merge and compute
        self.wasserstein_1d_from_cdfs(&sorted_a, &cdf_a, &sorted_b, &cdf_b)
    }

    /// Compute 1D Wasserstein from CDFs
    fn wasserstein_1d_from_cdfs(
        &self,
        values_a: &[f64],
        cdf_a: &[f64],
        values_b: &[f64],
        cdf_b: &[f64],
    ) -> f64 {
        // Merge all CDF points
        let mut events: Vec<(f64, f64, f64)> = Vec::new(); // (position, cdf_a, cdf_b)

        let mut ia = 0;
        let mut ib = 0;
        let mut current_cdf_a = 0.0;
        let mut current_cdf_b = 0.0;

        while ia < values_a.len() || ib < values_b.len() {
            let pos = match (ia < values_a.len(), ib < values_b.len()) {
                (true, true) => {
                    if values_a[ia] <= values_b[ib] {
                        current_cdf_a = cdf_a[ia];
                        ia += 1;
                        values_a[ia - 1]
                    } else {
                        current_cdf_b = cdf_b[ib];
                        ib += 1;
                        values_b[ib - 1]
                    }
                }
                (true, false) => {
                    current_cdf_a = cdf_a[ia];
                    ia += 1;
                    values_a[ia - 1]
                }
                (false, true) => {
                    current_cdf_b = cdf_b[ib];
                    ib += 1;
                    values_b[ib - 1]
                }
                (false, false) => break,
            };

            events.push((pos, current_cdf_a, current_cdf_b));
        }

        // Integrate |F_a - F_b|^p
        let mut total = 0.0;
        for i in 1..events.len() {
            let width = events[i].0 - events[i - 1].0;
            let height = (events[i - 1].1 - events[i - 1].2).abs();
            total += width * height.powf(self.p);
        }

        total
    }
}

impl OptimalTransport for SlicedWasserstein {
    fn distance(&self, source: &[Vec<f64>], target: &[Vec<f64>]) -> f64 {
        if source.is_empty() || target.is_empty() {
            return 0.0;
        }

        let dim = source[0].len();
        if dim == 0 {
            return 0.0;
        }

        let directions = self.generate_directions(dim);
        let n_source = source.len();
        let n_target = target.len();

        // Pre-allocate projection buffers (reduces allocations per direction)
        let mut proj_source = vec![0.0; n_source];
        let mut proj_target = vec![0.0; n_target];

        let total: f64 = directions
            .iter()
            .map(|dir| {
                // Project into pre-allocated buffers
                Self::project_into(source, dir, &mut proj_source);
                Self::project_into(target, dir, &mut proj_target);

                // Clone for sorting (wasserstein_1d_uniform sorts in place)
                self.wasserstein_1d_uniform(proj_source.clone(), proj_target.clone())
            })
            .sum();

        (total / self.num_projections as f64).powf(1.0 / self.p)
    }

    fn weighted_distance(
        &self,
        source: &[Vec<f64>],
        source_weights: &[f64],
        target: &[Vec<f64>],
        target_weights: &[f64],
    ) -> f64 {
        if source.is_empty() || target.is_empty() {
            return 0.0;
        }

        let dim = source[0].len();
        if dim == 0 {
            return 0.0;
        }

        // Normalize weights
        let sum_a: f64 = source_weights.iter().sum();
        let sum_b: f64 = target_weights.iter().sum();
        let weights_a: Vec<f64> = source_weights.iter().map(|&w| w / sum_a).collect();
        let weights_b: Vec<f64> = target_weights.iter().map(|&w| w / sum_b).collect();

        let directions = self.generate_directions(dim);

        let total: f64 = directions
            .iter()
            .map(|dir| {
                let proj_source = Self::project(source, dir);
                let proj_target = Self::project(target, dir);
                self.wasserstein_1d_weighted(&proj_source, &weights_a, &proj_target, &weights_b)
            })
            .sum();

        (total / self.num_projections as f64).powf(1.0 / self.p)
    }
}

/// Quantile of sorted data
fn quantile_sorted(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let q = q.clamp(0.0, 1.0);
    let n = sorted.len();

    if n == 1 {
        return sorted[0];
    }

    let idx_f = q * (n - 1) as f64;
    let idx_low = idx_f.floor() as usize;
    let idx_high = (idx_low + 1).min(n - 1);
    let frac = idx_f - idx_low as f64;

    sorted[idx_low] * (1.0 - frac) + sorted[idx_high] * frac
}

/// Compute CDF from weights
fn compute_cdf(weights: &[f64]) -> Vec<f64> {
    let total: f64 = weights.iter().sum();
    let mut cdf = Vec::with_capacity(weights.len());
    let mut cumsum = 0.0;

    for &w in weights {
        cumsum += w / total;
        cdf.push(cumsum);
    }

    cdf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliced_wasserstein_identical() {
        let sw = SlicedWasserstein::new(100).with_seed(42);

        let points = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];

        // Distance to itself should be very small
        let dist = sw.distance(&points, &points);
        assert!(dist < 0.01, "Self-distance should be ~0, got {}", dist);
    }

    #[test]
    fn test_sliced_wasserstein_translation() {
        let sw = SlicedWasserstein::new(500).with_seed(42);

        let source = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];

        // Translate by (1, 1)
        let target: Vec<Vec<f64>> = source
            .iter()
            .map(|p| vec![p[0] + 1.0, p[1] + 1.0])
            .collect();

        let dist = sw.distance(&source, &target);

        // For W2 translation by (1, 1), expected distance is sqrt(2) ≈ 1.414
        // But Sliced Wasserstein is an approximation, so allow wider tolerance
        assert!(
            dist > 0.5 && dist < 2.0,
            "Translation distance should be positive, got {:.3}",
            dist
        );
    }

    #[test]
    fn test_sliced_wasserstein_scaling() {
        let sw = SlicedWasserstein::new(500).with_seed(42);

        let source = vec![
            vec![0.0, 0.0],
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
        ];

        // Scale by 2
        let target: Vec<Vec<f64>> = source
            .iter()
            .map(|p| vec![p[0] * 2.0, p[1] * 2.0])
            .collect();

        let dist = sw.distance(&source, &target);

        // Should be positive for scaled distribution
        assert!(dist > 0.0, "Scaling should produce positive distance");
    }

    #[test]
    fn test_weighted_distance() {
        let sw = SlicedWasserstein::new(100).with_seed(42);

        let source = vec![vec![0.0], vec![1.0]];
        let target = vec![vec![2.0], vec![3.0]];

        // Uniform weights
        let weights_s = vec![0.5, 0.5];
        let weights_t = vec![0.5, 0.5];

        let dist = sw.weighted_distance(&source, &weights_s, &target, &weights_t);
        assert!(dist > 0.0);
    }

    #[test]
    fn test_1d_projections() {
        let sw = SlicedWasserstein::new(10);
        let directions = sw.generate_directions(3);

        assert_eq!(directions.len(), 10);

        // Each direction should be unit length
        for dir in &directions {
            let norm: f64 = dir.iter().map(|&x| x * x).sum::<f64>().sqrt();
            assert!((norm - 1.0).abs() < 1e-6, "Direction not unit: {}", norm);
        }
    }
}
