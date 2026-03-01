//! Benchmark Utilities for 7sense Performance Testing
//!
//! This module provides common utilities for benchmarking:
//! - Random vector generation
//! - Test index setup
//! - Recall calculation
//! - Ground truth computation
//! - Performance metrics

use std::collections::HashSet;
use std::time::{Duration, Instant};

/// Embedding dimensions for Perch 2.0 model
pub const PERCH_EMBEDDING_DIM: usize = 1536;

/// Default HNSW parameters from ADR-004
pub const DEFAULT_M: usize = 32;
pub const DEFAULT_EF_CONSTRUCTION: usize = 200;
pub const DEFAULT_EF_SEARCH: usize = 128;
pub const HIGH_RECALL_EF_SEARCH: usize = 256;

/// Performance targets from ADR-004
pub mod targets {
    use std::time::Duration;

    /// HNSW Search Targets
    pub const HNSW_SPEEDUP_VS_BRUTE_FORCE: f64 = 150.0;
    pub const QUERY_LATENCY_P50_MS: u64 = 10;
    pub const QUERY_LATENCY_P99_MS: u64 = 50;
    pub const RECALL_AT_10: f64 = 0.95;
    pub const RECALL_AT_100: f64 = 0.98;

    /// Embedding Inference Targets
    pub const EMBEDDING_SEGMENTS_PER_SECOND: u64 = 100;

    /// Batch Ingestion Targets
    pub const BATCH_VECTORS_PER_MINUTE: u64 = 1_000_000;
    pub const INSERT_THROUGHPUT_PER_SECOND: u64 = 10_000;

    /// Query Latency Targets
    pub const TOTAL_QUERY_LATENCY_MS: u64 = 100;

    /// Build Time Targets
    pub const BUILD_TIME_1M_VECTORS: Duration = Duration::from_secs(30 * 60);

    /// Quantization Targets
    pub const MAX_RECALL_LOSS_INT8: f64 = 0.03;
}

/// Generate random f32 vectors for benchmarking
///
/// # Arguments
/// * `count` - Number of vectors to generate
/// * `dims` - Dimensionality of each vector
///
/// # Returns
/// A vector of random f32 vectors, normalized to unit length
pub fn generate_random_vectors(count: usize, dims: usize) -> Vec<Vec<f32>> {
    

    let mut vectors = Vec::with_capacity(count);

    for i in 0..count {
        let mut vec = Vec::with_capacity(dims);

        // Use a simple deterministic random generator for reproducibility
        let mut seed = (i as u64).wrapping_mul(6364136223846793005).wrapping_add(1);

        for _ in 0..dims {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let val = ((seed >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            vec.push(val);
        }

        // Normalize to unit length
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in vec.iter_mut() {
                *x /= norm;
            }
        }

        vectors.push(vec);
    }

    vectors
}

/// Generate clustered random vectors for more realistic benchmarking
///
/// # Arguments
/// * `count` - Total number of vectors to generate
/// * `dims` - Dimensionality of each vector
/// * `num_clusters` - Number of clusters to create
/// * `cluster_spread` - Standard deviation within clusters (0.0 to 1.0)
///
/// # Returns
/// A vector of random f32 vectors organized around cluster centers
pub fn generate_clustered_vectors(
    count: usize,
    dims: usize,
    num_clusters: usize,
    cluster_spread: f32,
) -> Vec<Vec<f32>> {
    let mut vectors = Vec::with_capacity(count);

    // Generate cluster centers
    let centers = generate_random_vectors(num_clusters, dims);

    // Assign vectors to clusters
    for i in 0..count {
        let cluster_idx = i % num_clusters;
        let center = &centers[cluster_idx];

        let mut vec = Vec::with_capacity(dims);

        // Use deterministic random for offset
        let mut seed = (i as u64).wrapping_mul(2862933555777941757).wrapping_add(3);

        for d in 0..dims {
            seed = seed.wrapping_mul(2862933555777941757).wrapping_add(3);
            let noise = ((seed >> 33) as f32) / (u32::MAX as f32) * 2.0 - 1.0;
            let val = center[d] + noise * cluster_spread;
            vec.push(val);
        }

        // Normalize to unit length
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for x in vec.iter_mut() {
                *x /= norm;
            }
        }

        vectors.push(vec);
    }

    vectors
}

/// Compute L2 (Euclidean) distance between two vectors
#[inline]
pub fn l2_distance(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

/// Compute L2 squared distance (faster, no sqrt)
#[inline]
pub fn l2_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum()
}

/// Compute cosine similarity between two vectors
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a > 0.0 && norm_b > 0.0 {
        dot / (norm_a * norm_b)
    } else {
        0.0
    }
}

/// Compute brute-force k-nearest neighbors (ground truth)
///
/// # Arguments
/// * `query` - Query vector
/// * `dataset` - Dataset of vectors to search
/// * `k` - Number of neighbors to find
///
/// # Returns
/// Vector of (index, distance) pairs sorted by distance
pub fn brute_force_knn(query: &[f32], dataset: &[Vec<f32>], k: usize) -> Vec<(usize, f32)> {
    let mut distances: Vec<(usize, f32)> = dataset
        .iter()
        .enumerate()
        .map(|(i, vec)| (i, l2_distance(query, vec)))
        .collect();

    distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    distances.truncate(k);
    distances
}

/// Measure recall@k for approximate nearest neighbor results
///
/// # Arguments
/// * `results` - Approximate results (index, distance) pairs
/// * `ground_truth` - Exact brute-force results (index, distance) pairs
/// * `k` - Number of top results to consider
///
/// # Returns
/// Recall value between 0.0 and 1.0
pub fn measure_recall_at_k(
    results: &[(usize, f32)],
    ground_truth: &[(usize, f32)],
    k: usize,
) -> f32 {
    let k = k.min(results.len()).min(ground_truth.len());
    if k == 0 {
        return 0.0;
    }

    let result_set: HashSet<usize> = results.iter().take(k).map(|(idx, _)| *idx).collect();
    let truth_set: HashSet<usize> = ground_truth.iter().take(k).map(|(idx, _)| *idx).collect();

    let intersection = result_set.intersection(&truth_set).count();
    intersection as f32 / k as f32
}

/// Calculate percentile from a sorted slice of durations
pub fn percentile(sorted_latencies: &[Duration], p: f64) -> Duration {
    if sorted_latencies.is_empty() {
        return Duration::ZERO;
    }
    let idx = ((sorted_latencies.len() as f64 - 1.0) * p / 100.0).round() as usize;
    sorted_latencies[idx.min(sorted_latencies.len() - 1)]
}

/// Performance statistics from benchmark runs
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    pub count: usize,
    pub total_time: Duration,
    pub min: Duration,
    pub max: Duration,
    pub mean: Duration,
    pub p50: Duration,
    pub p95: Duration,
    pub p99: Duration,
    pub p999: Duration,
    pub throughput_per_sec: f64,
}

impl PerformanceStats {
    /// Calculate statistics from a collection of latency measurements
    pub fn from_latencies(mut latencies: Vec<Duration>) -> Self {
        if latencies.is_empty() {
            return Self {
                count: 0,
                total_time: Duration::ZERO,
                min: Duration::ZERO,
                max: Duration::ZERO,
                mean: Duration::ZERO,
                p50: Duration::ZERO,
                p95: Duration::ZERO,
                p99: Duration::ZERO,
                p999: Duration::ZERO,
                throughput_per_sec: 0.0,
            };
        }

        latencies.sort();

        let total_time: Duration = latencies.iter().sum();
        let count = latencies.len();
        let mean = total_time / count as u32;

        Self {
            count,
            total_time,
            min: latencies[0],
            max: latencies[count - 1],
            mean,
            p50: percentile(&latencies, 50.0),
            p95: percentile(&latencies, 95.0),
            p99: percentile(&latencies, 99.0),
            p999: percentile(&latencies, 99.9),
            throughput_per_sec: count as f64 / total_time.as_secs_f64(),
        }
    }

    /// Check if stats meet p99 latency target
    pub fn meets_p99_target(&self, target_ms: u64) -> bool {
        self.p99 <= Duration::from_millis(target_ms)
    }

    /// Check if stats meet throughput target
    pub fn meets_throughput_target(&self, target_per_sec: u64) -> bool {
        self.throughput_per_sec >= target_per_sec as f64
    }

    /// Format as a readable report
    pub fn report(&self) -> String {
        format!(
            "Count: {}\n\
             Total Time: {:?}\n\
             Min: {:?}\n\
             Max: {:?}\n\
             Mean: {:?}\n\
             P50: {:?}\n\
             P95: {:?}\n\
             P99: {:?}\n\
             P99.9: {:?}\n\
             Throughput: {:.2} ops/sec",
            self.count,
            self.total_time,
            self.min,
            self.max,
            self.mean,
            self.p50,
            self.p95,
            self.p99,
            self.p999,
            self.throughput_per_sec
        )
    }
}

/// Simple HNSW-like index for benchmarking
/// This is a simplified implementation for benchmark purposes
pub struct SimpleHnswIndex {
    vectors: Vec<Vec<f32>>,
    dims: usize,
    m: usize,
    ef_construction: usize,
    ef_search: usize,
    // Simplified graph structure: each vector has a list of neighbor indices
    graph: Vec<Vec<usize>>,
}

impl SimpleHnswIndex {
    /// Create a new empty index
    pub fn new(dims: usize, m: usize, ef_construction: usize, ef_search: usize) -> Self {
        Self {
            vectors: Vec::new(),
            dims,
            m,
            ef_construction,
            ef_search,
            graph: Vec::new(),
        }
    }

    /// Create an index with default parameters for Perch embeddings
    pub fn new_default() -> Self {
        Self::new(
            PERCH_EMBEDDING_DIM,
            DEFAULT_M,
            DEFAULT_EF_CONSTRUCTION,
            DEFAULT_EF_SEARCH,
        )
    }

    /// Get the number of vectors in the index
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }

    /// Add a single vector to the index
    pub fn add(&mut self, vector: Vec<f32>) -> usize {
        assert_eq!(vector.len(), self.dims);
        let id = self.vectors.len();

        // Find neighbors for the new vector
        let neighbors = if self.vectors.is_empty() {
            Vec::new()
        } else {
            self.search_internal(&vector, self.m.min(self.vectors.len()))
                .into_iter()
                .map(|(idx, _)| idx)
                .collect()
        };

        self.vectors.push(vector);
        self.graph.push(neighbors.clone());

        // Update bidirectional connections
        for &neighbor_id in &neighbors {
            if self.graph[neighbor_id].len() < self.m * 2 {
                self.graph[neighbor_id].push(id);
            }
        }

        id
    }

    /// Batch add vectors to the index
    pub fn batch_add(&mut self, vectors: Vec<Vec<f32>>) -> Vec<usize> {
        vectors.into_iter().map(|v| self.add(v)).collect()
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        assert_eq!(query.len(), self.dims);
        if self.vectors.is_empty() {
            return Vec::new();
        }
        self.search_internal(query, k)
    }

    /// Internal search implementation with simplified HNSW-like traversal
    fn search_internal(&self, query: &[f32], k: usize) -> Vec<(usize, f32)> {
        use std::collections::{BinaryHeap, HashSet};
        use std::cmp::Reverse;

        let ef = self.ef_search.max(k);

        // Start from a random entry point
        let entry_point = 0;

        let mut visited: HashSet<usize> = HashSet::new();
        let mut candidates: BinaryHeap<Reverse<(ordered_float::OrderedFloat<f32>, usize)>> =
            BinaryHeap::new();
        let mut results: BinaryHeap<(ordered_float::OrderedFloat<f32>, usize)> = BinaryHeap::new();

        let entry_dist = l2_distance(query, &self.vectors[entry_point]);
        candidates.push(Reverse((ordered_float::OrderedFloat(entry_dist), entry_point)));
        results.push((ordered_float::OrderedFloat(entry_dist), entry_point));
        visited.insert(entry_point);

        while let Some(Reverse((dist, current))) = candidates.pop() {
            let worst_dist = if results.len() >= ef {
                results.peek().map(|(d, _)| d.0).unwrap_or(f32::MAX)
            } else {
                f32::MAX
            };

            if dist.0 > worst_dist {
                break;
            }

            // Explore neighbors
            for &neighbor in &self.graph[current] {
                if visited.insert(neighbor) {
                    let neighbor_dist = l2_distance(query, &self.vectors[neighbor]);

                    if results.len() < ef || neighbor_dist < worst_dist {
                        candidates.push(Reverse((
                            ordered_float::OrderedFloat(neighbor_dist),
                            neighbor,
                        )));
                        results.push((ordered_float::OrderedFloat(neighbor_dist), neighbor));

                        if results.len() > ef {
                            results.pop();
                        }
                    }
                }
            }
        }

        // Convert to output format and sort by distance
        let mut output: Vec<(usize, f32)> =
            results.into_iter().map(|(d, idx)| (idx, d.0)).collect();
        output.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        output.truncate(k);
        output
    }

    /// Set ef_search parameter for queries
    pub fn set_ef_search(&mut self, ef: usize) {
        self.ef_search = ef;
    }
}

/// Setup a test index with the specified number of vectors
pub fn setup_test_index(size: usize) -> SimpleHnswIndex {
    let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);
    let mut index = SimpleHnswIndex::new_default();

    for vec in vectors {
        index.add(vec);
    }

    index
}

/// Scalar quantizer for int8 compression
pub struct ScalarQuantizer {
    mins: Vec<f32>,
    maxs: Vec<f32>,
    scales: Vec<f32>,
    dims: usize,
}

impl ScalarQuantizer {
    /// Create a new quantizer for the specified dimensions
    pub fn new(dims: usize) -> Self {
        Self {
            mins: vec![f32::MAX; dims],
            maxs: vec![f32::MIN; dims],
            scales: vec![1.0; dims],
            dims,
        }
    }

    /// Calibrate the quantizer from a sample of embeddings
    pub fn calibrate(&mut self, embeddings: &[Vec<f32>]) {
        // Find min/max per dimension
        for embedding in embeddings {
            for (d, &val) in embedding.iter().enumerate() {
                if val < self.mins[d] {
                    self.mins[d] = val;
                }
                if val > self.maxs[d] {
                    self.maxs[d] = val;
                }
            }
        }

        // Compute scales
        for d in 0..self.dims {
            let range = self.maxs[d] - self.mins[d];
            if range > 0.0 {
                self.scales[d] = 255.0 / range;
            } else {
                self.scales[d] = 1.0;
            }
        }
    }

    /// Quantize a float32 embedding to int8
    pub fn quantize(&self, embedding: &[f32]) -> Vec<u8> {
        embedding
            .iter()
            .enumerate()
            .map(|(d, &val)| {
                let normalized = (val - self.mins[d]) * self.scales[d];
                normalized.round().clamp(0.0, 255.0) as u8
            })
            .collect()
    }

    /// Dequantize an int8 embedding back to float32
    pub fn dequantize(&self, quantized: &[u8]) -> Vec<f32> {
        quantized
            .iter()
            .enumerate()
            .map(|(d, &val)| (val as f32) / self.scales[d] + self.mins[d])
            .collect()
    }
}

/// Timer utility for measuring operations
pub struct Timer {
    start: Instant,
}

impl Timer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Get elapsed time
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Stop and return elapsed time
    pub fn stop(self) -> Duration {
        self.start.elapsed()
    }
}

/// Measure execution time of a closure
pub fn measure_time<F, R>(f: F) -> (R, Duration)
where
    F: FnOnce() -> R,
{
    let start = Instant::now();
    let result = f();
    let duration = start.elapsed();
    (result, duration)
}

/// Measure average execution time over multiple iterations
pub fn measure_average<F>(iterations: usize, mut f: F) -> PerformanceStats
where
    F: FnMut() -> (),
{
    let latencies: Vec<Duration> = (0..iterations)
        .map(|_| {
            let start = Instant::now();
            f();
            start.elapsed()
        })
        .collect();

    PerformanceStats::from_latencies(latencies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_random_vectors() {
        let vectors = generate_random_vectors(100, 1536);
        assert_eq!(vectors.len(), 100);
        assert_eq!(vectors[0].len(), 1536);

        // Check normalization
        for vec in &vectors {
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_l2_distance() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let dist = l2_distance(&a, &b);
        assert!((dist - std::f32::consts::SQRT_2).abs() < 1e-5);
    }

    #[test]
    fn test_recall_at_k() {
        let results: Vec<(usize, f32)> = vec![(0, 0.1), (1, 0.2), (2, 0.3), (3, 0.4), (5, 0.5)];
        let ground_truth: Vec<(usize, f32)> =
            vec![(0, 0.1), (1, 0.2), (2, 0.3), (4, 0.4), (5, 0.5)];

        let recall = measure_recall_at_k(&results, &ground_truth, 5);
        assert!((recall - 0.8).abs() < 1e-5); // 4 out of 5 match
    }

    #[test]
    fn test_scalar_quantizer() {
        let vectors = generate_random_vectors(100, 128);
        let mut quantizer = ScalarQuantizer::new(128);
        quantizer.calibrate(&vectors);

        for vec in &vectors {
            let quantized = quantizer.quantize(vec);
            let dequantized = quantizer.dequantize(&quantized);

            // Check that dequantized is close to original
            let error: f32 = vec
                .iter()
                .zip(dequantized.iter())
                .map(|(a, b)| (a - b).abs())
                .sum::<f32>()
                / vec.len() as f32;

            assert!(error < 0.1); // Average error should be small
        }
    }

    #[test]
    fn test_performance_stats() {
        let latencies: Vec<Duration> = (0..100)
            .map(|i| Duration::from_micros(100 + i * 10))
            .collect();

        let stats = PerformanceStats::from_latencies(latencies);
        assert_eq!(stats.count, 100);
        assert!(stats.min <= stats.p50);
        assert!(stats.p50 <= stats.p95);
        assert!(stats.p95 <= stats.p99);
        assert!(stats.p99 <= stats.max);
    }
}
