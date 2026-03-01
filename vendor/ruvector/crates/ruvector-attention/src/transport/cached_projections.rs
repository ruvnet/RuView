//! Cached Projections for Fast OT
//!
//! Pre-compute and cache random projections per window to avoid
//! redundant computation across queries.

use rand::prelude::*;
use rand::rngs::StdRng;

/// Cache for random projection directions
#[derive(Debug, Clone)]
pub struct ProjectionCache {
    /// Random unit directions [P × dim]
    pub directions: Vec<Vec<f32>>,
    /// Number of projections
    pub num_projections: usize,
    /// Dimension
    pub dim: usize,
}

impl ProjectionCache {
    /// Create new projection cache with P random directions
    pub fn new(dim: usize, num_projections: usize, seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);

        let directions: Vec<Vec<f32>> = (0..num_projections)
            .map(|_| {
                let mut dir: Vec<f32> = (0..dim)
                    .map(|_| rng.sample::<f32, _>(rand::distributions::Standard) * 2.0 - 1.0)
                    .collect();
                // Normalize to unit vector
                let norm: f32 = dir.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 1e-8 {
                    for x in &mut dir {
                        *x /= norm;
                    }
                }
                dir
            })
            .collect();

        Self {
            directions,
            num_projections,
            dim,
        }
    }

    /// Project a single vector onto all directions
    /// Returns [P] projected values
    #[inline]
    pub fn project(&self, vector: &[f32]) -> Vec<f32> {
        self.directions
            .iter()
            .map(|dir| Self::dot_product_simd(vector, dir))
            .collect()
    }

    /// Project a single vector into pre-allocated buffer
    #[inline]
    pub fn project_into(&self, vector: &[f32], out: &mut [f32]) {
        for (i, dir) in self.directions.iter().enumerate() {
            out[i] = Self::dot_product_simd(vector, dir);
        }
    }

    /// SIMD-friendly 4-way unrolled dot product
    #[inline(always)]
    fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
        let len = a.len();
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

/// Per-window cache containing sorted projections
#[derive(Debug, Clone)]
pub struct WindowCache {
    /// Projected keys [num_keys × P]
    pub key_projections: Vec<Vec<f32>>,
    /// Sorted indices per projection [P × num_keys]
    pub sorted_indices: Vec<Vec<usize>>,
    /// Sorted values per projection [P × num_keys]
    pub sorted_values: Vec<Vec<f32>>,
    /// Histogram bins per projection [P × num_bins]
    pub histograms: Option<Vec<Vec<f32>>>,
    /// CDF per projection [P × num_bins]
    pub cdfs: Option<Vec<Vec<f32>>>,
    /// Number of keys in window
    pub num_keys: usize,
}

impl WindowCache {
    /// Build cache from keys using projection cache
    pub fn build(keys: &[&[f32]], proj_cache: &ProjectionCache) -> Self {
        let num_keys = keys.len();
        let num_proj = proj_cache.num_projections;

        // Project all keys
        let key_projections: Vec<Vec<f32>> = keys.iter().map(|k| proj_cache.project(k)).collect();

        // Sort indices and values for each projection
        let mut sorted_indices = vec![Vec::with_capacity(num_keys); num_proj];
        let mut sorted_values = vec![Vec::with_capacity(num_keys); num_proj];

        for p in 0..num_proj {
            let mut indexed: Vec<(usize, f32)> = key_projections
                .iter()
                .enumerate()
                .map(|(i, projs)| (i, projs[p]))
                .collect();
            indexed.sort_unstable_by(|a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            sorted_indices[p] = indexed.iter().map(|(i, _)| *i).collect();
            sorted_values[p] = indexed.iter().map(|(_, v)| *v).collect();
        }

        Self {
            key_projections,
            sorted_indices,
            sorted_values,
            histograms: None,
            cdfs: None,
            num_keys,
        }
    }

    /// Build histograms for ultra-fast CDF comparison
    pub fn build_histograms(&mut self, num_bins: usize) {
        let num_proj = self.sorted_values.len();

        let mut histograms = vec![vec![0.0f32; num_bins]; num_proj];
        let mut cdfs = vec![vec![0.0f32; num_bins]; num_proj];

        for p in 0..num_proj {
            let vals = &self.sorted_values[p];
            if vals.is_empty() {
                continue;
            }

            let min_val = vals[0];
            let max_val = vals[vals.len() - 1];
            let range = (max_val - min_val).max(1e-8);

            // Build histogram
            for &v in vals {
                let bin = ((v - min_val) / range * (num_bins - 1) as f32)
                    .clamp(0.0, (num_bins - 1) as f32) as usize;
                histograms[p][bin] += 1.0 / self.num_keys as f32;
            }

            // Build CDF
            let mut cumsum = 0.0f32;
            for bin in 0..num_bins {
                cumsum += histograms[p][bin];
                cdfs[p][bin] = cumsum;
            }
        }

        self.histograms = Some(histograms);
        self.cdfs = Some(cdfs);
    }

    /// Get sorted values for a projection
    #[inline]
    pub fn get_sorted(&self, projection_idx: usize) -> &[f32] {
        &self.sorted_values[projection_idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projection_cache() {
        let cache = ProjectionCache::new(64, 8, 42);

        assert_eq!(cache.num_projections, 8);
        assert_eq!(cache.dim, 64);

        // Check directions are unit vectors
        for dir in &cache.directions {
            let norm: f32 = dir.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_window_cache() {
        let proj_cache = ProjectionCache::new(32, 4, 42);

        let keys: Vec<Vec<f32>> = (0..10).map(|i| vec![i as f32 * 0.1; 32]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let window_cache = WindowCache::build(&keys_refs, &proj_cache);

        assert_eq!(window_cache.num_keys, 10);
        assert_eq!(window_cache.sorted_indices.len(), 4);
    }

    #[test]
    fn test_histograms() {
        let proj_cache = ProjectionCache::new(16, 2, 42);

        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32 * 0.05; 16]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let mut window_cache = WindowCache::build(&keys_refs, &proj_cache);
        window_cache.build_histograms(10);

        assert!(window_cache.cdfs.is_some());

        // CDF should end at 1.0
        let cdfs = window_cache.cdfs.as_ref().unwrap();
        for cdf in cdfs {
            assert!((cdf[cdf.len() - 1] - 1.0).abs() < 1e-5);
        }
    }
}
