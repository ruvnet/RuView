//! IVFFlat (Inverted File with Flat quantization) index implementation
//!
//! Provides approximate nearest neighbor search by partitioning vectors into clusters.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use dashmap::DashMap;
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::distance::{distance, DistanceMetric};

/// IVFFlat configuration
#[derive(Debug, Clone)]
pub struct IvfFlatConfig {
    /// Number of clusters (lists)
    pub lists: usize,
    /// Number of lists to probe during search
    pub probes: usize,
    /// Distance metric
    pub metric: DistanceMetric,
    /// K-means iterations for training
    pub kmeans_iterations: usize,
    /// Random seed for reproducibility
    pub seed: u64,
}

impl Default for IvfFlatConfig {
    fn default() -> Self {
        Self {
            lists: 100,
            probes: 1,
            metric: DistanceMetric::Euclidean,
            kmeans_iterations: 10,
            seed: 42,
        }
    }
}

/// Vector ID type
pub type VectorId = u64;

/// Entry in a cluster
#[derive(Debug, Clone)]
struct ClusterEntry {
    id: VectorId,
    vector: Vec<f32>,
}

/// Search result with distance
#[derive(Debug, Clone, Copy)]
struct SearchResult {
    id: VectorId,
    distance: f32,
}

impl PartialEq for SearchResult {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for SearchResult {}

impl PartialOrd for SearchResult {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SearchResult {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for max-heap
        other
            .distance
            .partial_cmp(&self.distance)
            .unwrap_or(Ordering::Equal)
    }
}

/// IVFFlat Index
pub struct IvfFlatIndex {
    /// Configuration
    config: IvfFlatConfig,
    /// Cluster centroids
    centroids: RwLock<Vec<Vec<f32>>>,
    /// Inverted lists (cluster_id -> vectors)
    lists: DashMap<usize, Vec<ClusterEntry>>,
    /// Vector ID to cluster mapping
    id_to_cluster: DashMap<VectorId, usize>,
    /// Next vector ID
    next_id: std::sync::atomic::AtomicU64,
    /// Total vector count
    vector_count: std::sync::atomic::AtomicUsize,
    /// Dimensions
    dimensions: usize,
    /// Whether the index has been trained
    trained: std::sync::atomic::AtomicBool,
}

impl IvfFlatIndex {
    /// Create a new IVFFlat index
    pub fn new(dimensions: usize, config: IvfFlatConfig) -> Self {
        Self {
            config,
            centroids: RwLock::new(Vec::new()),
            lists: DashMap::new(),
            id_to_cluster: DashMap::new(),
            next_id: std::sync::atomic::AtomicU64::new(0),
            vector_count: std::sync::atomic::AtomicUsize::new(0),
            dimensions,
            trained: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// Number of vectors in the index
    pub fn len(&self) -> usize {
        self.vector_count.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if index is trained
    pub fn is_trained(&self) -> bool {
        self.trained.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Calculate distance between vectors
    fn calc_distance(&self, a: &[f32], b: &[f32]) -> f32 {
        distance(a, b, self.config.metric)
    }

    /// Train the index on a sample of vectors
    pub fn train(&self, training_vectors: &[Vec<f32>]) {
        if training_vectors.is_empty() {
            return;
        }

        let n_clusters = self.config.lists.min(training_vectors.len());

        // Initialize centroids using k-means++
        let mut centroids = self.kmeans_plus_plus_init(training_vectors, n_clusters);

        // K-means iterations
        for _ in 0..self.config.kmeans_iterations {
            // Assign vectors to clusters
            let mut cluster_sums: Vec<Vec<f32>> = (0..n_clusters)
                .map(|_| vec![0.0; self.dimensions])
                .collect();
            let mut cluster_counts: Vec<usize> = vec![0; n_clusters];

            for vector in training_vectors {
                let cluster = self.find_nearest_centroid(vector, &centroids);
                for (i, &v) in vector.iter().enumerate() {
                    cluster_sums[cluster][i] += v;
                }
                cluster_counts[cluster] += 1;
            }

            // Update centroids
            for (i, centroid) in centroids.iter_mut().enumerate() {
                if cluster_counts[i] > 0 {
                    for j in 0..self.dimensions {
                        centroid[j] = cluster_sums[i][j] / cluster_counts[i] as f32;
                    }
                }
            }
        }

        *self.centroids.write() = centroids;

        // Initialize empty lists
        for i in 0..n_clusters {
            self.lists.insert(i, Vec::new());
        }

        self.trained
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// K-means++ initialization
    fn kmeans_plus_plus_init(&self, vectors: &[Vec<f32>], k: usize) -> Vec<Vec<f32>> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed);
        let mut centroids = Vec::with_capacity(k);

        // Choose first centroid randomly
        let first_idx = rng.gen_range(0..vectors.len());
        centroids.push(vectors[first_idx].clone());

        // Choose remaining centroids
        for _ in 1..k {
            let mut distances: Vec<f32> = vectors
                .iter()
                .map(|v| {
                    centroids
                        .iter()
                        .map(|c| self.calc_distance(v, c))
                        .fold(f32::MAX, f32::min)
                })
                .collect();

            // Square distances for probability weighting
            for d in &mut distances {
                *d = *d * *d;
            }

            let total: f32 = distances.iter().sum();
            if total == 0.0 {
                break;
            }

            // Roulette wheel selection
            let target = rng.gen_range(0.0..total);
            let mut cumsum = 0.0;
            let mut selected = 0;
            for (i, d) in distances.iter().enumerate() {
                cumsum += d;
                if cumsum >= target {
                    selected = i;
                    break;
                }
            }

            centroids.push(vectors[selected].clone());
        }

        centroids
    }

    /// Find nearest centroid to a vector
    fn find_nearest_centroid(&self, vector: &[f32], centroids: &[Vec<f32>]) -> usize {
        let mut best_cluster = 0;
        let mut best_dist = f32::MAX;

        for (i, centroid) in centroids.iter().enumerate() {
            let dist = self.calc_distance(vector, centroid);
            if dist < best_dist {
                best_dist = dist;
                best_cluster = i;
            }
        }

        best_cluster
    }

    /// Insert a vector into the index
    pub fn insert(&self, vector: Vec<f32>) -> VectorId {
        assert_eq!(vector.len(), self.dimensions, "Vector dimension mismatch");
        assert!(self.is_trained(), "Index must be trained before insertion");

        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let centroids = self.centroids.read();
        let cluster = self.find_nearest_centroid(&vector, &centroids);
        drop(centroids);

        let entry = ClusterEntry { id, vector };

        if let Some(mut list) = self.lists.get_mut(&cluster) {
            list.push(entry);
        }

        self.id_to_cluster.insert(id, cluster);
        self.vector_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        id
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize, probes: Option<usize>) -> Vec<(VectorId, f32)> {
        assert_eq!(query.len(), self.dimensions, "Query dimension mismatch");

        if !self.is_trained() {
            return Vec::new();
        }

        let n_probes = probes.unwrap_or(self.config.probes);
        let centroids = self.centroids.read();

        // Find nearest centroids
        let mut centroid_dists: Vec<(usize, f32)> = centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.calc_distance(query, c)))
            .collect();

        centroid_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        drop(centroids);

        // Search in top probes clusters
        let mut heap = BinaryHeap::new();

        for (cluster_id, _) in centroid_dists.iter().take(n_probes) {
            if let Some(list) = self.lists.get(cluster_id) {
                for entry in list.iter() {
                    let dist = self.calc_distance(query, &entry.vector);
                    heap.push(SearchResult {
                        id: entry.id,
                        distance: dist,
                    });

                    if heap.len() > k {
                        heap.pop();
                    }
                }
            }
        }

        // Convert to sorted results
        let mut results: Vec<_> = heap.into_iter().map(|r| (r.id, r.distance)).collect();
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        results
    }

    /// Parallel search
    pub fn search_parallel(
        &self,
        query: &[f32],
        k: usize,
        probes: Option<usize>,
    ) -> Vec<(VectorId, f32)> {
        assert_eq!(query.len(), self.dimensions, "Query dimension mismatch");

        if !self.is_trained() {
            return Vec::new();
        }

        let n_probes = probes.unwrap_or(self.config.probes);
        let centroids = self.centroids.read();

        // Find nearest centroids
        let mut centroid_dists: Vec<(usize, f32)> = centroids
            .iter()
            .enumerate()
            .map(|(i, c)| (i, self.calc_distance(query, c)))
            .collect();

        centroid_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        drop(centroids);

        // Get cluster IDs to probe
        let probe_clusters: Vec<usize> = centroid_dists
            .iter()
            .take(n_probes)
            .map(|(id, _)| *id)
            .collect();

        // Parallel search across clusters
        let results: Vec<(VectorId, f32)> = probe_clusters
            .par_iter()
            .flat_map(|cluster_id| {
                let mut local_results = Vec::new();
                if let Some(list) = self.lists.get(cluster_id) {
                    for entry in list.iter() {
                        let dist = self.calc_distance(query, &entry.vector);
                        local_results.push((entry.id, dist));
                    }
                }
                local_results
            })
            .collect();

        // Merge and get top k
        let mut heap = BinaryHeap::new();
        for (id, dist) in results {
            heap.push(SearchResult { id, distance: dist });
            if heap.len() > k {
                heap.pop();
            }
        }

        let mut final_results: Vec<_> = heap.into_iter().map(|r| (r.id, r.distance)).collect();
        final_results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        final_results
    }

    /// Get vector by ID
    pub fn get_vector(&self, id: VectorId) -> Option<Vec<f32>> {
        if let Some(cluster) = self.id_to_cluster.get(&id) {
            if let Some(list) = self.lists.get(&*cluster) {
                for entry in list.iter() {
                    if entry.id == id {
                        return Some(entry.vector.clone());
                    }
                }
            }
        }
        None
    }

    /// Get approximate memory usage in bytes
    pub fn memory_usage(&self) -> usize {
        let vector_bytes = self.len() * self.dimensions * 4;
        let centroid_bytes = self.config.lists * self.dimensions * 4;
        vector_bytes + centroid_bytes
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_random_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        (0..n)
            .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
            .collect()
    }

    #[test]
    fn test_train_and_search() {
        let config = IvfFlatConfig {
            lists: 10,
            probes: 3,
            metric: DistanceMetric::Euclidean,
            kmeans_iterations: 5,
            seed: 42,
        };

        let index = IvfFlatIndex::new(16, config);

        // Generate training data
        let training = generate_random_vectors(100, 16, 42);
        index.train(&training);

        assert!(index.is_trained());

        // Insert vectors
        for v in training.iter() {
            index.insert(v.clone());
        }

        assert_eq!(index.len(), 100);

        // Search
        let query = generate_random_vectors(1, 16, 123)[0].clone();
        let results = index.search(&query, 10, None);

        assert_eq!(results.len(), 10);
    }

    #[test]
    fn test_empty_index() {
        let index = IvfFlatIndex::new(8, IvfFlatConfig::default());
        assert!(index.is_empty());
        assert!(!index.is_trained());

        let results = index.search(&[0.0; 8], 10, None);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parallel_search() {
        let config = IvfFlatConfig {
            lists: 20,
            probes: 5,
            metric: DistanceMetric::Euclidean,
            kmeans_iterations: 5,
            seed: 42,
        };

        let index = IvfFlatIndex::new(32, config);

        let training = generate_random_vectors(500, 32, 42);
        index.train(&training);

        for v in training.iter() {
            index.insert(v.clone());
        }

        let query = generate_random_vectors(1, 32, 999)[0].clone();

        let serial = index.search(&query, 10, None);
        let parallel = index.search_parallel(&query, 10, None);

        // Results should be the same
        assert_eq!(serial.len(), parallel.len());
    }
}
