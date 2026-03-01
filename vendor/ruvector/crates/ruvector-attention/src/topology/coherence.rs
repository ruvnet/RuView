//! Window Coherence Metrics
//!
//! Fast structural metrics for measuring attention window stability.
//! These are permission signals, not similarity signals.

use serde::{Deserialize, Serialize};

/// Coherence metric type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoherenceMetric {
    /// k-NN graph boundary ratio
    BoundaryMass,
    /// Cut proxy score (edge cut estimate)
    CutProxy,
    /// Disagreement across neighbor labels
    Disagreement,
    /// Average neighbor similarity variance
    SimilarityVariance,
}

/// Per-window coherence scores
#[derive(Debug, Clone)]
pub struct WindowCoherence {
    /// Overall coherence score (0 = fragmented, 1 = coherent)
    pub score: f32,
    /// Individual metric scores
    pub metric_scores: Vec<f32>,
    /// Which metrics were used
    pub metrics: Vec<CoherenceMetric>,
    /// Number of keys in window
    pub window_size: usize,
    /// Whether this coherence is stale (needs update)
    pub is_stale: bool,
    /// Token count since last update
    pub tokens_since_update: usize,
}

impl WindowCoherence {
    /// Compute coherence from keys
    pub fn compute(keys: &[&[f32]], k_neighbors: usize, metrics: &[CoherenceMetric]) -> Self {
        let n = keys.len();
        if n < 2 {
            return Self {
                score: 1.0,
                metric_scores: vec![1.0],
                metrics: metrics.to_vec(),
                window_size: n,
                is_stale: false,
                tokens_since_update: 0,
            };
        }

        // Build k-NN graph (fast approximate)
        let knn_graph = Self::build_knn_graph(keys, k_neighbors);

        // Compute each metric
        let metric_scores: Vec<f32> = metrics
            .iter()
            .map(|m| Self::compute_metric(*m, keys, &knn_graph))
            .collect();

        // Average scores for overall coherence
        let score = metric_scores.iter().sum::<f32>() / metric_scores.len() as f32;

        Self {
            score,
            metric_scores,
            metrics: metrics.to_vec(),
            window_size: n,
            is_stale: false,
            tokens_since_update: 0,
        }
    }

    /// Mark as stale (needs recomputation)
    pub fn mark_stale(&mut self) {
        self.is_stale = true;
    }

    /// Increment token counter
    pub fn tick(&mut self) {
        self.tokens_since_update += 1;
    }

    /// Check if update is needed based on period
    pub fn needs_update(&self, update_period: usize) -> bool {
        self.is_stale || self.tokens_since_update >= update_period
    }

    /// Build approximate k-NN graph
    /// Returns [N × k] indices of nearest neighbors
    fn build_knn_graph(keys: &[&[f32]], k: usize) -> Vec<Vec<usize>> {
        let n = keys.len();
        let k = k.min(n - 1);

        keys.iter()
            .enumerate()
            .map(|(i, key)| {
                let mut distances: Vec<(usize, f32)> = keys
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(j, k2)| (j, Self::squared_distance(key, k2)))
                    .collect();

                distances.sort_unstable_by(|a, b| {
                    a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                });

                distances.iter().take(k).map(|(j, _)| *j).collect()
            })
            .collect()
    }

    /// Squared Euclidean distance
    #[inline]
    fn squared_distance(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(&ai, &bi)| (ai - bi) * (ai - bi))
            .sum()
    }

    /// Compute specific metric
    fn compute_metric(metric: CoherenceMetric, keys: &[&[f32]], knn_graph: &[Vec<usize>]) -> f32 {
        match metric {
            CoherenceMetric::BoundaryMass => Self::boundary_mass(knn_graph),
            CoherenceMetric::CutProxy => Self::cut_proxy(knn_graph),
            CoherenceMetric::Disagreement => Self::disagreement(keys, knn_graph),
            CoherenceMetric::SimilarityVariance => Self::similarity_variance(keys, knn_graph),
        }
    }

    /// Boundary mass: fraction of edges going to "far" neighbors
    /// High coherence = most edges go to nearby neighbors
    fn boundary_mass(knn_graph: &[Vec<usize>]) -> f32 {
        if knn_graph.is_empty() {
            return 1.0;
        }

        let n = knn_graph.len();
        let mut internal_edges = 0;
        let mut total_edges = 0;

        for (i, neighbors) in knn_graph.iter().enumerate() {
            for &j in neighbors {
                total_edges += 1;
                // "Internal" if neighbor is within n/4 positions
                if (i as i32 - j as i32).unsigned_abs() as usize <= n / 4 {
                    internal_edges += 1;
                }
            }
        }

        if total_edges == 0 {
            return 1.0;
        }

        internal_edges as f32 / total_edges as f32
    }

    /// Cut proxy: estimate of graph cut cost
    /// High coherence = low cut (well-connected)
    fn cut_proxy(knn_graph: &[Vec<usize>]) -> f32 {
        if knn_graph.is_empty() {
            return 1.0;
        }

        let n = knn_graph.len();
        let half = n / 2;

        // Count edges crossing the midpoint
        let mut crossing = 0;
        let mut total = 0;

        for (i, neighbors) in knn_graph.iter().enumerate() {
            for &j in neighbors {
                total += 1;
                if (i < half) != (j < half) {
                    crossing += 1;
                }
            }
        }

        if total == 0 {
            return 1.0;
        }

        // Invert: high coherence = few crossings
        1.0 - (crossing as f32 / total as f32)
    }

    /// Disagreement: variance in neighbor similarities
    /// High coherence = neighbors have similar similarities
    fn disagreement(keys: &[&[f32]], knn_graph: &[Vec<usize>]) -> f32 {
        if knn_graph.is_empty() || keys.is_empty() {
            return 1.0;
        }

        let mut total_variance = 0.0f32;
        let mut count = 0;

        for (i, neighbors) in knn_graph.iter().enumerate() {
            if neighbors.is_empty() {
                continue;
            }

            // Similarities to neighbors
            let sims: Vec<f32> = neighbors
                .iter()
                .map(|&j| Self::cosine_similarity(keys[i], keys[j]))
                .collect();

            let mean: f32 = sims.iter().sum::<f32>() / sims.len() as f32;
            let variance: f32 =
                sims.iter().map(|s| (s - mean) * (s - mean)).sum::<f32>() / sims.len() as f32;

            total_variance += variance;
            count += 1;
        }

        if count == 0 {
            return 1.0;
        }

        // Low variance = high coherence
        let avg_variance = total_variance / count as f32;
        1.0 - avg_variance.min(1.0)
    }

    /// Similarity variance across window
    fn similarity_variance(keys: &[&[f32]], knn_graph: &[Vec<usize>]) -> f32 {
        if knn_graph.is_empty() || keys.is_empty() {
            return 1.0;
        }

        // Collect all neighbor similarities
        let mut all_sims = Vec::new();
        for (i, neighbors) in knn_graph.iter().enumerate() {
            for &j in neighbors {
                all_sims.push(Self::cosine_similarity(keys[i], keys[j]));
            }
        }

        if all_sims.is_empty() {
            return 1.0;
        }

        let mean: f32 = all_sims.iter().sum::<f32>() / all_sims.len() as f32;
        let variance: f32 = all_sims
            .iter()
            .map(|s| (s - mean) * (s - mean))
            .sum::<f32>()
            / all_sims.len() as f32;

        // Low variance + high mean = high coherence
        let coherence = mean * (1.0 - variance.sqrt().min(1.0));
        coherence.max(0.0).min(1.0)
    }

    /// Cosine similarity
    #[inline]
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(&ai, &bi)| ai * bi).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a < 1e-8 || norm_b < 1e-8 {
            return 0.0;
        }

        (dot / (norm_a * norm_b)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coherence_computation() {
        let keys: Vec<Vec<f32>> = (0..20).map(|i| vec![i as f32 * 0.1; 32]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let coherence = WindowCoherence::compute(
            &keys_refs,
            5,
            &[
                CoherenceMetric::BoundaryMass,
                CoherenceMetric::SimilarityVariance,
            ],
        );

        assert!(coherence.score >= 0.0 && coherence.score <= 1.0);
        assert_eq!(coherence.window_size, 20);
    }

    #[test]
    fn test_coherent_window() {
        // Highly similar keys = high coherence
        let keys: Vec<Vec<f32>> = (0..10).map(|_| vec![0.5f32; 16]).collect();
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let coherence = WindowCoherence::compute(&keys_refs, 3, &[CoherenceMetric::Disagreement]);

        // Should be very coherent
        assert!(coherence.score > 0.8);
    }

    #[test]
    fn test_stale_tracking() {
        let keys: Vec<Vec<f32>> = vec![vec![1.0; 8]; 5];
        let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();

        let mut coherence =
            WindowCoherence::compute(&keys_refs, 2, &[CoherenceMetric::BoundaryMass]);

        assert!(!coherence.needs_update(4));

        coherence.tick();
        coherence.tick();
        coherence.tick();
        coherence.tick();

        assert!(coherence.needs_update(4));
    }
}
