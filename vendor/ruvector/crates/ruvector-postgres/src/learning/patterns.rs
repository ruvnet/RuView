//! Pattern extraction using k-means clustering

use super::trajectory::QueryTrajectory;

/// A learned pattern representing a cluster of similar queries
#[derive(Debug, Clone)]
pub struct LearnedPattern {
    /// Centroid vector of the pattern
    pub centroid: Vec<f32>,
    /// Optimal ef_search parameter for this pattern
    pub optimal_ef: usize,
    /// Optimal probes parameter for this pattern
    pub optimal_probes: usize,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
    /// Number of trajectories in this pattern
    pub sample_count: usize,
    /// Average latency for this pattern
    pub avg_latency_us: f64,
    /// Average precision (if feedback available)
    pub avg_precision: Option<f64>,
}

impl LearnedPattern {
    /// Create a new pattern
    pub fn new(
        centroid: Vec<f32>,
        optimal_ef: usize,
        optimal_probes: usize,
        confidence: f64,
        sample_count: usize,
        avg_latency_us: f64,
        avg_precision: Option<f64>,
    ) -> Self {
        Self {
            centroid,
            optimal_ef,
            optimal_probes,
            confidence,
            sample_count,
            avg_latency_us,
            avg_precision,
        }
    }

    /// Calculate similarity to a query vector (cosine similarity)
    pub fn similarity(&self, query: &[f32]) -> f64 {
        if query.len() != self.centroid.len() {
            return 0.0;
        }

        let dot: f32 = query.iter().zip(&self.centroid).map(|(a, b)| a * b).sum();
        let norm_q: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_c: f32 = self.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_q == 0.0 || norm_c == 0.0 {
            return 0.0;
        }

        (dot / (norm_q * norm_c)) as f64
    }
}

/// Pattern extractor using k-means clustering
pub struct PatternExtractor {
    /// Number of clusters
    k: usize,
    /// Maximum iterations for k-means
    max_iterations: usize,
}

impl PatternExtractor {
    /// Create a new pattern extractor
    pub fn new(k: usize) -> Self {
        Self {
            k,
            max_iterations: 100,
        }
    }

    /// Extract patterns from trajectories
    pub fn extract_patterns(&self, trajectories: &[QueryTrajectory]) -> Vec<LearnedPattern> {
        if trajectories.is_empty() || trajectories.len() < self.k {
            return Vec::new();
        }

        let dim = trajectories[0].query_vector.len();

        // Initialize centroids using k-means++
        let mut centroids = self.initialize_centroids(trajectories, dim);

        // Run k-means
        let mut assignments = vec![0; trajectories.len()];

        for _ in 0..self.max_iterations {
            let mut changed = false;

            // Assignment step
            for (i, traj) in trajectories.iter().enumerate() {
                let closest = self.find_closest_centroid(&traj.query_vector, &centroids);
                if assignments[i] != closest {
                    assignments[i] = closest;
                    changed = true;
                }
            }

            if !changed {
                break;
            }

            // Update step
            centroids = self.update_centroids(trajectories, &assignments, dim);
        }

        // Create patterns from clusters
        self.create_patterns(trajectories, &assignments, &centroids)
    }

    /// Initialize centroids using k-means++
    fn initialize_centroids(
        &self,
        trajectories: &[QueryTrajectory],
        _default_ivfflat_probes: usize,
    ) -> Vec<Vec<f32>> {
        let mut centroids = Vec::with_capacity(self.k);

        // First centroid: random
        centroids.push(trajectories[0].query_vector.clone());

        // Remaining centroids: weighted by distance
        for _ in 1..self.k {
            let mut distances = Vec::with_capacity(trajectories.len());

            for traj in trajectories {
                let min_dist = centroids
                    .iter()
                    .map(|c| self.euclidean_distance(&traj.query_vector, c))
                    .min_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                distances.push(min_dist);
            }

            // Select point with maximum distance
            let idx = distances
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            centroids.push(trajectories[idx].query_vector.clone());
        }

        centroids
    }

    /// Find closest centroid index
    fn find_closest_centroid(&self, point: &[f32], centroids: &[Vec<f32>]) -> usize {
        centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.euclidean_distance(point, c)))
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Update centroids based on assignments
    fn update_centroids(
        &self,
        trajectories: &[QueryTrajectory],
        assignments: &[usize],
        dim: usize,
    ) -> Vec<Vec<f32>> {
        let mut centroids = vec![vec![0.0; dim]; self.k];
        let mut counts = vec![0; self.k];

        for (traj, &cluster) in trajectories.iter().zip(assignments) {
            for (i, &val) in traj.query_vector.iter().enumerate() {
                centroids[cluster][i] += val;
            }
            counts[cluster] += 1;
        }

        for (centroid, &count) in centroids.iter_mut().zip(&counts) {
            if count > 0 {
                for val in centroid.iter_mut() {
                    *val /= count as f32;
                }
            }
        }

        centroids
    }

    /// Create patterns from clusters
    fn create_patterns(
        &self,
        trajectories: &[QueryTrajectory],
        assignments: &[usize],
        centroids: &[Vec<f32>],
    ) -> Vec<LearnedPattern> {
        let mut patterns = Vec::new();

        for cluster_id in 0..self.k {
            let cluster_trajs: Vec<&QueryTrajectory> = trajectories
                .iter()
                .zip(assignments)
                .filter(|(_, &a)| a == cluster_id)
                .map(|(t, _)| t)
                .collect();

            if cluster_trajs.is_empty() {
                continue;
            }

            // Calculate optimal parameters
            let optimal_ef = self.calculate_optimal_ef(&cluster_trajs);
            let optimal_probes = self.calculate_optimal_probes(&cluster_trajs);

            // Calculate statistics
            let sample_count = cluster_trajs.len();
            let avg_latency = cluster_trajs.iter().map(|t| t.latency_us).sum::<u64>() as f64
                / sample_count as f64;

            let precisions: Vec<f64> = cluster_trajs.iter().filter_map(|t| t.precision()).collect();
            let avg_precision = if !precisions.is_empty() {
                Some(precisions.iter().sum::<f64>() / precisions.len() as f64)
            } else {
                None
            };

            // Confidence based on sample count and consistency
            let confidence = self.calculate_confidence(&cluster_trajs);

            patterns.push(LearnedPattern::new(
                centroids[cluster_id].clone(),
                optimal_ef,
                optimal_probes,
                confidence,
                sample_count,
                avg_latency,
                avg_precision,
            ));
        }

        patterns
    }

    /// Calculate optimal ef_search for cluster
    fn calculate_optimal_ef(&self, trajectories: &[&QueryTrajectory]) -> usize {
        // Use median ef_search weighted by precision/latency trade-off
        let mut efs: Vec<_> = trajectories.iter().map(|t| t.ef_search).collect();
        efs.sort_unstable();

        if efs.is_empty() {
            return 50; // Default
        }

        efs[efs.len() / 2]
    }

    /// Calculate optimal probes for cluster
    fn calculate_optimal_probes(&self, trajectories: &[&QueryTrajectory]) -> usize {
        let mut probes: Vec<_> = trajectories.iter().map(|t| t.probes).collect();
        probes.sort_unstable();

        if probes.is_empty() {
            return 10; // Default
        }

        probes[probes.len() / 2]
    }

    /// Calculate confidence score
    fn calculate_confidence(&self, trajectories: &[&QueryTrajectory]) -> f64 {
        let n = trajectories.len() as f64;

        // Base confidence on sample size
        let size_confidence = (n / 100.0).min(1.0);

        // Consistency of parameters
        let ef_variance = self.calculate_variance(
            &trajectories
                .iter()
                .map(|t| t.ef_search as f64)
                .collect::<Vec<_>>(),
        );
        let consistency = 1.0 / (1.0 + ef_variance);

        // Combined confidence
        (size_confidence * 0.7 + consistency * 0.3).min(1.0)
    }

    /// Calculate variance
    fn calculate_variance(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / values.len() as f64;

        variance
    }

    /// Euclidean distance between vectors
    fn euclidean_distance(&self, a: &[f32], b: &[f32]) -> f64 {
        a.iter()
            .zip(b)
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_similarity() {
        let pattern =
            LearnedPattern::new(vec![1.0, 0.0, 0.0], 50, 10, 0.9, 100, 1000.0, Some(0.95));

        let query1 = vec![1.0, 0.0, 0.0]; // Same direction
        let query2 = vec![0.0, 1.0, 0.0]; // Perpendicular

        assert!((pattern.similarity(&query1) - 1.0).abs() < 0.001);
        assert!((pattern.similarity(&query2) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_pattern_extraction() {
        let trajectories = vec![
            QueryTrajectory::new(vec![1.0, 0.0], vec![1], 1000, 50, 10),
            QueryTrajectory::new(vec![1.1, 0.1], vec![1], 1100, 50, 10),
            QueryTrajectory::new(vec![0.0, 1.0], vec![2], 2000, 60, 15),
            QueryTrajectory::new(vec![0.1, 1.1], vec![2], 2100, 60, 15),
        ];

        let extractor = PatternExtractor::new(2);
        let patterns = extractor.extract_patterns(&trajectories);

        assert_eq!(patterns.len(), 2);
        assert!(patterns.iter().all(|p| p.sample_count > 0));
    }

    #[test]
    fn test_confidence_calculation() {
        let extractor = PatternExtractor::new(2);

        // Consistent trajectories - create bindings first to avoid temporary value drop
        let traj1 = QueryTrajectory::new(vec![1.0], vec![1], 1000, 50, 10);
        let traj2 = QueryTrajectory::new(vec![1.0], vec![1], 1000, 50, 10);
        let trajs: Vec<&QueryTrajectory> = vec![&traj1, &traj2];

        let confidence = extractor.calculate_confidence(&trajs);
        assert!(confidence > 0.0 && confidence <= 1.0);
    }
}
