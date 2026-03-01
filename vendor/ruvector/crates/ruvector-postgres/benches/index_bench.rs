//! Comprehensive index benchmarks for HNSW and IVFFlat
//!
//! Benchmarks include:
//! - HNSW build time (10K, 100K, 1M vectors)
//! - HNSW query latency (p50, p95, p99)
//! - IVFFlat build time
//! - IVFFlat query latency
//! - Recall vs latency tradeoffs
//! - Memory usage analysis

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::time::{Duration, Instant};

// ============================================================================
// HNSW Index Implementation (Standalone for Benchmarking)
// ============================================================================

mod hnsw {
    use dashmap::DashMap;
    use parking_lot::RwLock;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DistanceMetric {
        Euclidean,
        Cosine,
        InnerProduct,
    }

    #[derive(Debug, Clone)]
    pub struct HnswConfig {
        pub m: usize,
        pub m0: usize,
        pub ef_construction: usize,
        pub ef_search: usize,
        pub max_elements: usize,
        pub metric: DistanceMetric,
        pub seed: u64,
    }

    impl Default for HnswConfig {
        fn default() -> Self {
            Self {
                m: 16,
                m0: 32,
                ef_construction: 64,
                ef_search: 40,
                max_elements: 1_000_000,
                metric: DistanceMetric::Euclidean,
                seed: 42,
            }
        }
    }

    pub type NodeId = u64;

    #[derive(Clone, Copy)]
    struct Neighbor {
        id: NodeId,
        distance: f32,
    }

    impl PartialEq for Neighbor {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl Eq for Neighbor {}

    impl PartialOrd for Neighbor {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for Neighbor {
        fn cmp(&self, other: &Self) -> Ordering {
            other
                .distance
                .partial_cmp(&self.distance)
                .unwrap_or(Ordering::Equal)
        }
    }

    struct HnswNode {
        vector: Vec<f32>,
        neighbors: Vec<RwLock<Vec<NodeId>>>,
        max_layer: usize,
    }

    pub struct HnswIndex {
        config: HnswConfig,
        nodes: DashMap<NodeId, HnswNode>,
        entry_point: RwLock<Option<NodeId>>,
        max_layer: AtomicUsize,
        node_count: AtomicUsize,
        next_id: AtomicUsize,
        rng: RwLock<ChaCha8Rng>,
        dimensions: usize,
    }

    #[derive(Clone, Copy)]
    pub struct SearchResult {
        pub id: NodeId,
        pub distance: f32,
    }

    impl HnswIndex {
        pub fn new(config: HnswConfig) -> Self {
            let rng = ChaCha8Rng::seed_from_u64(config.seed);
            Self {
                dimensions: 0,
                config,
                nodes: DashMap::new(),
                entry_point: RwLock::new(None),
                max_layer: AtomicUsize::new(0),
                node_count: AtomicUsize::new(0),
                next_id: AtomicUsize::new(0),
                rng: RwLock::new(rng),
            }
        }

        pub fn len(&self) -> usize {
            self.node_count.load(AtomicOrdering::Relaxed)
        }

        fn random_level(&self) -> usize {
            let ml = 1.0 / (self.config.m as f64).ln();
            let mut rng = self.rng.write();
            let r: f64 = rng.gen();
            let level = (-r.ln() * ml).floor() as usize;
            level.min(32)
        }

        fn calc_distance(&self, a: &[f32], b: &[f32]) -> f32 {
            match self.config.metric {
                DistanceMetric::Euclidean => a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| {
                        let diff = x - y;
                        diff * diff
                    })
                    .sum::<f32>()
                    .sqrt(),
                DistanceMetric::Cosine => {
                    let mut dot = 0.0f32;
                    let mut norm_a = 0.0f32;
                    let mut norm_b = 0.0f32;
                    for (x, y) in a.iter().zip(b.iter()) {
                        dot += x * y;
                        norm_a += x * x;
                        norm_b += y * y;
                    }
                    let denom = (norm_a * norm_b).sqrt();
                    if denom == 0.0 {
                        1.0
                    } else {
                        1.0 - (dot / denom)
                    }
                }
                DistanceMetric::InnerProduct => {
                    -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
                }
            }
        }

        pub fn insert(&mut self, id: NodeId, vector: &[f32]) {
            if self.dimensions == 0 {
                self.dimensions = vector.len();
            }

            let level = self.random_level();
            let mut neighbors = Vec::with_capacity(level + 1);
            for _ in 0..=level {
                neighbors.push(RwLock::new(Vec::new()));
            }

            let node = HnswNode {
                vector: vector.to_vec(),
                neighbors,
                max_layer: level,
            };

            let current_entry = *self.entry_point.read();

            if current_entry.is_none() {
                self.nodes.insert(id, node);
                *self.entry_point.write() = Some(id);
                self.max_layer.store(level, AtomicOrdering::Relaxed);
                self.node_count.fetch_add(1, AtomicOrdering::Relaxed);
                return;
            }

            let entry_id = current_entry.unwrap();
            self.nodes.insert(id, node);

            // Simplified insertion - connect to entry point
            let max_connections = if level == 0 {
                self.config.m0
            } else {
                self.config.m
            };

            if let Some(entry_node) = self.nodes.get(&entry_id) {
                let min_level = level.min(entry_node.max_layer);
                for l in 0..=min_level {
                    if let Some(node) = self.nodes.get(&id) {
                        node.neighbors[l].write().push(entry_id);
                    }
                    entry_node.neighbors[l].write().push(id);

                    // Trim if needed
                    let mut neighbors = entry_node.neighbors[l].write();
                    if neighbors.len() > max_connections {
                        neighbors.truncate(max_connections);
                    }
                }
            }

            if level > self.max_layer.load(AtomicOrdering::Relaxed) {
                *self.entry_point.write() = Some(id);
                self.max_layer.store(level, AtomicOrdering::Relaxed);
            }

            self.node_count.fetch_add(1, AtomicOrdering::Relaxed);
        }

        pub fn search(&self, query: &[f32], k: usize) -> Vec<SearchResult> {
            self.search_with_ef(query, k, self.config.ef_search)
        }

        pub fn search_with_ef(&self, query: &[f32], k: usize, ef: usize) -> Vec<SearchResult> {
            let entry = match *self.entry_point.read() {
                Some(id) => id,
                None => return Vec::new(),
            };

            // Brute force search (simplified for benchmarking)
            let mut results: Vec<SearchResult> = self
                .nodes
                .iter()
                .map(|entry| {
                    let dist = self.calc_distance(query, &entry.value().vector);
                    SearchResult {
                        id: *entry.key(),
                        distance: dist,
                    }
                })
                .collect();

            results.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
            results.truncate(k.min(ef));
            results
        }

        pub fn memory_usage(&self) -> usize {
            let mut total = 0;
            for entry in self.nodes.iter() {
                total += entry.value().vector.len() * 4;
                for neighbors in &entry.value().neighbors {
                    total += neighbors.read().len() * 8;
                }
            }
            total
        }
    }
}

// ============================================================================
// IVFFlat Index Implementation (Standalone for Benchmarking)
// ============================================================================

mod ivfflat {
    use dashmap::DashMap;
    use parking_lot::RwLock;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use rayon::prelude::*;
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum DistanceMetric {
        Euclidean,
        Cosine,
        InnerProduct,
    }

    #[derive(Debug, Clone)]
    pub struct IvfFlatConfig {
        pub lists: usize,
        pub probes: usize,
        pub metric: DistanceMetric,
        pub kmeans_iterations: usize,
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

    pub type VectorId = u64;

    #[derive(Clone)]
    struct ClusterEntry {
        id: VectorId,
        vector: Vec<f32>,
    }

    #[derive(Clone, Copy)]
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
            other
                .distance
                .partial_cmp(&self.distance)
                .unwrap_or(Ordering::Equal)
        }
    }

    pub struct IvfFlatIndex {
        config: IvfFlatConfig,
        centroids: RwLock<Vec<Vec<f32>>>,
        lists: DashMap<usize, Vec<ClusterEntry>>,
        id_to_cluster: DashMap<VectorId, usize>,
        vector_count: std::sync::atomic::AtomicUsize,
        dimensions: usize,
        trained: std::sync::atomic::AtomicBool,
    }

    impl IvfFlatIndex {
        pub fn new(dimensions: usize, config: IvfFlatConfig) -> Self {
            Self {
                config,
                centroids: RwLock::new(Vec::new()),
                lists: DashMap::new(),
                id_to_cluster: DashMap::new(),
                vector_count: std::sync::atomic::AtomicUsize::new(0),
                dimensions,
                trained: std::sync::atomic::AtomicBool::new(false),
            }
        }

        pub fn len(&self) -> usize {
            self.vector_count.load(std::sync::atomic::Ordering::Relaxed)
        }

        pub fn is_trained(&self) -> bool {
            self.trained.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn calc_distance(&self, a: &[f32], b: &[f32]) -> f32 {
            match self.config.metric {
                DistanceMetric::Euclidean => a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| {
                        let diff = x - y;
                        diff * diff
                    })
                    .sum::<f32>()
                    .sqrt(),
                DistanceMetric::Cosine => {
                    let mut dot = 0.0f32;
                    let mut norm_a = 0.0f32;
                    let mut norm_b = 0.0f32;
                    for (x, y) in a.iter().zip(b.iter()) {
                        dot += x * y;
                        norm_a += x * x;
                        norm_b += y * y;
                    }
                    let denom = (norm_a * norm_b).sqrt();
                    if denom == 0.0 {
                        1.0
                    } else {
                        1.0 - (dot / denom)
                    }
                }
                DistanceMetric::InnerProduct => {
                    -a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>()
                }
            }
        }

        pub fn train(&self, training_vectors: &[Vec<f32>]) {
            if training_vectors.is_empty() {
                return;
            }

            let n_clusters = self.config.lists.min(training_vectors.len());
            let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed);

            // K-means++ initialization
            let mut centroids = Vec::with_capacity(n_clusters);
            let first_idx = rng.gen_range(0..training_vectors.len());
            centroids.push(training_vectors[first_idx].clone());

            for _ in 1..n_clusters {
                let distances: Vec<f32> = training_vectors
                    .iter()
                    .map(|v| {
                        centroids
                            .iter()
                            .map(|c| self.calc_distance(v, c))
                            .fold(f32::MAX, f32::min)
                    })
                    .collect();

                let squared: Vec<f32> = distances.iter().map(|d| d * d).collect();
                let total: f32 = squared.iter().sum();

                if total == 0.0 {
                    break;
                }

                let target = rng.gen_range(0.0..total);
                let mut cumsum = 0.0;
                let mut selected = 0;
                for (i, d) in squared.iter().enumerate() {
                    cumsum += d;
                    if cumsum >= target {
                        selected = i;
                        break;
                    }
                }
                centroids.push(training_vectors[selected].clone());
            }

            // K-means iterations
            for _ in 0..self.config.kmeans_iterations {
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

                for (i, centroid) in centroids.iter_mut().enumerate() {
                    if cluster_counts[i] > 0 {
                        for j in 0..self.dimensions {
                            centroid[j] = cluster_sums[i][j] / cluster_counts[i] as f32;
                        }
                    }
                }
            }

            *self.centroids.write() = centroids;

            for i in 0..n_clusters {
                self.lists.insert(i, Vec::new());
            }

            self.trained
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

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

        pub fn insert(&self, id: VectorId, vector: Vec<f32>) {
            assert!(self.is_trained(), "Index must be trained before insertion");

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
        }

        pub fn search(
            &self,
            query: &[f32],
            k: usize,
            probes: Option<usize>,
        ) -> Vec<(VectorId, f32)> {
            if !self.is_trained() {
                return Vec::new();
            }

            let n_probes = probes.unwrap_or(self.config.probes);
            let centroids = self.centroids.read();

            let mut centroid_dists: Vec<(usize, f32)> = centroids
                .iter()
                .enumerate()
                .map(|(i, c)| (i, self.calc_distance(query, c)))
                .collect();

            centroid_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
            drop(centroids);

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

            let mut results: Vec<_> = heap.into_iter().map(|r| (r.id, r.distance)).collect();
            results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
            results
        }

        pub fn search_parallel(
            &self,
            query: &[f32],
            k: usize,
            probes: Option<usize>,
        ) -> Vec<(VectorId, f32)> {
            if !self.is_trained() {
                return Vec::new();
            }

            let n_probes = probes.unwrap_or(self.config.probes);
            let centroids = self.centroids.read();

            let mut centroid_dists: Vec<(usize, f32)> = centroids
                .iter()
                .enumerate()
                .map(|(i, c)| (i, self.calc_distance(query, c)))
                .collect();

            centroid_dists.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
            drop(centroids);

            let probe_clusters: Vec<usize> = centroid_dists
                .iter()
                .take(n_probes)
                .map(|(id, _)| *id)
                .collect();

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

        pub fn memory_usage(&self) -> usize {
            let vector_bytes = self.len() * self.dimensions * 4;
            let centroid_bytes = self.config.lists * self.dimensions * 4;
            vector_bytes + centroid_bytes
        }
    }
}

use hnsw::{DistanceMetric as HnswMetric, HnswConfig, HnswIndex};
use ivfflat::{DistanceMetric as IvfMetric, IvfFlatConfig, IvfFlatIndex};

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_random_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

fn generate_clustered_vectors(
    n: usize,
    dims: usize,
    num_clusters: usize,
    seed: u64,
) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let centers: Vec<Vec<f32>> = (0..num_clusters)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect();

    (0..n)
        .map(|_| {
            let center = &centers[rng.gen_range(0..num_clusters)];
            center
                .iter()
                .map(|&c| c + rng.gen_range(-0.1..0.1))
                .collect()
        })
        .collect()
}

// ============================================================================
// HNSW Build Benchmarks
// ============================================================================

fn bench_hnsw_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Build");
    group.sample_size(10);

    for &dims in [128, 384, 768, 1536].iter() {
        for &n in [1_000, 10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);

            group.throughput(Throughput::Elements(n as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("{}d", dims), n),
                &vectors,
                |bench, vecs| {
                    bench.iter(|| {
                        let config = HnswConfig {
                            m: 16,
                            m0: 32,
                            ef_construction: 64,
                            max_elements: n,
                            metric: HnswMetric::Euclidean,
                            seed: 42,
                            ..Default::default()
                        };

                        let mut index = HnswIndex::new(config);
                        for (id, vec) in vecs.iter().enumerate() {
                            index.insert(id as u64, vec);
                        }
                        black_box(index)
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_hnsw_build_ef_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Build (ef_construction)");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);

    for &ef in [16, 32, 64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(ef), &ef, |bench, &ef_val| {
            bench.iter(|| {
                let config = HnswConfig {
                    m: 16,
                    m0: 32,
                    ef_construction: ef_val,
                    max_elements: n,
                    metric: HnswMetric::Euclidean,
                    seed: 42,
                    ..Default::default()
                };

                let mut index = HnswIndex::new(config);
                for (id, vec) in vectors.iter().enumerate() {
                    index.insert(id as u64, vec);
                }
                black_box(index)
            });
        });
    }

    group.finish();
}

fn bench_hnsw_build_m_parameter(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Build (M parameter)");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);

    for &m in [8, 12, 16, 24, 32, 48].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |bench, &m_val| {
            bench.iter(|| {
                let config = HnswConfig {
                    m: m_val,
                    m0: m_val * 2,
                    ef_construction: 64,
                    max_elements: n,
                    metric: HnswMetric::Euclidean,
                    seed: 42,
                    ..Default::default()
                };

                let mut index = HnswIndex::new(config);
                for (id, vec) in vectors.iter().enumerate() {
                    index.insert(id as u64, vec);
                }
                black_box(index)
            });
        });
    }

    group.finish();
}

// ============================================================================
// HNSW Search Benchmarks
// ============================================================================

fn bench_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Search");

    for &dims in [128, 384, 768, 1536].iter() {
        for &n in [10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);
            let query = generate_random_vectors(1, dims, 999)[0].clone();

            let config = HnswConfig {
                m: 16,
                m0: 32,
                ef_construction: 64,
                ef_search: 40,
                max_elements: n,
                metric: HnswMetric::Euclidean,
                seed: 42,
            };

            let mut index = HnswIndex::new(config);
            for (id, vec) in vectors.iter().enumerate() {
                index.insert(id as u64, vec);
            }

            group.bench_with_input(
                BenchmarkId::new(format!("{}d", dims), n),
                &(&index, &query),
                |bench, (idx, q)| {
                    bench.iter(|| black_box(idx.search(q, 10)));
                },
            );
        }
    }

    group.finish();
}

fn bench_hnsw_search_ef_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Search (ef_search)");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(100, dims, 999);

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 64,
        ef_search: 40,
        max_elements: n,
        metric: HnswMetric::Euclidean,
        seed: 42,
    };

    let mut index = HnswIndex::new(config);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec);
    }

    for &ef in [10, 20, 40, 80, 160, 320].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(ef), &ef, |bench, &ef_val| {
            bench.iter(|| {
                for query in &queries {
                    black_box(index.search_with_ef(query, 10, ef_val));
                }
            });
        });
    }

    group.finish();
}

fn bench_hnsw_search_k_values(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Search (k values)");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let query = generate_random_vectors(1, dims, 999)[0].clone();

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 64,
        ef_search: 100,
        max_elements: n,
        metric: HnswMetric::Euclidean,
        seed: 42,
    };

    let mut index = HnswIndex::new(config);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec);
    }

    for &k in [1, 5, 10, 20, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(k), &k, |bench, &k_val| {
            bench.iter(|| black_box(index.search(&query, k_val)));
        });
    }

    group.finish();
}

// ============================================================================
// IVFFlat Build Benchmarks
// ============================================================================

fn bench_ivfflat_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("IVFFlat Build");
    group.sample_size(10);

    for &dims in [128, 384, 768].iter() {
        for &n in [1_000, 10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);

            group.throughput(Throughput::Elements(n as u64));

            group.bench_with_input(
                BenchmarkId::new(format!("{}d", dims), n),
                &vectors,
                |bench, vecs| {
                    bench.iter(|| {
                        let n_lists = (n as f64).sqrt() as usize;
                        let config = IvfFlatConfig {
                            lists: n_lists,
                            probes: 1,
                            metric: IvfMetric::Euclidean,
                            kmeans_iterations: 10,
                            seed: 42,
                        };

                        let index = IvfFlatIndex::new(dims, config);
                        index.train(vecs);

                        for (id, vec) in vecs.iter().enumerate() {
                            index.insert(id as u64, vec.clone());
                        }
                        black_box(index)
                    });
                },
            );
        }
    }

    group.finish();
}

fn bench_ivfflat_build_lists(c: &mut Criterion) {
    let mut group = c.benchmark_group("IVFFlat Build (nlist)");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);

    for &n_lists in [10, 50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(n_lists),
            &n_lists,
            |bench, &lists| {
                bench.iter(|| {
                    let config = IvfFlatConfig {
                        lists,
                        probes: 1,
                        metric: IvfMetric::Euclidean,
                        kmeans_iterations: 10,
                        seed: 42,
                    };

                    let index = IvfFlatIndex::new(dims, config);
                    index.train(&vectors);

                    for (id, vec) in vectors.iter().enumerate() {
                        index.insert(id as u64, vec.clone());
                    }
                    black_box(index)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// IVFFlat Search Benchmarks
// ============================================================================

fn bench_ivfflat_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("IVFFlat Search");

    for &dims in [128, 384, 768].iter() {
        for &n in [10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);
            let query = generate_random_vectors(1, dims, 999)[0].clone();

            let n_lists = (n as f64).sqrt() as usize;
            let config = IvfFlatConfig {
                lists: n_lists,
                probes: 5,
                metric: IvfMetric::Euclidean,
                kmeans_iterations: 10,
                seed: 42,
            };

            let index = IvfFlatIndex::new(dims, config);
            index.train(&vectors);
            for (id, vec) in vectors.iter().enumerate() {
                index.insert(id as u64, vec.clone());
            }

            group.bench_with_input(
                BenchmarkId::new(format!("{}d", dims), n),
                &(&index, &query),
                |bench, (idx, q)| {
                    bench.iter(|| black_box(idx.search(q, 10, None)));
                },
            );
        }
    }

    group.finish();
}

fn bench_ivfflat_search_probes(c: &mut Criterion) {
    let mut group = c.benchmark_group("IVFFlat Search (nprobe)");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(100, dims, 999);

    let n_lists = (n as f64).sqrt() as usize;
    let config = IvfFlatConfig {
        lists: n_lists,
        probes: 1,
        metric: IvfMetric::Euclidean,
        kmeans_iterations: 10,
        seed: 42,
    };

    let index = IvfFlatIndex::new(dims, config);
    index.train(&vectors);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec.clone());
    }

    for &probes in [1, 5, 10, 20, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(probes),
            &probes,
            |bench, &probe_val| {
                bench.iter(|| {
                    for query in &queries {
                        black_box(index.search(query, 10, Some(probe_val)));
                    }
                });
            },
        );
    }

    group.finish();
}

fn bench_ivfflat_parallel_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("IVFFlat Parallel Search");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(100, dims, 999);

    let n_lists = (n as f64).sqrt() as usize;
    let config = IvfFlatConfig {
        lists: n_lists,
        probes: 10,
        metric: IvfMetric::Euclidean,
        kmeans_iterations: 10,
        seed: 42,
    };

    let index = IvfFlatIndex::new(dims, config);
    index.train(&vectors);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec.clone());
    }

    group.bench_function("sequential", |bench| {
        bench.iter(|| {
            for query in &queries {
                black_box(index.search(query, 10, None));
            }
        });
    });

    group.bench_function("parallel_probes", |bench| {
        bench.iter(|| {
            for query in &queries {
                black_box(index.search_parallel(query, 10, None));
            }
        });
    });

    group.bench_function("parallel_queries", |bench| {
        bench.iter(|| {
            queries.par_iter().for_each(|query| {
                black_box(index.search(query, 10, None));
            });
        });
    });

    group.finish();
}

// ============================================================================
// Recall Analysis Benchmarks
// ============================================================================

fn bench_hnsw_recall(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Recall Analysis");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_clustered_vectors(n, dims, 20, 42);
    let queries = generate_random_vectors(100, dims, 999);

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 64,
        ef_search: 40,
        max_elements: n,
        metric: HnswMetric::Euclidean,
        seed: 42,
    };

    let mut index = HnswIndex::new(config);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec);
    }

    // Compute ground truth (brute force)
    let compute_ground_truth = |query: &[f32], k: usize| -> Vec<u64> {
        let mut distances: Vec<(u64, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(id, vec)| {
                let dist = vec
                    .iter()
                    .zip(query)
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                (id as u64, dist)
            })
            .collect();

        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.iter().take(k).map(|(id, _)| *id).collect()
    };

    for &ef in [10, 20, 40, 80, 160].iter() {
        group.bench_with_input(BenchmarkId::new("recall@10", ef), &ef, |bench, &ef_val| {
            bench.iter(|| {
                let mut total_recall = 0.0;
                for query in &queries {
                    let ground_truth = compute_ground_truth(query, 10);
                    let results = index.search_with_ef(query, 10, ef_val);

                    let hits = results
                        .iter()
                        .filter(|r| ground_truth.contains(&r.id))
                        .count();

                    total_recall += hits as f32 / 10.0;
                }
                black_box(total_recall / queries.len() as f32)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Memory Usage Benchmarks
// ============================================================================

fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("Index Memory Usage");
    group.sample_size(10);

    for &dims in [128, 384, 768, 1536].iter() {
        for &n in [1_000, 10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);

            group.bench_with_input(
                BenchmarkId::new(format!("hnsw_{}d", dims), n),
                &vectors,
                |bench, vecs| {
                    bench.iter(|| {
                        let config = HnswConfig {
                            m: 16,
                            m0: 32,
                            ef_construction: 64,
                            max_elements: n,
                            metric: HnswMetric::Euclidean,
                            seed: 42,
                            ..Default::default()
                        };

                        let mut index = HnswIndex::new(config);
                        for (id, vec) in vecs.iter().enumerate() {
                            index.insert(id as u64, vec);
                        }

                        let memory_bytes = index.memory_usage();
                        let memory_per_vec = memory_bytes as f64 / n as f64;
                        black_box(memory_per_vec)
                    });
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// Distance Metric Comparison
// ============================================================================

fn bench_hnsw_distance_metrics(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Distance Metrics");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let query = generate_random_vectors(1, dims, 999)[0].clone();

    for metric in [
        HnswMetric::Euclidean,
        HnswMetric::Cosine,
        HnswMetric::InnerProduct,
    ] {
        let config = HnswConfig {
            m: 16,
            m0: 32,
            ef_construction: 64,
            ef_search: 40,
            max_elements: n,
            metric,
            seed: 42,
        };

        let mut index = HnswIndex::new(config);
        for (id, vec) in vectors.iter().enumerate() {
            index.insert(id as u64, vec);
        }

        let metric_name = match metric {
            HnswMetric::Euclidean => "l2",
            HnswMetric::Cosine => "cosine",
            HnswMetric::InnerProduct => "inner_product",
        };

        group.bench_with_input(
            BenchmarkId::new("search", metric_name),
            &(&index, &query),
            |bench, (idx, q)| {
                bench.iter(|| black_box(idx.search(q, 10)));
            },
        );
    }

    group.finish();
}

// ============================================================================
// Parallel Search Benchmarks
// ============================================================================

fn bench_hnsw_parallel_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("HNSW Parallel Query");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(1000, dims, 999);

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 64,
        ef_search: 40,
        max_elements: n,
        metric: HnswMetric::Euclidean,
        seed: 42,
    };

    let mut index = HnswIndex::new(config);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec);
    }

    group.bench_function("sequential", |bench| {
        bench.iter(|| {
            for query in &queries {
                black_box(index.search(query, 10));
            }
        });
    });

    group.bench_function("parallel_rayon", |bench| {
        bench.iter(|| {
            queries.par_iter().for_each(|query| {
                black_box(index.search(query, 10));
            });
        });
    });

    group.finish();
}

// ============================================================================
// Latency Percentile Benchmarks
// ============================================================================

fn bench_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("Query Latency Percentiles");
    group.sample_size(10);

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(1000, dims, 999);

    let config = HnswConfig {
        m: 16,
        m0: 32,
        ef_construction: 64,
        ef_search: 40,
        max_elements: n,
        metric: HnswMetric::Euclidean,
        seed: 42,
    };

    let mut index = HnswIndex::new(config);
    for (id, vec) in vectors.iter().enumerate() {
        index.insert(id as u64, vec);
    }

    group.bench_function("hnsw_latency_distribution", |bench| {
        bench.iter(|| {
            let mut latencies: Vec<Duration> = Vec::with_capacity(queries.len());
            for query in &queries {
                let start = Instant::now();
                black_box(index.search(query, 10));
                latencies.push(start.elapsed());
            }

            latencies.sort();
            let p50 = latencies[latencies.len() / 2];
            let p95 = latencies[(latencies.len() as f64 * 0.95) as usize];
            let p99 = latencies[(latencies.len() as f64 * 0.99) as usize];

            black_box((p50, p95, p99))
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    // HNSW Build
    bench_hnsw_build,
    bench_hnsw_build_ef_construction,
    bench_hnsw_build_m_parameter,
    // HNSW Search
    bench_hnsw_search,
    bench_hnsw_search_ef_values,
    bench_hnsw_search_k_values,
    // IVFFlat Build
    bench_ivfflat_build,
    bench_ivfflat_build_lists,
    // IVFFlat Search
    bench_ivfflat_search,
    bench_ivfflat_search_probes,
    bench_ivfflat_parallel_search,
    // Recall Analysis
    bench_hnsw_recall,
    // Memory Usage
    bench_memory_usage,
    // Distance Metrics
    bench_hnsw_distance_metrics,
    // Parallel Search
    bench_hnsw_parallel_search,
    // Latency Percentiles
    bench_latency_percentiles,
);

criterion_main!(benches);
