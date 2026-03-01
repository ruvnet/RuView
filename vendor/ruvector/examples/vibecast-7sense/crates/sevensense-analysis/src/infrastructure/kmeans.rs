//! K-Means clustering implementation.
//!
//! Standard K-Means algorithm with k-means++ initialization for
//! partitioning embeddings into k clusters.

use ndarray::{Array2, ArrayView1};
use tracing::{debug, instrument};

use crate::application::services::AnalysisError;

/// K-Means clustering algorithm.
pub struct KMeansClusterer {
    /// Number of clusters.
    k: usize,
    /// Maximum iterations.
    max_iterations: usize,
    /// Convergence tolerance.
    tolerance: f32,
    /// Random seed for reproducibility.
    seed: Option<u64>,
}

impl KMeansClusterer {
    /// Create a new K-Means clusterer.
    #[must_use]
    pub fn new(k: usize, seed: Option<u64>) -> Self {
        Self {
            k,
            max_iterations: 300,
            tolerance: 1e-4,
            seed,
        }
    }

    /// Set maximum iterations.
    #[must_use]
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = max_iterations;
        self
    }

    /// Set convergence tolerance.
    #[must_use]
    pub fn with_tolerance(mut self, tolerance: f32) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Fit K-Means to the data and return cluster labels and centroids.
    ///
    /// # Arguments
    ///
    /// * `data` - 2D array where rows are samples and columns are features
    ///
    /// # Returns
    ///
    /// Tuple of (cluster labels, centroid matrix)
    #[instrument(skip(self, data), fields(n_samples = data.nrows(), n_features = data.ncols(), k = self.k))]
    pub fn fit(&self, data: &Array2<f32>) -> Result<(Vec<usize>, Array2<f32>), AnalysisError> {
        let n = data.nrows();
        let d = data.ncols();

        if n < self.k {
            return Err(AnalysisError::InsufficientData(format!(
                "Need at least {} samples for k={}, got {}",
                self.k, self.k, n
            )));
        }

        debug!(
            n_samples = n,
            n_features = d,
            k = self.k,
            "Starting K-Means fit"
        );

        // Initialize centroids using k-means++ algorithm
        let mut centroids = self.kmeans_plus_plus_init(data);

        let mut labels = vec![0usize; n];
        let mut prev_inertia = f32::MAX;

        for iteration in 0..self.max_iterations {
            // Assignment step: assign each point to nearest centroid
            for i in 0..n {
                let point = data.row(i);
                let mut min_dist = f32::MAX;
                let mut best_cluster = 0;

                for (j, centroid) in centroids.outer_iter().enumerate() {
                    let dist = self.euclidean_distance(point, centroid);
                    if dist < min_dist {
                        min_dist = dist;
                        best_cluster = j;
                    }
                }

                labels[i] = best_cluster;
            }

            // Update step: compute new centroids
            let mut new_centroids = Array2::<f32>::zeros((self.k, d));
            let mut counts = vec![0usize; self.k];

            for (i, &label) in labels.iter().enumerate() {
                for j in 0..d {
                    new_centroids[[label, j]] += data[[i, j]];
                }
                counts[label] += 1;
            }

            for j in 0..self.k {
                if counts[j] > 0 {
                    for l in 0..d {
                        new_centroids[[j, l]] /= counts[j] as f32;
                    }
                } else {
                    // Handle empty cluster by keeping old centroid
                    for l in 0..d {
                        new_centroids[[j, l]] = centroids[[j, l]];
                    }
                }
            }

            // Compute inertia (sum of squared distances to centroids)
            let inertia: f32 = labels
                .iter()
                .enumerate()
                .map(|(i, &label)| {
                    self.euclidean_distance(data.row(i), centroids.row(label)).powi(2)
                })
                .sum();

            // Check convergence
            let inertia_change = (prev_inertia - inertia).abs() / prev_inertia.max(1.0);

            debug!(
                iteration = iteration,
                inertia = inertia,
                change = inertia_change,
                "K-Means iteration"
            );

            if inertia_change < self.tolerance {
                debug!(
                    iterations = iteration + 1,
                    final_inertia = inertia,
                    "K-Means converged"
                );
                break;
            }

            centroids = new_centroids;
            prev_inertia = inertia;
        }

        Ok((labels, centroids))
    }

    /// Initialize centroids using k-means++ algorithm.
    fn kmeans_plus_plus_init(&self, data: &Array2<f32>) -> Array2<f32> {
        let n = data.nrows();
        let d = data.ncols();
        let mut centroids = Array2::<f32>::zeros((self.k, d));

        // Use seed for deterministic initialization if provided
        let seed = self.seed.unwrap_or(42);
        let mut rng_state = seed;

        // Helper function for pseudo-random number generation
        let mut next_random = || {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((rng_state >> 33) as f32) / (u32::MAX as f32)
        };

        // Choose first centroid randomly
        let first_idx = (next_random() * n as f32) as usize % n;
        for j in 0..d {
            centroids[[0, j]] = data[[first_idx, j]];
        }

        // Choose remaining centroids with probability proportional to D^2
        for i in 1..self.k {
            // Compute distances to nearest existing centroid
            let mut distances = Vec::with_capacity(n);
            let mut total_dist = 0.0f32;

            for point_idx in 0..n {
                let point = data.row(point_idx);
                let mut min_dist = f32::MAX;

                for j in 0..i {
                    let dist = self.euclidean_distance(point, centroids.row(j));
                    min_dist = min_dist.min(dist);
                }

                let dist_sq = min_dist * min_dist;
                distances.push(dist_sq);
                total_dist += dist_sq;
            }

            // Sample proportionally to D^2
            let target = next_random() * total_dist;
            let mut cumsum = 0.0f32;
            let mut chosen_idx = 0;

            for (idx, &dist) in distances.iter().enumerate() {
                cumsum += dist;
                if cumsum >= target {
                    chosen_idx = idx;
                    break;
                }
            }

            for j in 0..d {
                centroids[[i, j]] = data[[chosen_idx, j]];
            }
        }

        centroids
    }

    /// Compute Euclidean distance between two vectors.
    fn euclidean_distance(&self, a: ArrayView1<f32>, b: ArrayView1<f32>) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }

    /// Predict cluster labels for new data given fitted centroids.
    pub fn predict(&self, data: &Array2<f32>, centroids: &Array2<f32>) -> Vec<usize> {
        let n = data.nrows();
        let mut labels = vec![0usize; n];

        for i in 0..n {
            let point = data.row(i);
            let mut min_dist = f32::MAX;
            let mut best_cluster = 0;

            for (j, centroid) in centroids.outer_iter().enumerate() {
                let dist = self.euclidean_distance(point, centroid);
                if dist < min_dist {
                    min_dist = dist;
                    best_cluster = j;
                }
            }

            labels[i] = best_cluster;
        }

        labels
    }

    /// Compute inertia (within-cluster sum of squares).
    pub fn compute_inertia(
        &self,
        data: &Array2<f32>,
        labels: &[usize],
        centroids: &Array2<f32>,
    ) -> f32 {
        labels
            .iter()
            .enumerate()
            .map(|(i, &label)| {
                self.euclidean_distance(data.row(i), centroids.row(label)).powi(2)
            })
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::Array1;

    fn create_test_data() -> Array2<f32> {
        // Create simple separable clusters
        let mut data = Array2::<f32>::zeros((12, 2));

        // Cluster 0: points near (0, 0)
        data[[0, 0]] = 0.0;
        data[[0, 1]] = 0.0;
        data[[1, 0]] = 0.1;
        data[[1, 1]] = 0.1;
        data[[2, 0]] = -0.1;
        data[[2, 1]] = 0.1;
        data[[3, 0]] = 0.0;
        data[[3, 1]] = -0.1;

        // Cluster 1: points near (5, 5)
        data[[4, 0]] = 5.0;
        data[[4, 1]] = 5.0;
        data[[5, 0]] = 5.1;
        data[[5, 1]] = 5.1;
        data[[6, 0]] = 4.9;
        data[[6, 1]] = 5.0;
        data[[7, 0]] = 5.0;
        data[[7, 1]] = 4.9;

        // Cluster 2: points near (10, 0)
        data[[8, 0]] = 10.0;
        data[[8, 1]] = 0.0;
        data[[9, 0]] = 10.1;
        data[[9, 1]] = 0.1;
        data[[10, 0]] = 9.9;
        data[[10, 1]] = 0.0;
        data[[11, 0]] = 10.0;
        data[[11, 1]] = -0.1;

        data
    }

    #[test]
    fn test_kmeans_basic() {
        let clusterer = KMeansClusterer::new(3, Some(42));
        let data = create_test_data();

        let (labels, centroids) = clusterer.fit(&data).unwrap();

        assert_eq!(labels.len(), 12);
        assert_eq!(centroids.nrows(), 3);

        // Check that points in same original cluster have same label
        // (with high probability given clear separation)
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[0], labels[2]);
        assert_eq!(labels[0], labels[3]);

        assert_eq!(labels[4], labels[5]);
        assert_eq!(labels[4], labels[6]);
        assert_eq!(labels[4], labels[7]);

        assert_eq!(labels[8], labels[9]);
        assert_eq!(labels[8], labels[10]);
        assert_eq!(labels[8], labels[11]);
    }

    #[test]
    fn test_kmeans_insufficient_data() {
        let clusterer = KMeansClusterer::new(10, None);
        let data = Array2::<f32>::zeros((5, 2));

        let result = clusterer.fit(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_kmeans_predict() {
        let clusterer = KMeansClusterer::new(2, Some(42));

        let train_data = Array2::from_shape_vec(
            (4, 2),
            vec![0.0, 0.0, 0.1, 0.1, 5.0, 5.0, 5.1, 5.1],
        )
        .unwrap();

        let (_, centroids) = clusterer.fit(&train_data).unwrap();

        let test_data = Array2::from_shape_vec(
            (2, 2),
            vec![0.05, 0.05, 4.95, 4.95],
        )
        .unwrap();

        let predictions = clusterer.predict(&test_data, &centroids);
        assert_eq!(predictions.len(), 2);

        // First point should be in same cluster as (0,0) points
        // Second point should be in same cluster as (5,5) points
        assert_ne!(predictions[0], predictions[1]);
    }

    #[test]
    fn test_euclidean_distance() {
        let clusterer = KMeansClusterer::new(2, None);
        let a = Array1::from_vec(vec![0.0, 0.0]);
        let b = Array1::from_vec(vec![3.0, 4.0]);

        let dist = clusterer.euclidean_distance(a.view(), b.view());
        assert!((dist - 5.0).abs() < 0.001);
    }
}
