//! Tangent Space Operations for HNSW Pruning Optimization
//!
//! This module implements the key optimization for hyperbolic HNSW:
//! - Precompute tangent space coordinates at shard centroids
//! - Use cheap Euclidean distance in tangent space for pruning
//! - Only compute exact Poincaré distance for final ranking
//!
//! # HNSW Speed Trick
//!
//! The core insight is that for points near a centroid c:
//! 1. Map points to tangent space: u = log_c(x)
//! 2. Euclidean distance ||u_q - u_p|| approximates hyperbolic distance
//! 3. Prune candidates using fast Euclidean comparisons
//! 4. Rank final top-N candidates with exact Poincaré distance

use crate::error::{HyperbolicError, HyperbolicResult};
use crate::poincare::{
    conformal_factor, frechet_mean, log_map, norm, norm_squared, poincare_distance,
    project_to_ball, PoincareConfig, EPS,
};
use serde::{Deserialize, Serialize};

/// Tangent space cache for a shard
///
/// Stores precomputed tangent coordinates for fast pruning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TangentCache {
    /// Centroid point (base of tangent space)
    pub centroid: Vec<f32>,
    /// Precomputed tangent coordinates for all points in shard
    pub tangent_coords: Vec<Vec<f32>>,
    /// Original point indices
    pub point_indices: Vec<usize>,
    /// Curvature parameter
    pub curvature: f32,
    /// Cached conformal factor at centroid
    conformal: f32,
}

impl TangentCache {
    /// Create a new tangent cache for a shard
    ///
    /// # Arguments
    /// * `points` - Points in the shard (Poincaré ball coordinates)
    /// * `indices` - Original indices of the points
    /// * `curvature` - Curvature parameter
    pub fn new(points: &[Vec<f32>], indices: &[usize], curvature: f32) -> HyperbolicResult<Self> {
        if points.is_empty() {
            return Err(HyperbolicError::EmptyCollection);
        }

        let config = PoincareConfig::with_curvature(curvature)?;

        // Compute centroid as Fréchet mean
        let point_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();
        let centroid = frechet_mean(&point_refs, None, &config)?;

        // Precompute tangent coordinates
        let tangent_coords: Vec<Vec<f32>> = points
            .iter()
            .map(|p| log_map(p, &centroid, curvature))
            .collect();

        let conformal = conformal_factor(&centroid, curvature);

        Ok(Self {
            centroid,
            tangent_coords,
            point_indices: indices.to_vec(),
            curvature,
            conformal,
        })
    }

    /// Create from centroid directly (for incremental updates)
    pub fn from_centroid(
        centroid: Vec<f32>,
        points: &[Vec<f32>],
        indices: &[usize],
        curvature: f32,
    ) -> HyperbolicResult<Self> {
        let tangent_coords: Vec<Vec<f32>> = points
            .iter()
            .map(|p| log_map(p, &centroid, curvature))
            .collect();

        let conformal = conformal_factor(&centroid, curvature);

        Ok(Self {
            centroid,
            tangent_coords,
            point_indices: indices.to_vec(),
            curvature,
            conformal,
        })
    }

    /// Get tangent coordinates for a query point
    pub fn query_tangent(&self, query: &[f32]) -> Vec<f32> {
        log_map(query, &self.centroid, self.curvature)
    }

    /// Fast Euclidean distance in tangent space (for pruning)
    #[inline]
    pub fn tangent_distance_squared(&self, query_tangent: &[f32], idx: usize) -> f32 {
        if idx >= self.tangent_coords.len() {
            return f32::MAX;
        }

        let p = &self.tangent_coords[idx];
        query_tangent
            .iter()
            .zip(p.iter())
            .map(|(&a, &b)| (a - b) * (a - b))
            .sum()
    }

    /// Exact Poincaré distance for final ranking
    pub fn exact_distance(&self, query: &[f32], idx: usize, points: &[Vec<f32>]) -> f32 {
        if idx >= points.len() {
            return f32::MAX;
        }
        poincare_distance(query, &points[idx], self.curvature)
    }

    /// Add a new point to the cache (for incremental updates)
    pub fn add_point(&mut self, point: &[f32], index: usize) {
        let tangent = log_map(point, &self.centroid, self.curvature);
        self.tangent_coords.push(tangent);
        self.point_indices.push(index);
    }

    /// Update centroid and recompute all tangent coordinates
    pub fn recompute_centroid(&mut self, points: &[Vec<f32>]) -> HyperbolicResult<()> {
        if points.is_empty() {
            return Err(HyperbolicError::EmptyCollection);
        }

        let config = PoincareConfig::with_curvature(self.curvature)?;
        let point_refs: Vec<&[f32]> = points.iter().map(|p| p.as_slice()).collect();
        self.centroid = frechet_mean(&point_refs, None, &config)?;

        self.tangent_coords = points
            .iter()
            .map(|p| log_map(p, &self.centroid, self.curvature))
            .collect();

        self.conformal = conformal_factor(&self.centroid, self.curvature);

        Ok(())
    }

    /// Get number of points in cache
    pub fn len(&self) -> usize {
        self.tangent_coords.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.tangent_coords.is_empty()
    }

    /// Get the dimension of the tangent space
    pub fn dim(&self) -> usize {
        self.centroid.len()
    }
}

/// Tangent space pruning result
#[derive(Debug, Clone)]
pub struct PrunedCandidate {
    /// Original index
    pub index: usize,
    /// Tangent space distance (for initial ranking)
    pub tangent_dist: f32,
    /// Exact Poincaré distance (computed lazily)
    pub exact_dist: Option<f32>,
}

/// Tangent space pruner for HNSW neighbor selection
///
/// Implements the two-phase search:
/// 1. Fast pruning using Euclidean distance in tangent space
/// 2. Exact ranking using Poincaré distance for top candidates
pub struct TangentPruner {
    /// Tangent caches for each shard
    caches: Vec<TangentCache>,
    /// Number of candidates to consider in exact phase
    top_n: usize,
    /// Pruning factor (how many candidates to keep from tangent phase)
    prune_factor: usize,
}

impl TangentPruner {
    /// Create a new pruner
    ///
    /// # Arguments
    /// * `top_n` - Number of final results
    /// * `prune_factor` - Multiplier for candidates to consider (e.g., 10 means consider 10*top_n)
    pub fn new(top_n: usize, prune_factor: usize) -> Self {
        Self {
            caches: Vec::new(),
            top_n,
            prune_factor,
        }
    }

    /// Add a shard cache
    pub fn add_cache(&mut self, cache: TangentCache) {
        self.caches.push(cache);
    }

    /// Get shard caches
    pub fn caches(&self) -> &[TangentCache] {
        &self.caches
    }

    /// Get mutable shard caches
    pub fn caches_mut(&mut self) -> &mut [TangentCache] {
        &mut self.caches
    }

    /// Search across all shards with tangent pruning
    ///
    /// Returns top_n candidates sorted by exact Poincaré distance.
    pub fn search(
        &self,
        query: &[f32],
        points: &[Vec<f32>],
        curvature: f32,
    ) -> Vec<PrunedCandidate> {
        let num_prune = self.top_n * self.prune_factor;
        let mut candidates: Vec<PrunedCandidate> = Vec::with_capacity(num_prune);

        // Phase 1: Tangent space pruning across all shards
        for cache in &self.caches {
            let query_tangent = cache.query_tangent(query);

            for (local_idx, &global_idx) in cache.point_indices.iter().enumerate() {
                let tangent_dist = cache.tangent_distance_squared(&query_tangent, local_idx);
                candidates.push(PrunedCandidate {
                    index: global_idx,
                    tangent_dist,
                    exact_dist: None,
                });
            }
        }

        // Sort by tangent distance and keep top prune_factor * top_n
        candidates.sort_by(|a, b| a.tangent_dist.partial_cmp(&b.tangent_dist).unwrap());
        candidates.truncate(num_prune);

        // Phase 2: Exact Poincaré distance for finalists
        for candidate in &mut candidates {
            if candidate.index < points.len() {
                candidate.exact_dist =
                    Some(poincare_distance(query, &points[candidate.index], curvature));
            }
        }

        // Sort by exact distance and return top_n
        candidates.sort_by(|a, b| {
            a.exact_dist
                .unwrap_or(f32::MAX)
                .partial_cmp(&b.exact_dist.unwrap_or(f32::MAX))
                .unwrap()
        });
        candidates.truncate(self.top_n);

        candidates
    }
}

/// Compute micro tangent update for incremental operations
///
/// For small updates (reflex loop), compute tangent-space delta
/// that keeps the point inside the ball.
pub fn tangent_micro_update(
    point: &[f32],
    delta: &[f32],
    centroid: &[f32],
    curvature: f32,
    max_step: f32,
) -> Vec<f32> {
    // Get current tangent coordinates
    let tangent = log_map(point, centroid, curvature);

    // Apply bounded delta in tangent space
    let delta_norm = norm(delta);
    let scale = if delta_norm > max_step {
        max_step / delta_norm
    } else {
        1.0
    };

    let new_tangent: Vec<f32> = tangent
        .iter()
        .zip(delta.iter())
        .map(|(&t, &d)| t + scale * d)
        .collect();

    // Map back to ball and project
    let new_point = crate::poincare::exp_map(&new_tangent, centroid, curvature);
    project_to_ball(&new_point, curvature, EPS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tangent_cache_creation() {
        let points = vec![
            vec![0.1, 0.2, 0.1],
            vec![-0.1, 0.15, 0.05],
            vec![0.2, -0.1, 0.1],
        ];
        let indices: Vec<usize> = (0..3).collect();

        let cache = TangentCache::new(&points, &indices, 1.0).unwrap();

        assert_eq!(cache.len(), 3);
        assert_eq!(cache.dim(), 3);
    }

    #[test]
    fn test_tangent_pruning() {
        let points = vec![
            vec![0.1, 0.2],
            vec![-0.1, 0.15],
            vec![0.2, -0.1],
            vec![0.05, 0.05],
        ];
        let indices: Vec<usize> = (0..4).collect();

        let cache = TangentCache::new(&points, &indices, 1.0).unwrap();

        let mut pruner = TangentPruner::new(2, 2);
        pruner.add_cache(cache);

        let query = vec![0.08, 0.1];
        let results = pruner.search(&query, &points, 1.0);

        assert_eq!(results.len(), 2);
        // Results should be sorted by exact distance
        assert!(results[0].exact_dist.unwrap() <= results[1].exact_dist.unwrap());
    }
}
