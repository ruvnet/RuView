//! Shard Management with Curvature Registry
//!
//! This module implements per-shard curvature management for hierarchical data.
//! Different parts of the hierarchy may have different optimal curvatures.
//!
//! # Features
//!
//! - Per-shard curvature configuration
//! - Hot reload of curvature parameters
//! - Canary testing for curvature updates
//! - Hierarchy preservation metrics

use crate::error::{HyperbolicError, HyperbolicResult};
use crate::hnsw::{HyperbolicHnsw, HyperbolicHnswConfig, SearchResult};
use crate::poincare::{frechet_mean, poincare_distance, project_to_ball, PoincareConfig, EPS};
use crate::tangent::TangentCache;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Curvature configuration for a shard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardCurvature {
    /// Current active curvature
    pub current: f32,
    /// Canary curvature (for testing)
    pub canary: Option<f32>,
    /// Traffic percentage for canary (0-100)
    pub canary_traffic: u8,
    /// Learned curvature from data
    pub learned: Option<f32>,
    /// Last update timestamp
    pub updated_at: i64,
}

impl Default for ShardCurvature {
    fn default() -> Self {
        Self {
            current: 1.0,
            canary: None,
            canary_traffic: 0,
            learned: None,
            updated_at: 0,
        }
    }
}

impl ShardCurvature {
    /// Get the effective curvature (considering canary traffic)
    pub fn effective(&self, use_canary: bool) -> f32 {
        if use_canary && self.canary.is_some() && self.canary_traffic > 0 {
            self.canary.unwrap()
        } else {
            self.current
        }
    }

    /// Promote canary to current
    pub fn promote_canary(&mut self) {
        if let Some(c) = self.canary {
            self.current = c;
            self.canary = None;
            self.canary_traffic = 0;
        }
    }

    /// Rollback canary
    pub fn rollback_canary(&mut self) {
        self.canary = None;
        self.canary_traffic = 0;
    }
}

/// Curvature registry for managing per-shard curvatures
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CurvatureRegistry {
    /// Shard curvatures by shard ID
    pub shards: HashMap<String, ShardCurvature>,
    /// Global default curvature
    pub default_curvature: f32,
    /// Registry version (for hot reload)
    pub version: u64,
}

impl CurvatureRegistry {
    /// Create a new registry with default curvature
    pub fn new(default_curvature: f32) -> Self {
        Self {
            shards: HashMap::new(),
            default_curvature,
            version: 0,
        }
    }

    /// Get curvature for a shard
    pub fn get(&self, shard_id: &str) -> f32 {
        self.shards
            .get(shard_id)
            .map(|s| s.current)
            .unwrap_or(self.default_curvature)
    }

    /// Get curvature with canary consideration
    pub fn get_effective(&self, shard_id: &str, use_canary: bool) -> f32 {
        self.shards
            .get(shard_id)
            .map(|s| s.effective(use_canary))
            .unwrap_or(self.default_curvature)
    }

    /// Set curvature for a shard
    pub fn set(&mut self, shard_id: &str, curvature: f32) {
        let entry = self.shards.entry(shard_id.to_string()).or_default();
        entry.current = curvature;
        entry.updated_at = chrono_timestamp();
        self.version += 1;
    }

    /// Set canary curvature
    pub fn set_canary(&mut self, shard_id: &str, curvature: f32, traffic: u8) {
        let entry = self.shards.entry(shard_id.to_string()).or_default();
        entry.canary = Some(curvature);
        entry.canary_traffic = traffic.min(100);
        entry.updated_at = chrono_timestamp();
        self.version += 1;
    }

    /// Promote all canaries
    pub fn promote_all_canaries(&mut self) {
        for (_, shard) in self.shards.iter_mut() {
            shard.promote_canary();
        }
        self.version += 1;
    }

    /// Rollback all canaries
    pub fn rollback_all_canaries(&mut self) {
        for (_, shard) in self.shards.iter_mut() {
            shard.rollback_canary();
        }
        self.version += 1;
    }

    /// Record learned curvature
    pub fn set_learned(&mut self, shard_id: &str, curvature: f32) {
        let entry = self.shards.entry(shard_id.to_string()).or_default();
        entry.learned = Some(curvature);
        entry.updated_at = chrono_timestamp();
    }
}

fn chrono_timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A single shard in the sharded HNSW system
#[derive(Debug)]
pub struct HyperbolicShard {
    /// Shard ID
    pub id: String,
    /// HNSW index for this shard
    pub index: HyperbolicHnsw,
    /// Tangent cache
    pub tangent_cache: Option<TangentCache>,
    /// Shard centroid
    pub centroid: Vec<f32>,
    /// Hierarchy depth range (min, max)
    pub depth_range: (usize, usize),
    /// Number of vectors in shard
    pub count: usize,
}

impl HyperbolicShard {
    /// Create a new shard
    pub fn new(id: String, curvature: f32) -> Self {
        let mut config = HyperbolicHnswConfig::default();
        config.curvature = curvature;

        Self {
            id,
            index: HyperbolicHnsw::new(config),
            tangent_cache: None,
            centroid: Vec::new(),
            depth_range: (0, 0),
            count: 0,
        }
    }

    /// Insert a vector
    pub fn insert(&mut self, vector: Vec<f32>) -> HyperbolicResult<usize> {
        let id = self.index.insert(vector)?;
        self.count += 1;
        // Invalidate tangent cache
        self.tangent_cache = None;
        Ok(id)
    }

    /// Build tangent cache
    pub fn build_cache(&mut self) -> HyperbolicResult<()> {
        if self.count == 0 {
            return Ok(());
        }

        let vectors: Vec<Vec<f32>> = self
            .index
            .vectors()
            .iter()
            .map(|v| v.to_vec())
            .collect();
        let indices: Vec<usize> = (0..vectors.len()).collect();

        self.tangent_cache = Some(TangentCache::new(
            &vectors,
            &indices,
            self.index.config.curvature,
        )?);

        if let Some(cache) = &self.tangent_cache {
            self.centroid = cache.centroid.clone();
        }

        Ok(())
    }

    /// Search with tangent pruning
    pub fn search(&self, query: &[f32], k: usize) -> HyperbolicResult<Vec<SearchResult>> {
        self.index.search(query, k)
    }

    /// Update curvature
    pub fn set_curvature(&mut self, curvature: f32) -> HyperbolicResult<()> {
        self.index.set_curvature(curvature)?;
        // Rebuild cache with new curvature
        if self.tangent_cache.is_some() {
            self.build_cache()?;
        }
        Ok(())
    }
}

/// Sharded hyperbolic HNSW manager
#[derive(Debug)]
pub struct ShardedHyperbolicHnsw {
    /// Shards by ID
    pub shards: HashMap<String, HyperbolicShard>,
    /// Curvature registry
    pub registry: CurvatureRegistry,
    /// Global ID to shard mapping
    pub id_to_shard: Vec<(String, usize)>,
    /// Shard assignment strategy
    pub strategy: ShardStrategy,
}

/// Strategy for assigning vectors to shards
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardStrategy {
    /// Assign by hash
    Hash,
    /// Assign by hierarchy depth
    Depth,
    /// Assign by radius (distance from origin)
    Radius,
    /// Round-robin
    RoundRobin,
}

impl Default for ShardStrategy {
    fn default() -> Self {
        Self::Radius
    }
}

impl ShardedHyperbolicHnsw {
    /// Create a new sharded manager
    pub fn new(default_curvature: f32) -> Self {
        Self {
            shards: HashMap::new(),
            registry: CurvatureRegistry::new(default_curvature),
            id_to_shard: Vec::new(),
            strategy: ShardStrategy::default(),
        }
    }

    /// Create or get a shard
    pub fn get_or_create_shard(&mut self, shard_id: &str) -> &mut HyperbolicShard {
        let curvature = self.registry.get(shard_id);
        self.shards
            .entry(shard_id.to_string())
            .or_insert_with(|| HyperbolicShard::new(shard_id.to_string(), curvature))
    }

    /// Determine shard for a vector
    pub fn assign_shard(&self, vector: &[f32], depth: Option<usize>) -> String {
        match self.strategy {
            ShardStrategy::Hash => {
                let hash: u64 = vector.iter().fold(0u64, |acc, &v| {
                    acc.wrapping_add((v.to_bits() as u64).wrapping_mul(31))
                });
                format!("shard_{}", hash % (self.shards.len().max(1) as u64))
            }
            ShardStrategy::Depth => {
                let d = depth.unwrap_or(0);
                format!("depth_{}", d / 10) // Group by depth buckets
            }
            ShardStrategy::Radius => {
                let radius: f32 = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                let bucket = (radius * 10.0) as usize;
                format!("radius_{}", bucket)
            }
            ShardStrategy::RoundRobin => {
                let idx = self.id_to_shard.len() % self.shards.len().max(1);
                self.shards
                    .keys()
                    .nth(idx)
                    .cloned()
                    .unwrap_or_else(|| "default".to_string())
            }
        }
    }

    /// Insert vector with automatic shard assignment
    pub fn insert(&mut self, vector: Vec<f32>, depth: Option<usize>) -> HyperbolicResult<usize> {
        let shard_id = self.assign_shard(&vector, depth);
        let shard = self.get_or_create_shard(&shard_id);
        let local_id = shard.insert(vector)?;

        let global_id = self.id_to_shard.len();
        self.id_to_shard.push((shard_id, local_id));

        Ok(global_id)
    }

    /// Insert into specific shard
    pub fn insert_to_shard(
        &mut self,
        shard_id: &str,
        vector: Vec<f32>,
    ) -> HyperbolicResult<usize> {
        let shard = self.get_or_create_shard(shard_id);
        let local_id = shard.insert(vector)?;

        let global_id = self.id_to_shard.len();
        self.id_to_shard.push((shard_id.to_string(), local_id));

        Ok(global_id)
    }

    /// Search across all shards
    pub fn search(&self, query: &[f32], k: usize) -> HyperbolicResult<Vec<(usize, SearchResult)>> {
        let mut all_results: Vec<(usize, SearchResult)> = Vec::new();

        for (shard_id, shard) in &self.shards {
            let results = shard.search(query, k)?;
            for result in results {
                // Map local ID to global ID
                if let Some((global_id, _)) = self.id_to_shard.iter().enumerate().find(|(_, (s, l))| s == shard_id && *l == result.id) {
                    all_results.push((global_id, result));
                }
            }
        }

        // Sort by distance and take top k
        all_results.sort_by(|a, b| a.1.distance.partial_cmp(&b.1.distance).unwrap());
        all_results.truncate(k);

        Ok(all_results)
    }

    /// Build all tangent caches
    pub fn build_caches(&mut self) -> HyperbolicResult<()> {
        for shard in self.shards.values_mut() {
            shard.build_cache()?;
        }
        Ok(())
    }

    /// Update curvature for a shard
    pub fn update_curvature(&mut self, shard_id: &str, curvature: f32) -> HyperbolicResult<()> {
        self.registry.set(shard_id, curvature);
        if let Some(shard) = self.shards.get_mut(shard_id) {
            shard.set_curvature(curvature)?;
        }
        Ok(())
    }

    /// Hot reload curvatures from registry
    pub fn reload_curvatures(&mut self) -> HyperbolicResult<()> {
        for (shard_id, shard) in self.shards.iter_mut() {
            let curvature = self.registry.get(shard_id);
            shard.set_curvature(curvature)?;
        }
        Ok(())
    }

    /// Get total vector count
    pub fn len(&self) -> usize {
        self.id_to_shard.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.id_to_shard.is_empty()
    }

    /// Get number of shards
    pub fn num_shards(&self) -> usize {
        self.shards.len()
    }
}

/// Metrics for hierarchy preservation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HierarchyMetrics {
    /// Spearman correlation between radius and depth
    pub radius_depth_correlation: f32,
    /// Average distance distortion
    pub distance_distortion: f32,
    /// Ancestor preservation (AUPRC)
    pub ancestor_auprc: f32,
    /// Mean rank
    pub mean_rank: f32,
    /// NDCG scores
    pub ndcg: HashMap<String, f32>,
}

impl HierarchyMetrics {
    /// Compute hierarchy metrics
    pub fn compute(
        points: &[Vec<f32>],
        depths: &[usize],
        curvature: f32,
    ) -> HyperbolicResult<Self> {
        if points.is_empty() || points.len() != depths.len() {
            return Err(HyperbolicError::EmptyCollection);
        }

        // Compute radii
        let radii: Vec<f32> = points
            .iter()
            .map(|p| p.iter().map(|v| v * v).sum::<f32>().sqrt())
            .collect();

        // Spearman correlation between radius and depth
        let radius_depth_correlation = spearman_correlation(&radii, depths);

        // Distance distortion (sample-based for efficiency)
        let sample_size = points.len().min(100);
        let mut distortion_sum = 0.0;
        let mut distortion_count = 0;

        for i in 0..sample_size {
            for j in (i + 1)..sample_size {
                let hyp_dist = poincare_distance(&points[i], &points[j], curvature);
                let depth_diff = (depths[i] as f32 - depths[j] as f32).abs();

                if depth_diff > 0.0 {
                    distortion_sum += (hyp_dist - depth_diff).abs() / depth_diff;
                    distortion_count += 1;
                }
            }
        }

        let distance_distortion = if distortion_count > 0 {
            distortion_sum / distortion_count as f32
        } else {
            0.0
        };

        Ok(Self {
            radius_depth_correlation,
            distance_distortion,
            ancestor_auprc: 0.0, // Requires ground truth
            mean_rank: 0.0,     // Requires ground truth
            ndcg: HashMap::new(),
        })
    }
}

/// Compute Spearman rank correlation
fn spearman_correlation(x: &[f32], y: &[usize]) -> f32 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len();

    // Compute ranks for x
    let mut x_indexed: Vec<(usize, f32)> = x.iter().cloned().enumerate().collect();
    x_indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
    let mut x_ranks = vec![0.0; n];
    for (rank, (idx, _)) in x_indexed.iter().enumerate() {
        x_ranks[*idx] = rank as f32;
    }

    // Compute ranks for y
    let mut y_indexed: Vec<(usize, usize)> = y.iter().cloned().enumerate().collect();
    y_indexed.sort_by_key(|a| a.1);
    let mut y_ranks = vec![0.0; n];
    for (rank, (idx, _)) in y_indexed.iter().enumerate() {
        y_ranks[*idx] = rank as f32;
    }

    // Compute Spearman correlation
    let mean_x: f32 = x_ranks.iter().sum::<f32>() / n as f32;
    let mean_y: f32 = y_ranks.iter().sum::<f32>() / n as f32;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..n {
        let dx = x_ranks[i] - mean_x;
        let dy = y_ranks[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    if var_x == 0.0 || var_y == 0.0 {
        return 0.0;
    }

    cov / (var_x * var_y).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curvature_registry() {
        let mut registry = CurvatureRegistry::new(1.0);

        registry.set("shard_1", 0.5);
        assert_eq!(registry.get("shard_1"), 0.5);
        assert_eq!(registry.get("shard_2"), 1.0); // Default

        registry.set_canary("shard_1", 0.3, 50);
        assert_eq!(registry.get_effective("shard_1", false), 0.5);
        assert_eq!(registry.get_effective("shard_1", true), 0.3);
    }

    #[test]
    fn test_sharded_hnsw() {
        let mut manager = ShardedHyperbolicHnsw::new(1.0);

        for i in 0..20 {
            let v = vec![0.1 * i as f32, 0.05 * i as f32];
            manager.insert(v, Some(i / 5)).unwrap();
        }

        assert_eq!(manager.len(), 20);

        let query = vec![0.3, 0.15];
        let results = manager.search(&query, 5).unwrap();
        assert!(!results.is_empty());
    }

    #[test]
    fn test_spearman() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![1, 2, 3, 4, 5];
        let corr = spearman_correlation(&x, &y);
        assert!((corr - 1.0).abs() < 0.01);

        let y_rev = vec![5, 4, 3, 2, 1];
        let corr_rev = spearman_correlation(&x, &y_rev);
        assert!((corr_rev + 1.0).abs() < 0.01);
    }
}
