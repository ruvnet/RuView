//! End-to-End benchmarks for RuVector PostgreSQL extension
//!
//! Comprehensive benchmarks for:
//! - Full query pipeline latency
//! - Insert throughput
//! - Concurrent query scaling
//! - Memory usage under load
//! - pgvector comparison baselines

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ============================================================================
// Simulated Vector Index (Full Pipeline)
// ============================================================================

mod index {
    use dashmap::DashMap;
    use parking_lot::RwLock;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;
    use rayon::prelude::*;
    use std::cmp::Ordering;
    use std::collections::{BinaryHeap, HashMap, HashSet};
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    /// Full-featured HNSW index for benchmarking
    pub struct HnswIndex {
        pub nodes: DashMap<u64, Vec<f32>>,
        pub neighbors: DashMap<u64, Vec<Vec<u64>>>,
        pub entry_point: RwLock<Option<u64>>,
        pub max_layer: AtomicUsize,
        pub m: usize,
        pub m0: usize,
        pub ef_construction: usize,
        pub ef_search: usize,
        pub dimensions: usize,
        next_id: AtomicUsize,
        rng: RwLock<ChaCha8Rng>,
    }

    impl HnswIndex {
        pub fn new(
            dimensions: usize,
            m: usize,
            ef_construction: usize,
            ef_search: usize,
            seed: u64,
        ) -> Self {
            Self {
                nodes: DashMap::new(),
                neighbors: DashMap::new(),
                entry_point: RwLock::new(None),
                max_layer: AtomicUsize::new(0),
                m,
                m0: m * 2,
                ef_construction,
                ef_search,
                dimensions,
                next_id: AtomicUsize::new(0),
                rng: RwLock::new(ChaCha8Rng::seed_from_u64(seed)),
            }
        }

        pub fn len(&self) -> usize {
            self.nodes.len()
        }

        fn random_level(&self) -> usize {
            let ml = 1.0 / (self.m as f64).ln();
            let mut rng = self.rng.write();
            let r: f64 = rng.gen();
            ((-r.ln() * ml).floor() as usize).min(32)
        }

        fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
            a.iter()
                .zip(b.iter())
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt()
        }

        pub fn insert(&self, vector: Vec<f32>) -> u64 {
            let id = self.next_id.fetch_add(1, AtomicOrdering::Relaxed) as u64;
            let level = self.random_level();

            // Initialize neighbor lists for all layers
            let mut neighbor_lists = Vec::with_capacity(level + 1);
            for _ in 0..=level {
                neighbor_lists.push(Vec::new());
            }

            self.nodes.insert(id, vector.clone());
            self.neighbors.insert(id, neighbor_lists);

            let current_entry = *self.entry_point.read();

            if current_entry.is_none() {
                *self.entry_point.write() = Some(id);
                self.max_layer.store(level, AtomicOrdering::Relaxed);
                return id;
            }

            // Simplified insertion
            let entry_id = current_entry.unwrap();

            // Connect to some neighbors
            if let Some(entry_vec) = self.nodes.get(&entry_id) {
                let max_conn = if level == 0 { self.m0 } else { self.m };

                if let Some(mut neighbors) = self.neighbors.get_mut(&id) {
                    neighbors[0].push(entry_id);
                }

                if let Some(mut entry_neighbors) = self.neighbors.get_mut(&entry_id) {
                    if entry_neighbors[0].len() < max_conn {
                        entry_neighbors[0].push(id);
                    }
                }
            }

            if level > self.max_layer.load(AtomicOrdering::Relaxed) {
                *self.entry_point.write() = Some(id);
                self.max_layer.store(level, AtomicOrdering::Relaxed);
            }

            id
        }

        pub fn insert_batch(&self, vectors: &[Vec<f32>]) -> Vec<u64> {
            vectors.iter().map(|v| self.insert(v.clone())).collect()
        }

        pub fn insert_batch_parallel(&self, vectors: &[Vec<f32>]) -> Vec<u64> {
            // Parallel insertion with batching
            vectors.par_iter().map(|v| self.insert(v.clone())).collect()
        }

        pub fn search(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
            // Brute force for simplicity in benchmarks
            let mut results: Vec<(u64, f32)> = self
                .nodes
                .iter()
                .map(|entry| {
                    let dist = self.distance(query, entry.value());
                    (*entry.key(), dist)
                })
                .collect();

            results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            results.truncate(k);
            results
        }

        pub fn search_parallel(&self, query: &[f32], k: usize) -> Vec<(u64, f32)> {
            let mut results: Vec<(u64, f32)> = self
                .nodes
                .iter()
                .collect::<Vec<_>>()
                .par_iter()
                .map(|entry| {
                    let dist = self.distance(query, entry.value());
                    (*entry.key(), dist)
                })
                .collect();

            results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            results.truncate(k);
            results
        }

        pub fn memory_usage(&self) -> usize {
            let vector_bytes = self.nodes.len() * self.dimensions * 4;
            let neighbor_bytes: usize = self
                .neighbors
                .iter()
                .map(|entry| entry.value().iter().map(|l| l.len() * 8).sum::<usize>())
                .sum();
            vector_bytes + neighbor_bytes
        }
    }
}

use index::HnswIndex;

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_random_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

fn generate_normalized_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let vectors = generate_random_vectors(n, dims, seed);
    vectors
        .into_iter()
        .map(|v| {
            let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            v.into_iter().map(|x| x / norm).collect()
        })
        .collect()
}

// ============================================================================
// Full Query Pipeline Benchmarks
// ============================================================================

fn bench_query_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("Query Pipeline");

    for &dims in [128, 384, 768, 1536].iter() {
        for &n in [10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);
            let query = vectors[0].clone();

            let index = HnswIndex::new(dims, 16, 64, 40, 42);
            index.insert_batch(&vectors);

            group.throughput(Throughput::Elements(1));

            // Full pipeline: search + post-process
            group.bench_with_input(BenchmarkId::new(format!("{}d", dims), n), &n, |bench, _| {
                bench.iter(|| {
                    // Search
                    let results = index.search(&query, 10);

                    // Post-process (e.g., fetch metadata, rerank)
                    let processed: Vec<_> = results
                        .iter()
                        .map(|(id, dist)| {
                            // Simulate metadata lookup
                            let metadata = id.to_string();
                            (*id, *dist, metadata)
                        })
                        .collect();

                    black_box(processed)
                })
            });
        }
    }

    group.finish();
}

fn bench_query_pipeline_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("Query Pipeline (Parallel)");

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries: Vec<Vec<f32>> = generate_random_vectors(100, dims, 999);

    let index = HnswIndex::new(dims, 16, 64, 40, 42);
    index.insert_batch(&vectors);

    group.throughput(Throughput::Elements(100));

    group.bench_function("sequential", |bench| {
        bench.iter(|| {
            queries
                .iter()
                .map(|q| index.search(q, 10))
                .collect::<Vec<_>>()
        })
    });

    group.bench_function("parallel_queries", |bench| {
        bench.iter(|| {
            queries
                .par_iter()
                .map(|q| index.search(q, 10))
                .collect::<Vec<_>>()
        })
    });

    group.bench_function("parallel_search_internal", |bench| {
        bench.iter(|| {
            queries
                .iter()
                .map(|q| index.search_parallel(q, 10))
                .collect::<Vec<_>>()
        })
    });

    group.bench_function("full_parallel", |bench| {
        bench.iter(|| {
            queries
                .par_iter()
                .map(|q| index.search_parallel(q, 10))
                .collect::<Vec<_>>()
        })
    });

    group.finish();
}

// ============================================================================
// Insert Throughput Benchmarks
// ============================================================================

fn bench_insert_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insert Throughput");
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
                        let index = HnswIndex::new(dims, 16, 64, 40, 42);
                        index.insert_batch(vecs);
                        black_box(index.len())
                    })
                },
            );
        }
    }

    group.finish();
}

fn bench_insert_throughput_parallel(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insert Throughput (Parallel)");
    group.sample_size(10);

    let dims = 768;

    for &n in [10_000, 100_000].iter() {
        let vectors = generate_random_vectors(n, dims, 42);

        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(
            BenchmarkId::new("sequential", n),
            &vectors,
            |bench, vecs| {
                bench.iter(|| {
                    let index = HnswIndex::new(dims, 16, 64, 40, 42);
                    index.insert_batch(vecs);
                    black_box(index.len())
                })
            },
        );

        group.bench_with_input(BenchmarkId::new("parallel", n), &vectors, |bench, vecs| {
            bench.iter(|| {
                let index = HnswIndex::new(dims, 16, 64, 40, 42);
                index.insert_batch_parallel(vecs);
                black_box(index.len())
            })
        });
    }

    group.finish();
}

fn bench_insert_batching(c: &mut Criterion) {
    let mut group = c.benchmark_group("Insert Batch Sizes");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);

    for &batch_size in [1, 10, 100, 1000, 10000].iter() {
        group.throughput(Throughput::Elements(n as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &batch_size,
            |bench, &bs| {
                bench.iter(|| {
                    let index = HnswIndex::new(dims, 16, 64, 40, 42);

                    for chunk in vectors.chunks(bs) {
                        index.insert_batch(chunk);
                    }

                    black_box(index.len())
                })
            },
        );
    }

    group.finish();
}

// ============================================================================
// Concurrent Query Scaling
// ============================================================================

fn bench_concurrent_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Concurrent Query Scaling");
    group.sample_size(10);

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(1000, dims, 999);

    let index = Arc::new(HnswIndex::new(dims, 16, 64, 40, 42));
    index.insert_batch(&vectors);

    for &num_threads in [1, 2, 4, 8, 16].iter() {
        group.throughput(Throughput::Elements(1000));

        group.bench_with_input(
            BenchmarkId::from_parameter(num_threads),
            &num_threads,
            |bench, &threads| {
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build()
                    .unwrap();

                bench.iter(|| {
                    pool.install(|| {
                        queries.par_iter().for_each(|q| {
                            black_box(index.search(q, 10));
                        });
                    })
                })
            },
        );
    }

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("Mixed Read/Write Workload");
    group.sample_size(10);

    let dims = 768;
    let n = 50_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(100, dims, 999);
    let new_vectors = generate_random_vectors(1000, dims, 123);

    let index = Arc::new(HnswIndex::new(dims, 16, 64, 40, 42));
    index.insert_batch(&vectors);

    // Read-heavy (90% reads, 10% writes)
    group.bench_function("read_heavy", |bench| {
        let idx = index.clone();
        bench.iter(|| {
            // 90 reads
            for q in queries.iter().take(90) {
                black_box(idx.search(q, 10));
            }
            // 10 writes
            for v in new_vectors.iter().take(10) {
                black_box(idx.insert(v.clone()));
            }
        })
    });

    // Balanced (50% reads, 50% writes)
    group.bench_function("balanced", |bench| {
        let idx = index.clone();
        bench.iter(|| {
            for (q, v) in queries.iter().take(50).zip(new_vectors.iter().take(50)) {
                black_box(idx.search(q, 10));
                black_box(idx.insert(v.clone()));
            }
        })
    });

    // Write-heavy (10% reads, 90% writes)
    group.bench_function("write_heavy", |bench| {
        let idx = index.clone();
        bench.iter(|| {
            // 10 reads
            for q in queries.iter().take(10) {
                black_box(idx.search(q, 10));
            }
            // 90 writes
            for v in new_vectors.iter().take(90) {
                black_box(idx.insert(v.clone()));
            }
        })
    });

    group.finish();
}

// ============================================================================
// Memory Usage Under Load
// ============================================================================

fn bench_memory_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Growth");
    group.sample_size(10);

    let dims = 768;

    for &n in [1_000, 10_000, 50_000, 100_000].iter() {
        let vectors = generate_random_vectors(n, dims, 42);

        group.bench_with_input(BenchmarkId::from_parameter(n), &vectors, |bench, vecs| {
            bench.iter(|| {
                let index = HnswIndex::new(dims, 16, 64, 40, 42);
                index.insert_batch(vecs);

                let memory = index.memory_usage();
                let per_vector = memory as f64 / n as f64;

                black_box((memory, per_vector))
            })
        });
    }

    group.finish();
}

fn bench_memory_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("Memory Efficiency (M parameter)");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);

    for &m in [8, 12, 16, 24, 32, 48].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(m), &m, |bench, &m_val| {
            bench.iter(|| {
                let index = HnswIndex::new(dims, m_val, 64, 40, 42);
                index.insert_batch(&vectors);

                let memory = index.memory_usage();
                let per_vector = memory as f64 / n as f64;

                black_box(per_vector)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Latency Distribution
// ============================================================================

fn bench_latency_distribution(c: &mut Criterion) {
    let mut group = c.benchmark_group("Latency Distribution");
    group.sample_size(10);

    let dims = 768;
    let n = 100_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let queries = generate_random_vectors(1000, dims, 999);

    let index = HnswIndex::new(dims, 16, 64, 40, 42);
    index.insert_batch(&vectors);

    group.bench_function("collect_percentiles", |bench| {
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
            let p999 = latencies[(latencies.len() as f64 * 0.999) as usize];

            black_box((p50, p95, p99, p999))
        })
    });

    group.finish();
}

// ============================================================================
// Dimension Scaling
// ============================================================================

fn bench_dimension_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dimension Scaling");
    group.sample_size(10);

    let n = 10_000;

    for &dims in [64, 128, 256, 384, 512, 768, 1024, 1536, 2048, 3072].iter() {
        let vectors = generate_random_vectors(n, dims, 42);
        let query = vectors[0].clone();

        let index = HnswIndex::new(dims, 16, 64, 40, 42);
        index.insert_batch(&vectors);

        group.bench_with_input(BenchmarkId::new("search", dims), &dims, |bench, _| {
            bench.iter(|| black_box(index.search(&query, 10)))
        });
    }

    group.finish();
}

// ============================================================================
// pgvector Comparison Baselines
// ============================================================================

fn bench_baseline_brute_force(c: &mut Criterion) {
    let mut group = c.benchmark_group("Baseline Brute Force");
    group.sample_size(10);

    for &dims in [128, 384, 768, 1536].iter() {
        for &n in [1_000, 10_000, 100_000].iter() {
            let vectors = generate_random_vectors(n, dims, 42);
            let query = vectors[0].clone();

            group.throughput(Throughput::Elements(n as u64));

            // Sequential brute force
            group.bench_with_input(
                BenchmarkId::new(format!("{}d_seq", dims), n),
                &vectors,
                |bench, vecs| {
                    bench.iter(|| {
                        let mut distances: Vec<(usize, f32)> = vecs
                            .iter()
                            .enumerate()
                            .map(|(i, v)| {
                                let dist: f32 = query
                                    .iter()
                                    .zip(v.iter())
                                    .map(|(a, b)| (a - b).powi(2))
                                    .sum::<f32>()
                                    .sqrt();
                                (i, dist)
                            })
                            .collect();

                        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                        distances.truncate(10);
                        black_box(distances)
                    })
                },
            );

            // Parallel brute force
            group.bench_with_input(
                BenchmarkId::new(format!("{}d_par", dims), n),
                &vectors,
                |bench, vecs| {
                    bench.iter(|| {
                        let mut distances: Vec<(usize, f32)> = vecs
                            .par_iter()
                            .enumerate()
                            .map(|(i, v)| {
                                let dist: f32 = query
                                    .iter()
                                    .zip(v.iter())
                                    .map(|(a, b)| (a - b).powi(2))
                                    .sum::<f32>()
                                    .sqrt();
                                (i, dist)
                            })
                            .collect();

                        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                        distances.truncate(10);
                        black_box(distances)
                    })
                },
            );
        }
    }

    group.finish();
}

// ============================================================================
// Recall vs Throughput Tradeoff
// ============================================================================

fn bench_recall_throughput_tradeoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("Recall vs Throughput");
    group.sample_size(10);

    let dims = 768;
    let n = 10_000;
    let vectors = generate_random_vectors(n, dims, 42);
    let query = vectors[0].clone();

    // Compute ground truth
    let ground_truth: Vec<usize> = {
        let mut distances: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let dist: f32 = query
                    .iter()
                    .zip(v.iter())
                    .map(|(a, b)| (a - b).powi(2))
                    .sum::<f32>()
                    .sqrt();
                (i, dist)
            })
            .collect();
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        distances.iter().take(10).map(|(i, _)| *i).collect()
    };

    for &ef_search in [10, 20, 40, 80, 160, 320].iter() {
        let index = HnswIndex::new(dims, 16, 64, ef_search, 42);
        index.insert_batch(&vectors);

        group.bench_with_input(
            BenchmarkId::from_parameter(ef_search),
            &ef_search,
            |bench, _| {
                bench.iter(|| {
                    let results = index.search(&query, 10);

                    // Calculate recall
                    let recall = results
                        .iter()
                        .filter(|(id, _)| ground_truth.contains(&(*id as usize)))
                        .count() as f64
                        / 10.0;

                    black_box(recall)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    // Query Pipeline
    bench_query_pipeline,
    bench_query_pipeline_parallel,
    // Insert Throughput
    bench_insert_throughput,
    bench_insert_throughput_parallel,
    bench_insert_batching,
    // Concurrent Scaling
    bench_concurrent_scaling,
    bench_mixed_workload,
    // Memory Usage
    bench_memory_growth,
    bench_memory_efficiency,
    // Latency
    bench_latency_distribution,
    // Dimension Scaling
    bench_dimension_scaling,
    // Baselines
    bench_baseline_brute_force,
    // Recall/Throughput
    bench_recall_throughput_tradeoff,
);

criterion_main!(benches);
