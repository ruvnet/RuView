//! ReasoningBank - Pattern storage and extraction for SONA
//!
//! Implements trajectory clustering using K-means++ for pattern discovery.

use crate::sona::types::{LearnedPattern, PatternType, QueryTrajectory};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// ReasoningBank configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatternConfig {
    /// Number of clusters for K-means++
    pub k_clusters: usize,
    /// Embedding dimension
    pub embedding_dim: usize,
    /// Maximum K-means iterations
    pub max_iterations: usize,
    /// Convergence threshold
    pub convergence_threshold: f32,
    /// Minimum cluster size to keep
    pub min_cluster_size: usize,
    /// Maximum trajectories to store
    pub max_trajectories: usize,
    /// Quality threshold for pattern
    pub quality_threshold: f32,
}

impl Default for PatternConfig {
    fn default() -> Self {
        // OPTIMIZED DEFAULTS based on @ruvector/sona v0.1.1 benchmarks:
        // - 100 clusters = 1.3ms search vs 50 clusters = 3.0ms (2.3x faster)
        // - Quality threshold 0.3 balances learning vs noise filtering
        Self {
            k_clusters: 100, // OPTIMIZED: 2.3x faster search (1.3ms vs 3.0ms)
            embedding_dim: 256,
            max_iterations: 100,
            convergence_threshold: 0.001,
            min_cluster_size: 5,
            max_trajectories: 10000,
            quality_threshold: 0.3, // OPTIMIZED: Lower threshold for more learning
        }
    }
}

/// ReasoningBank for pattern storage and extraction
#[derive(Clone, Debug)]
pub struct ReasoningBank {
    /// Configuration
    config: PatternConfig,
    /// Stored trajectories
    trajectories: Vec<TrajectoryEntry>,
    /// Extracted patterns
    patterns: HashMap<u64, LearnedPattern>,
    /// Next pattern ID
    next_pattern_id: u64,
    /// Pattern index (embedding -> pattern_id)
    pattern_index: Vec<(Vec<f32>, u64)>,
}

/// Internal trajectory entry with embedding
#[derive(Clone, Debug)]
struct TrajectoryEntry {
    /// Trajectory embedding (query + avg activations)
    embedding: Vec<f32>,
    /// Quality score
    quality: f32,
    /// Cluster assignment
    cluster: Option<usize>,
    /// Original trajectory ID
    trajectory_id: u64,
}

impl ReasoningBank {
    /// Create new ReasoningBank
    pub fn new(config: PatternConfig) -> Self {
        Self {
            config,
            trajectories: Vec::new(),
            patterns: HashMap::new(),
            next_pattern_id: 0,
            pattern_index: Vec::new(),
        }
    }

    /// Add trajectory to bank
    pub fn add_trajectory(&mut self, trajectory: &QueryTrajectory) {
        // Compute embedding from trajectory
        let embedding = self.compute_embedding(trajectory);

        let entry = TrajectoryEntry {
            embedding,
            quality: trajectory.final_quality,
            cluster: None,
            trajectory_id: trajectory.id,
        };

        // Enforce capacity
        if self.trajectories.len() >= self.config.max_trajectories {
            // Remove oldest entries
            let to_remove = self.trajectories.len() - self.config.max_trajectories + 1;
            self.trajectories.drain(0..to_remove);
        }

        self.trajectories.push(entry);
    }

    /// Compute embedding from trajectory
    fn compute_embedding(&self, trajectory: &QueryTrajectory) -> Vec<f32> {
        let dim = self.config.embedding_dim;
        let mut embedding = vec![0.0f32; dim];

        // Start with query embedding
        let query_len = trajectory.query_embedding.len().min(dim);
        embedding[..query_len].copy_from_slice(&trajectory.query_embedding[..query_len]);

        // Average in step activations (weighted by reward)
        if !trajectory.steps.is_empty() {
            let mut total_reward = 0.0f32;

            for step in &trajectory.steps {
                let weight = step.reward.max(0.0);
                total_reward += weight;

                for (i, &act) in step.activations.iter().enumerate() {
                    if i < dim {
                        embedding[i] += act * weight;
                    }
                }
            }

            if total_reward > 0.0 {
                for e in &mut embedding {
                    *e /= total_reward + 1.0; // +1 for query contribution
                }
            }
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for e in &mut embedding {
                *e /= norm;
            }
        }

        embedding
    }

    /// Extract patterns using K-means++
    pub fn extract_patterns(&mut self) -> Vec<LearnedPattern> {
        if self.trajectories.is_empty() {
            return Vec::new();
        }

        let k = self.config.k_clusters.min(self.trajectories.len());
        if k == 0 {
            return Vec::new();
        }

        // K-means++ initialization
        let centroids = self.kmeans_plus_plus_init(k);

        // Run K-means
        let (final_centroids, assignments) = self.run_kmeans(centroids);

        // Create patterns from clusters
        let mut patterns = Vec::new();

        for (cluster_idx, centroid) in final_centroids.into_iter().enumerate() {
            // Collect cluster members
            let members: Vec<_> = self
                .trajectories
                .iter()
                .enumerate()
                .filter(|(i, _)| assignments.get(*i) == Some(&cluster_idx))
                .map(|(_, t)| t)
                .collect();

            if members.len() < self.config.min_cluster_size {
                continue;
            }

            // Compute cluster statistics
            let cluster_size = members.len();
            let total_weight: f32 = members.iter().map(|t| t.quality).sum();
            let avg_quality = total_weight / cluster_size as f32;

            if avg_quality < self.config.quality_threshold {
                continue;
            }

            let pattern_id = self.next_pattern_id;
            self.next_pattern_id += 1;

            let pattern = LearnedPattern {
                id: pattern_id,
                centroid,
                cluster_size,
                total_weight,
                avg_quality,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                last_accessed: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
                access_count: 0,
                pattern_type: PatternType::General,
            };

            self.patterns.insert(pattern_id, pattern.clone());
            self.pattern_index
                .push((pattern.centroid.clone(), pattern_id));
            patterns.push(pattern);
        }

        // Update trajectory cluster assignments
        for (i, cluster) in assignments.into_iter().enumerate() {
            if i < self.trajectories.len() {
                self.trajectories[i].cluster = Some(cluster);
            }
        }

        patterns
    }

    /// K-means++ initialization
    fn kmeans_plus_plus_init(&self, k: usize) -> Vec<Vec<f32>> {
        let mut centroids = Vec::with_capacity(k);
        let n = self.trajectories.len();

        if n == 0 || k == 0 {
            return centroids;
        }

        // First centroid: random (use deterministic selection for reproducibility)
        let first_idx = 0;
        centroids.push(self.trajectories[first_idx].embedding.clone());

        // Remaining centroids: D^2 weighting
        for _ in 1..k {
            // Compute distances to nearest centroid
            let mut distances: Vec<f32> = self
                .trajectories
                .iter()
                .map(|t| {
                    centroids
                        .iter()
                        .map(|c| self.squared_distance(&t.embedding, c))
                        .fold(f32::MAX, f32::min)
                })
                .collect();

            // Normalize to probabilities
            let total: f32 = distances.iter().sum();
            if total > 0.0 {
                for d in &mut distances {
                    *d /= total;
                }
            }

            // Select next centroid (deterministic: highest distance)
            let (next_idx, _) = distances
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap_or((0, &0.0));

            centroids.push(self.trajectories[next_idx].embedding.clone());
        }

        centroids
    }

    /// Run K-means algorithm
    fn run_kmeans(&self, mut centroids: Vec<Vec<f32>>) -> (Vec<Vec<f32>>, Vec<usize>) {
        let n = self.trajectories.len();
        let k = centroids.len();
        let dim = self.config.embedding_dim;

        let mut assignments = vec![0usize; n];

        for _iter in 0..self.config.max_iterations {
            // Assign points to nearest centroid
            let mut changed = false;
            for (i, t) in self.trajectories.iter().enumerate() {
                let (nearest, _) = centroids
                    .iter()
                    .enumerate()
                    .map(|(j, c)| (j, self.squared_distance(&t.embedding, c)))
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .unwrap_or((0, 0.0));

                if assignments[i] != nearest {
                    assignments[i] = nearest;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update centroids
            let mut new_centroids = vec![vec![0.0f32; dim]; k];
            let mut counts = vec![0usize; k];

            for (i, t) in self.trajectories.iter().enumerate() {
                let cluster = assignments[i];
                counts[cluster] += 1;
                for (j, &e) in t.embedding.iter().enumerate() {
                    new_centroids[cluster][j] += e;
                }
            }

            // Average and check convergence
            let mut max_shift = 0.0f32;
            for (i, new_c) in new_centroids.iter_mut().enumerate() {
                if counts[i] > 0 {
                    for e in new_c.iter_mut() {
                        *e /= counts[i] as f32;
                    }
                    let shift = self.squared_distance(new_c, &centroids[i]).sqrt();
                    max_shift = max_shift.max(shift);
                }
            }

            centroids = new_centroids;

            if max_shift < self.config.convergence_threshold {
                break;
            }
        }

        (centroids, assignments)
    }

    /// Squared Euclidean distance
    fn squared_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(&x, &y)| (x - y) * (x - y))
            .sum()
    }

    /// Find similar patterns
    pub fn find_similar(&self, query: &[f32], k: usize) -> Vec<&LearnedPattern> {
        let mut scored: Vec<_> = self
            .patterns
            .values()
            .map(|p| (p, p.similarity(query)))
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        scored.into_iter().take(k).map(|(p, _)| p).collect()
    }

    /// Get pattern by ID
    pub fn get_pattern(&self, id: u64) -> Option<&LearnedPattern> {
        self.patterns.get(&id)
    }

    /// Get mutable pattern by ID
    pub fn get_pattern_mut(&mut self, id: u64) -> Option<&mut LearnedPattern> {
        self.patterns.get_mut(&id)
    }

    /// Get trajectory count
    pub fn trajectory_count(&self) -> usize {
        self.trajectories.len()
    }

    /// Get pattern count
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Clear trajectories (keep patterns)
    pub fn clear_trajectories(&mut self) {
        self.trajectories.clear();
    }

    /// Prune low-quality patterns
    pub fn prune_patterns(&mut self, min_quality: f32, min_accesses: u32, max_age_secs: u64) {
        let to_remove: Vec<u64> = self
            .patterns
            .iter()
            .filter(|(_, p)| p.should_prune(min_quality, min_accesses, max_age_secs))
            .map(|(id, _)| *id)
            .collect();

        for id in to_remove {
            self.patterns.remove(&id);
        }

        // Update index
        self.pattern_index
            .retain(|(_, id)| self.patterns.contains_key(id));
    }

    /// Consolidate similar patterns
    pub fn consolidate(&mut self, similarity_threshold: f32) {
        let pattern_ids: Vec<u64> = self.patterns.keys().copied().collect();
        let mut merged = Vec::new();

        for i in 0..pattern_ids.len() {
            for j in i + 1..pattern_ids.len() {
                let id1 = pattern_ids[i];
                let id2 = pattern_ids[j];

                if merged.contains(&id1) || merged.contains(&id2) {
                    continue;
                }

                if let (Some(p1), Some(p2)) = (self.patterns.get(&id1), self.patterns.get(&id2)) {
                    let sim = p1.similarity(&p2.centroid);
                    if sim > similarity_threshold {
                        // Merge p2 into p1
                        let merged_pattern = p1.merge(p2);
                        self.patterns.insert(id1, merged_pattern);
                        merged.push(id2);
                    }
                }
            }
        }

        // Remove merged patterns
        for id in merged {
            self.patterns.remove(&id);
        }

        // Update index
        self.pattern_index
            .retain(|(_, id)| self.patterns.contains_key(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trajectory(id: u64, embedding: Vec<f32>, quality: f32) -> QueryTrajectory {
        let mut t = QueryTrajectory::new(id, embedding);
        t.finalize(quality, 1000);
        t
    }

    #[test]
    fn test_bank_creation() {
        let bank = ReasoningBank::new(PatternConfig::default());
        assert_eq!(bank.trajectory_count(), 0);
        assert_eq!(bank.pattern_count(), 0);
    }

    #[test]
    fn test_add_trajectory() {
        let config = PatternConfig {
            embedding_dim: 4,
            ..Default::default()
        };
        let mut bank = ReasoningBank::new(config);

        let t = make_trajectory(1, vec![0.1, 0.2, 0.3, 0.4], 0.8);
        bank.add_trajectory(&t);

        assert_eq!(bank.trajectory_count(), 1);
    }

    #[test]
    fn test_extract_patterns() {
        let config = PatternConfig {
            embedding_dim: 4,
            k_clusters: 2,
            min_cluster_size: 2,
            quality_threshold: 0.0,
            ..Default::default()
        };
        let mut bank = ReasoningBank::new(config);

        // Add clustered trajectories
        for i in 0..5 {
            let t = make_trajectory(i, vec![1.0, 0.0, 0.0, 0.0], 0.8);
            bank.add_trajectory(&t);
        }
        for i in 5..10 {
            let t = make_trajectory(i, vec![0.0, 1.0, 0.0, 0.0], 0.7);
            bank.add_trajectory(&t);
        }

        let patterns = bank.extract_patterns();
        assert!(!patterns.is_empty());
    }

    #[test]
    fn test_find_similar() {
        let config = PatternConfig {
            embedding_dim: 4,
            k_clusters: 2,
            min_cluster_size: 2,
            quality_threshold: 0.0,
            ..Default::default()
        };
        let mut bank = ReasoningBank::new(config);

        for i in 0..10 {
            let emb = if i < 5 {
                vec![1.0, 0.0, 0.0, 0.0]
            } else {
                vec![0.0, 1.0, 0.0, 0.0]
            };
            bank.add_trajectory(&make_trajectory(i, emb, 0.8));
        }

        bank.extract_patterns();

        let query = vec![0.9, 0.1, 0.0, 0.0];
        let similar = bank.find_similar(&query, 1);
        assert!(!similar.is_empty());
    }

    #[test]
    fn test_consolidate() {
        let config = PatternConfig {
            embedding_dim: 4,
            k_clusters: 3,
            min_cluster_size: 1,
            quality_threshold: 0.0,
            ..Default::default()
        };
        let mut bank = ReasoningBank::new(config);

        // Create very similar trajectories
        for i in 0..9 {
            let emb = vec![1.0 + (i as f32 * 0.001), 0.0, 0.0, 0.0];
            bank.add_trajectory(&make_trajectory(i, emb, 0.8));
        }

        bank.extract_patterns();
        let before = bank.pattern_count();

        bank.consolidate(0.99);
        let after = bank.pattern_count();

        assert!(after <= before);
    }
}
