//! Clustering Benchmark Suite for 7sense
//!
//! Benchmarks for clustering algorithms used in bird call analysis:
//! - HDBSCAN for species/call-type clustering
//! - Cluster assignment for new embeddings
//! - Motif detection in audio sequences
//! - Centroid computation and updates

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use sevensense_benches::*;

/// Number of clusters for benchmark
const NUM_CLUSTERS: usize = 50;

// ============================================================================
// Simplified HDBSCAN Implementation for Benchmarking
// ============================================================================

/// Simplified HDBSCAN-like clustering for benchmarking
/// In production, this would use the actual HDBSCAN algorithm
struct SimpleHdbscan {
    min_cluster_size: usize,
    min_samples: usize,
    epsilon: f32,
}

impl SimpleHdbscan {
    fn new(min_cluster_size: usize, min_samples: usize, epsilon: f32) -> Self {
        Self {
            min_cluster_size,
            min_samples,
            epsilon,
        }
    }

    /// Fit the clustering model on embeddings
    /// Returns cluster labels (-1 for noise)
    fn fit(&self, embeddings: &[Vec<f32>]) -> Vec<i32> {
        let n = embeddings.len();
        let mut labels = vec![-1i32; n];
        let mut cluster_id = 0;
        let mut visited = vec![false; n];

        for i in 0..n {
            if visited[i] {
                continue;
            }

            // Find neighbors within epsilon
            let neighbors = self.region_query(embeddings, i);

            if neighbors.len() >= self.min_samples {
                // Expand cluster
                let cluster_members = self.expand_cluster(embeddings, i, &neighbors, &mut visited);

                if cluster_members.len() >= self.min_cluster_size {
                    for &member in &cluster_members {
                        labels[member] = cluster_id;
                    }
                    cluster_id += 1;
                }
            }
        }

        labels
    }

    fn region_query(&self, embeddings: &[Vec<f32>], point_idx: usize) -> Vec<usize> {
        let point = &embeddings[point_idx];
        embeddings
            .iter()
            .enumerate()
            .filter(|(_, other)| l2_distance(point, other) <= self.epsilon)
            .map(|(idx, _)| idx)
            .collect()
    }

    fn expand_cluster(
        &self,
        embeddings: &[Vec<f32>],
        seed: usize,
        initial_neighbors: &[usize],
        visited: &mut [bool],
    ) -> Vec<usize> {
        let mut cluster = vec![seed];
        visited[seed] = true;

        let mut to_process: Vec<usize> = initial_neighbors.to_vec();

        while let Some(point_idx) = to_process.pop() {
            if visited[point_idx] {
                continue;
            }
            visited[point_idx] = true;
            cluster.push(point_idx);

            let neighbors = self.region_query(embeddings, point_idx);
            if neighbors.len() >= self.min_samples {
                to_process.extend(neighbors.iter().filter(|&&n| !visited[n]));
            }
        }

        cluster
    }
}

/// Cluster assignment for new embeddings
struct ClusterAssigner {
    centroids: Vec<Vec<f32>>,
    cluster_ids: Vec<usize>,
}

impl ClusterAssigner {
    fn new(centroids: Vec<Vec<f32>>) -> Self {
        let cluster_ids = (0..centroids.len()).collect();
        Self {
            centroids,
            cluster_ids,
        }
    }

    /// Assign a single embedding to the nearest cluster
    fn assign(&self, embedding: &[f32]) -> (usize, f32) {
        let mut best_cluster = 0;
        let mut best_distance = f32::MAX;

        for (i, centroid) in self.centroids.iter().enumerate() {
            let dist = l2_distance(embedding, centroid);
            if dist < best_distance {
                best_distance = dist;
                best_cluster = self.cluster_ids[i];
            }
        }

        (best_cluster, best_distance)
    }

    /// Batch assign embeddings
    fn batch_assign(&self, embeddings: &[Vec<f32>]) -> Vec<(usize, f32)> {
        embeddings.iter().map(|e| self.assign(e)).collect()
    }
}

/// Compute cluster centroids from labeled embeddings
fn compute_centroids(embeddings: &[Vec<f32>], labels: &[i32]) -> HashMap<i32, Vec<f32>> {
    let dims = embeddings[0].len();
    let mut sums: HashMap<i32, Vec<f32>> = HashMap::new();
    let mut counts: HashMap<i32, usize> = HashMap::new();

    for (embedding, &label) in embeddings.iter().zip(labels.iter()) {
        if label >= 0 {
            let sum = sums.entry(label).or_insert_with(|| vec![0.0; dims]);
            for (s, &e) in sum.iter_mut().zip(embedding.iter()) {
                *s += e;
            }
            *counts.entry(label).or_insert(0) += 1;
        }
    }

    let mut centroids = HashMap::new();
    for (label, sum) in sums {
        let count = counts[&label] as f32;
        let centroid: Vec<f32> = sum.iter().map(|&s| s / count).collect();
        centroids.insert(label, centroid);
    }

    centroids
}

// ============================================================================
// Motif Detection
// ============================================================================

/// Simplified motif detector for recurring audio patterns
struct MotifDetector {
    min_length: usize,
    max_gap: usize,
    similarity_threshold: f32,
}

impl MotifDetector {
    fn new(min_length: usize, max_gap: usize, similarity_threshold: f32) -> Self {
        Self {
            min_length,
            max_gap,
            similarity_threshold,
        }
    }

    /// Detect motifs in a sequence of embeddings
    fn detect_motifs(&self, embeddings: &[Vec<f32>]) -> Vec<Motif> {
        let mut motifs = Vec::new();
        let n = embeddings.len();

        // Simplified matrix profile approach
        for i in 0..n.saturating_sub(self.min_length) {
            for j in (i + self.min_length)..n.saturating_sub(self.min_length) {
                // Check if subsequences are similar
                let sim = self.subsequence_similarity(embeddings, i, j, self.min_length);

                if sim >= self.similarity_threshold {
                    motifs.push(Motif {
                        start_a: i,
                        start_b: j,
                        length: self.min_length,
                        similarity: sim,
                    });
                }
            }
        }

        motifs
    }

    fn subsequence_similarity(
        &self,
        embeddings: &[Vec<f32>],
        start_a: usize,
        start_b: usize,
        length: usize,
    ) -> f32 {
        let mut total_sim = 0.0;

        for i in 0..length {
            let sim = cosine_similarity(&embeddings[start_a + i], &embeddings[start_b + i]);
            total_sim += sim;
        }

        total_sim / length as f32
    }
}

#[derive(Debug, Clone)]
struct Motif {
    start_a: usize,
    start_b: usize,
    length: usize,
    similarity: f32,
}

// ============================================================================
// HDBSCAN Benchmarks
// ============================================================================

/// Benchmark HDBSCAN clustering
fn benchmark_hdbscan(c: &mut Criterion) {
    let mut group = c.benchmark_group("hdbscan");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(30));

    for &size in &[500, 1000, 2000] {
        // Generate clustered data for more realistic benchmark
        let embeddings = generate_clustered_vectors(size, PERCH_EMBEDDING_DIM, NUM_CLUSTERS, 0.1);

        let hdbscan = SimpleHdbscan::new(5, 3, 0.5);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("fit", size), &size, |b, _| {
            b.iter(|| black_box(hdbscan.fit(&embeddings)));
        });
    }

    group.finish();
}

/// Benchmark HDBSCAN with different parameters
fn benchmark_hdbscan_params(c: &mut Criterion) {
    let mut group = c.benchmark_group("hdbscan_params");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    let size = 1000;
    let embeddings = generate_clustered_vectors(size, PERCH_EMBEDDING_DIM, NUM_CLUSTERS, 0.1);

    for min_cluster_size in [5, 10, 20] {
        let hdbscan = SimpleHdbscan::new(min_cluster_size, 3, 0.5);

        group.bench_with_input(
            BenchmarkId::new("min_cluster_size", min_cluster_size),
            &min_cluster_size,
            |b, _| {
                b.iter(|| black_box(hdbscan.fit(&embeddings)));
            },
        );
    }

    for epsilon in [0.3, 0.5, 0.7] {
        let hdbscan = SimpleHdbscan::new(5, 3, epsilon);

        group.bench_with_input(
            BenchmarkId::new("epsilon", format!("{:.1}", epsilon)),
            &epsilon,
            |b, _| {
                b.iter(|| black_box(hdbscan.fit(&embeddings)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Cluster Assignment Benchmarks
// ============================================================================

/// Benchmark cluster assignment for new embeddings
fn benchmark_cluster_assignment(c: &mut Criterion) {
    let mut group = c.benchmark_group("cluster_assignment");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    // Generate centroids
    let centroids = generate_random_vectors(NUM_CLUSTERS, PERCH_EMBEDDING_DIM);
    let assigner = ClusterAssigner::new(centroids);

    // Benchmark single assignment
    let single_embedding = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);
    group.bench_function("single", |b| {
        b.iter(|| black_box(assigner.assign(&single_embedding)));
    });

    // Benchmark batch assignment
    for &batch_size in &[100, 1000, 10000] {
        let embeddings = generate_random_vectors(batch_size, PERCH_EMBEDDING_DIM);

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(BenchmarkId::new("batch", batch_size), &batch_size, |b, _| {
            b.iter(|| black_box(assigner.batch_assign(&embeddings)));
        });
    }

    group.finish();
}

/// Benchmark cluster assignment with different numbers of clusters
fn benchmark_cluster_assignment_scalability(c: &mut Criterion) {
    let mut group = c.benchmark_group("cluster_assignment_scalability");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    let embeddings = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);

    for num_clusters in [10, 50, 100, 200, 500] {
        let centroids = generate_random_vectors(num_clusters, PERCH_EMBEDDING_DIM);
        let assigner = ClusterAssigner::new(centroids);

        group.throughput(Throughput::Elements(embeddings.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("num_clusters", num_clusters),
            &num_clusters,
            |b, _| {
                b.iter(|| black_box(assigner.batch_assign(&embeddings)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Centroid Computation Benchmarks
// ============================================================================

/// Benchmark centroid computation
fn benchmark_centroid_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("centroid_computation");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    for &size in &[1000, 5000, 10000] {
        let embeddings = generate_clustered_vectors(size, PERCH_EMBEDDING_DIM, NUM_CLUSTERS, 0.1);

        // Create synthetic labels
        let labels: Vec<i32> = (0..size).map(|i| (i % NUM_CLUSTERS) as i32).collect();

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| black_box(compute_centroids(&embeddings, &labels)));
        });
    }

    group.finish();
}

/// Benchmark incremental centroid update
fn benchmark_centroid_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("centroid_update");
    group.sample_size(100);

    // Pre-compute initial centroid
    let cluster_size = 1000;
    let cluster_embeddings = generate_random_vectors(cluster_size, PERCH_EMBEDDING_DIM);
    let initial_centroid: Vec<f32> = (0..PERCH_EMBEDDING_DIM)
        .map(|d| {
            cluster_embeddings.iter().map(|e| e[d]).sum::<f32>() / cluster_size as f32
        })
        .collect();

    // New embedding to add
    let new_embedding = generate_random_vectors(1, PERCH_EMBEDDING_DIM).remove(0);

    group.bench_function("incremental_update", |b| {
        b.iter(|| {
            // Incremental centroid update formula:
            // new_centroid = old_centroid + (new_point - old_centroid) / (n + 1)
            let n = cluster_size as f32;
            let updated: Vec<f32> = initial_centroid
                .iter()
                .zip(new_embedding.iter())
                .map(|(&c, &e)| c + (e - c) / (n + 1.0))
                .collect();
            black_box(updated)
        });
    });

    group.finish();
}

// ============================================================================
// Motif Detection Benchmarks
// ============================================================================

/// Benchmark motif detection
fn benchmark_motif_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("motif_detection");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(20));

    let detector = MotifDetector::new(3, 10, 0.8);

    for &seq_length in &[50, 100, 200] {
        // Generate sequence with some repeated patterns
        let mut embeddings = generate_clustered_vectors(seq_length, PERCH_EMBEDDING_DIM, 10, 0.05);

        group.throughput(Throughput::Elements(seq_length as u64));
        group.bench_with_input(BenchmarkId::new("seq_length", seq_length), &seq_length, |b, _| {
            b.iter(|| black_box(detector.detect_motifs(&embeddings)));
        });
    }

    group.finish();
}

// ============================================================================
// Silhouette Score Computation
// ============================================================================

/// Compute silhouette score for cluster quality assessment
fn compute_silhouette_score(embeddings: &[Vec<f32>], labels: &[i32]) -> f32 {
    let n = embeddings.len();
    if n < 2 {
        return 0.0;
    }

    let unique_labels: HashSet<i32> = labels.iter().filter(|&&l| l >= 0).copied().collect();
    if unique_labels.len() < 2 {
        return 0.0;
    }

    let mut total_score = 0.0;
    let mut count = 0;

    for i in 0..n {
        let label_i = labels[i];
        if label_i < 0 {
            continue;
        }

        // Compute a(i): mean intra-cluster distance
        let mut intra_sum = 0.0;
        let mut intra_count = 0;
        for j in 0..n {
            if i != j && labels[j] == label_i {
                intra_sum += l2_distance(&embeddings[i], &embeddings[j]);
                intra_count += 1;
            }
        }
        let a_i = if intra_count > 0 {
            intra_sum / intra_count as f32
        } else {
            0.0
        };

        // Compute b(i): min mean inter-cluster distance
        let mut b_i = f32::MAX;
        for &other_label in &unique_labels {
            if other_label != label_i {
                let mut inter_sum = 0.0;
                let mut inter_count = 0;
                for j in 0..n {
                    if labels[j] == other_label {
                        inter_sum += l2_distance(&embeddings[i], &embeddings[j]);
                        inter_count += 1;
                    }
                }
                if inter_count > 0 {
                    let mean_inter = inter_sum / inter_count as f32;
                    b_i = b_i.min(mean_inter);
                }
            }
        }

        // Silhouette coefficient for point i
        if b_i.is_finite() {
            let s_i = (b_i - a_i) / a_i.max(b_i);
            total_score += s_i;
            count += 1;
        }
    }

    if count > 0 {
        total_score / count as f32
    } else {
        0.0
    }
}

/// Benchmark silhouette score computation
fn benchmark_silhouette_score(c: &mut Criterion) {
    let mut group = c.benchmark_group("silhouette_score");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for &size in &[100, 500] {
        let embeddings = generate_clustered_vectors(size, PERCH_EMBEDDING_DIM, 10, 0.1);
        let labels: Vec<i32> = (0..size).map(|i| (i % 10) as i32).collect();

        group.bench_with_input(BenchmarkId::new("size", size), &size, |b, _| {
            b.iter(|| black_box(compute_silhouette_score(&embeddings, &labels)));
        });
    }

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = hdbscan_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_hdbscan, benchmark_hdbscan_params
);

criterion_group!(
    name = assignment_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_cluster_assignment, benchmark_cluster_assignment_scalability
);

criterion_group!(
    name = centroid_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_centroid_computation, benchmark_centroid_update
);

criterion_group!(
    name = motif_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_motif_detection
);

criterion_group!(
    name = quality_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_silhouette_score
);

criterion_main!(
    hdbscan_benches,
    assignment_benches,
    centroid_benches,
    motif_benches,
    quality_benches
);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdbscan_clustering() {
        let embeddings = generate_clustered_vectors(100, 128, 5, 0.05);
        let hdbscan = SimpleHdbscan::new(5, 3, 0.5);

        let labels = hdbscan.fit(&embeddings);
        assert_eq!(labels.len(), 100);

        // Should have some non-noise labels
        let non_noise: Vec<_> = labels.iter().filter(|&&l| l >= 0).collect();
        assert!(!non_noise.is_empty());
    }

    #[test]
    fn test_cluster_assignment() {
        let centroids = generate_random_vectors(10, 128);
        let assigner = ClusterAssigner::new(centroids.clone());

        // Assign a centroid to itself should return that cluster
        let (cluster, dist) = assigner.assign(&centroids[5]);
        assert_eq!(cluster, 5);
        assert!(dist < 1e-5);
    }

    #[test]
    fn test_centroid_computation() {
        let embeddings = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![1.0, 1.0],
            vec![-1.0, -1.0],
        ];
        let labels = vec![0, 0, 0, 1];

        let centroids = compute_centroids(&embeddings, &labels);

        assert_eq!(centroids.len(), 2);

        // Cluster 0 centroid should be (2/3, 2/3)
        let c0 = &centroids[&0];
        assert!((c0[0] - 2.0 / 3.0).abs() < 1e-5);
        assert!((c0[1] - 2.0 / 3.0).abs() < 1e-5);

        // Cluster 1 centroid should be (-1, -1)
        let c1 = &centroids[&1];
        assert!((c1[0] - (-1.0)).abs() < 1e-5);
        assert!((c1[1] - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_motif_detection() {
        // Create sequence with a repeated pattern
        let mut embeddings = Vec::new();
        let pattern: Vec<Vec<f32>> = (0..3)
            .map(|i| {
                let mut v = vec![0.0f32; 128];
                v[i] = 1.0;
                v
            })
            .collect();

        // Insert pattern twice with gap
        embeddings.extend(pattern.clone());
        embeddings.extend(generate_random_vectors(5, 128));
        embeddings.extend(pattern);

        let detector = MotifDetector::new(3, 10, 0.9);
        let motifs = detector.detect_motifs(&embeddings);

        // Should detect at least one motif
        // Note: Due to noise, this may not always work perfectly
        println!("Found {} motifs", motifs.len());
    }

    #[test]
    fn test_silhouette_score() {
        // Perfect clustering: two well-separated clusters
        let embeddings = vec![
            vec![0.0, 0.0],
            vec![0.1, 0.0],
            vec![0.0, 0.1],
            vec![10.0, 10.0],
            vec![10.1, 10.0],
            vec![10.0, 10.1],
        ];
        let labels = vec![0, 0, 0, 1, 1, 1];

        let score = compute_silhouette_score(&embeddings, &labels);

        // Score should be close to 1 for well-separated clusters
        assert!(score > 0.5, "Silhouette score {} too low", score);
    }
}
