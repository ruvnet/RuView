//! HNSW Benchmark Suite for 7sense
//!
//! Performance targets from ADR-004:
//! - HNSW Search: 150x speedup vs brute force
//! - Query Latency p99: < 50ms
//! - Recall@10: >= 0.95
//! - Recall@100: >= 0.98
//! - Insert Throughput: >= 10,000 vectors/s
//! - Build Time: < 30 min for 1M vectors

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::time::{Duration, Instant};

use sevensense_benches::*;

/// Index sizes to benchmark
const SMALL_INDEX: usize = 10_000;
const MEDIUM_INDEX: usize = 100_000;
const LARGE_INDEX: usize = 500_000;

/// K values for search benchmarks
const K_VALUES: &[usize] = &[10, 50, 100];

// ============================================================================
// HNSW Search Benchmarks
// ============================================================================

/// Benchmark HNSW search performance with different index sizes and k values
fn benchmark_hnsw_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    // Generate query vectors once
    let queries = generate_random_vectors(100, PERCH_EMBEDDING_DIM);

    for &size in &[SMALL_INDEX, MEDIUM_INDEX] {
        // Build index
        println!("Building index with {} vectors...", size);
        let index = setup_test_index(size);

        for &k in K_VALUES {
            group.throughput(Throughput::Elements(queries.len() as u64));
            group.bench_with_input(
                BenchmarkId::new(format!("size_{}_k_{}", size, k), k),
                &k,
                |b, &k| {
                    b.iter(|| {
                        for query in &queries {
                            black_box(index.search(query, k));
                        }
                    });
                },
            );
        }
    }

    group.finish();
}

/// Benchmark HNSW search with different ef_search values
fn benchmark_hnsw_search_ef(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_search_ef");
    group.sample_size(30);
    group.measurement_time(Duration::from_secs(8));

    let size = MEDIUM_INDEX;
    let mut index = setup_test_index(size);
    let queries = generate_random_vectors(50, PERCH_EMBEDDING_DIM);
    let k = 10;

    for ef in [64, 128, 256, 512] {
        index.set_ef_search(ef);

        group.throughput(Throughput::Elements(queries.len() as u64));
        group.bench_with_input(BenchmarkId::new("ef", ef), &ef, |b, _| {
            b.iter(|| {
                for query in &queries {
                    black_box(index.search(query, k));
                }
            });
        });
    }

    group.finish();
}

/// Benchmark HNSW vs brute force to calculate speedup ratio
fn benchmark_hnsw_vs_brute_force(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_vs_brute_force");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));

    // Use smaller index for brute force comparison
    let size = 10_000;
    let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);
    let mut index = SimpleHnswIndex::new_default();

    for vec in &vectors {
        index.add(vec.clone());
    }

    let queries = generate_random_vectors(20, PERCH_EMBEDDING_DIM);
    let k = 10;

    // Benchmark brute force
    group.bench_function("brute_force", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(brute_force_knn(query, &vectors, k));
            }
        });
    });

    // Benchmark HNSW
    group.bench_function("hnsw", |b| {
        b.iter(|| {
            for query in &queries {
                black_box(index.search(query, k));
            }
        });
    });

    group.finish();
}

// ============================================================================
// HNSW Insert Benchmarks
// ============================================================================

/// Benchmark single vector insertion
fn benchmark_hnsw_insert_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_insert_single");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(10));

    // Benchmark insertion into indices of different sizes
    for &initial_size in &[1000, 10_000, 50_000] {
        let vectors_to_insert = generate_random_vectors(100, PERCH_EMBEDDING_DIM);

        group.bench_with_input(
            BenchmarkId::new("initial_size", initial_size),
            &initial_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        // Setup: create index with initial vectors
                        setup_test_index(size)
                    },
                    |mut index| {
                        // Insert new vectors
                        for vec in &vectors_to_insert {
                            black_box(index.add(vec.clone()));
                        }
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

/// Benchmark batch vector insertion
fn benchmark_hnsw_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_insert_batch");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(15));

    for &batch_size in &[100, 1000, 5000] {
        let vectors = generate_random_vectors(batch_size, PERCH_EMBEDDING_DIM);

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch_size", batch_size),
            &batch_size,
            |b, _| {
                b.iter_batched(
                    || {
                        // Setup: create empty index
                        SimpleHnswIndex::new_default()
                    },
                    |mut index| {
                        // Insert batch
                        black_box(index.batch_add(vectors.clone()));
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }

    group.finish();
}

// ============================================================================
// HNSW Build Benchmarks
// ============================================================================

/// Benchmark index construction time
fn benchmark_hnsw_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_build");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(30));

    for &size in &[1000, 5000, 10_000] {
        let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);

        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("vectors", size), &size, |b, _| {
            b.iter(|| {
                let mut index = SimpleHnswIndex::new_default();
                for vec in &vectors {
                    index.add(vec.clone());
                }
                black_box(index)
            });
        });
    }

    group.finish();
}

/// Benchmark index construction with different M parameters
fn benchmark_hnsw_build_m_param(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_build_m_param");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(20));

    let size = 5000;
    let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);

    for m in [16, 24, 32, 48] {
        group.bench_with_input(BenchmarkId::new("M", m), &m, |b, &m| {
            b.iter(|| {
                let mut index =
                    SimpleHnswIndex::new(PERCH_EMBEDDING_DIM, m, DEFAULT_EF_CONSTRUCTION, DEFAULT_EF_SEARCH);
                for vec in &vectors {
                    index.add(vec.clone());
                }
                black_box(index)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Recall Measurement
// ============================================================================

/// Measure and report recall metrics (not a benchmark, but a validation)
fn measure_recall(c: &mut Criterion) {
    let mut group = c.benchmark_group("recall_measurement");
    group.sample_size(10);

    let size = 10_000;
    let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);
    let mut index = SimpleHnswIndex::new_default();

    for vec in &vectors {
        index.add(vec.clone());
    }

    let queries = generate_random_vectors(100, PERCH_EMBEDDING_DIM);

    // This benchmark measures time to compute recall (including brute force)
    group.bench_function("recall_computation", |b| {
        b.iter(|| {
            let mut total_recall_10 = 0.0;
            let mut total_recall_100 = 0.0;

            for query in &queries {
                let hnsw_results = index.search(query, 100);
                let ground_truth = brute_force_knn(query, &vectors, 100);

                total_recall_10 += measure_recall_at_k(&hnsw_results, &ground_truth, 10);
                total_recall_100 += measure_recall_at_k(&hnsw_results, &ground_truth, 100);
            }

            let avg_recall_10 = total_recall_10 / queries.len() as f32;
            let avg_recall_100 = total_recall_100 / queries.len() as f32;

            black_box((avg_recall_10, avg_recall_100))
        });
    });

    group.finish();
}

// ============================================================================
// Speedup Ratio Calculation
// ============================================================================

/// Calculate and report the speedup ratio of HNSW vs brute force
/// This is run as a single iteration with detailed output
fn calculate_speedup_ratio() {
    println!("\n=== HNSW vs Brute Force Speedup Analysis ===\n");

    for &size in &[1_000, 5_000, 10_000, 50_000] {
        println!("Index size: {} vectors", size);
        println!("Dimension: {}", PERCH_EMBEDDING_DIM);

        let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);
        let mut index = SimpleHnswIndex::new_default();

        for vec in &vectors {
            index.add(vec.clone());
        }

        let queries = generate_random_vectors(100, PERCH_EMBEDDING_DIM);
        let k = 10;

        // Time brute force
        let bf_start = Instant::now();
        for query in &queries {
            let _ = brute_force_knn(query, &vectors, k);
        }
        let bf_time = bf_start.elapsed();

        // Time HNSW
        let hnsw_start = Instant::now();
        for query in &queries {
            let _ = index.search(query, k);
        }
        let hnsw_time = hnsw_start.elapsed();

        let speedup = bf_time.as_secs_f64() / hnsw_time.as_secs_f64();

        // Calculate recall
        let mut total_recall = 0.0;
        for query in &queries {
            let hnsw_results = index.search(query, k);
            let ground_truth = brute_force_knn(query, &vectors, k);
            total_recall += measure_recall_at_k(&hnsw_results, &ground_truth, k);
        }
        let avg_recall = total_recall / queries.len() as f32;

        println!("  Brute Force: {:?} ({} queries)", bf_time, queries.len());
        println!("  HNSW:        {:?} ({} queries)", hnsw_time, queries.len());
        println!("  Speedup:     {:.1}x", speedup);
        println!("  Recall@{}:   {:.3}", k, avg_recall);
        println!(
            "  Target:      {}x speedup ({})",
            targets::HNSW_SPEEDUP_VS_BRUTE_FORCE,
            if speedup >= targets::HNSW_SPEEDUP_VS_BRUTE_FORCE {
                "PASS"
            } else {
                "FAIL"
            }
        );
        println!();
    }
}

// ============================================================================
// Latency Distribution Analysis
// ============================================================================

/// Analyze query latency distribution
fn analyze_latency_distribution() {
    println!("\n=== Query Latency Distribution Analysis ===\n");

    let size = MEDIUM_INDEX;
    println!("Building index with {} vectors...", size);
    let index = setup_test_index(size);

    let queries = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);
    let k = 10;

    let mut latencies = Vec::with_capacity(queries.len());

    for query in &queries {
        let start = Instant::now();
        let _ = index.search(query, k);
        latencies.push(start.elapsed());
    }

    let stats = PerformanceStats::from_latencies(latencies);

    println!("Query latency statistics (k={}, {} queries):", k, queries.len());
    println!("{}", stats.report());
    println!();
    println!("Performance targets:");
    println!(
        "  p50 target:  {}ms ({})",
        targets::QUERY_LATENCY_P50_MS,
        if stats.p50 <= Duration::from_millis(targets::QUERY_LATENCY_P50_MS) {
            "PASS"
        } else {
            "FAIL"
        }
    );
    println!(
        "  p99 target:  {}ms ({})",
        targets::QUERY_LATENCY_P99_MS,
        if stats.p99 <= Duration::from_millis(targets::QUERY_LATENCY_P99_MS) {
            "PASS"
        } else {
            "FAIL"
        }
    );
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = search_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_hnsw_search, benchmark_hnsw_search_ef, benchmark_hnsw_vs_brute_force
);

criterion_group!(
    name = insert_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_hnsw_insert_single, benchmark_hnsw_insert_batch
);

criterion_group!(
    name = build_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_hnsw_build, benchmark_hnsw_build_m_param
);

criterion_group!(
    name = recall_benches;
    config = Criterion::default().with_output_color(true);
    targets = measure_recall
);

criterion_main!(search_benches, insert_benches, build_benches, recall_benches);

// ============================================================================
// Additional Analysis Functions (run separately)
// ============================================================================

#[cfg(test)]
mod analysis {
    use super::*;

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored --nocapture
    fn run_speedup_analysis() {
        calculate_speedup_ratio();
    }

    #[test]
    #[ignore]
    fn run_latency_analysis() {
        analyze_latency_distribution();
    }

    #[test]
    fn test_target_recall_at_10() {
        let size = 5_000;
        let vectors = generate_random_vectors(size, PERCH_EMBEDDING_DIM);
        let mut index = SimpleHnswIndex::new_default();

        for vec in &vectors {
            index.add(vec.clone());
        }

        let queries = generate_random_vectors(50, PERCH_EMBEDDING_DIM);

        let mut total_recall = 0.0;
        for query in &queries {
            let hnsw_results = index.search(query, 10);
            let ground_truth = brute_force_knn(query, &vectors, 10);
            total_recall += measure_recall_at_k(&hnsw_results, &ground_truth, 10);
        }

        let avg_recall = total_recall / queries.len() as f32;
        println!("Average Recall@10: {:.3}", avg_recall);
        assert!(
            avg_recall as f64 >= targets::RECALL_AT_10,
            "Recall@10 {} below target {}",
            avg_recall,
            targets::RECALL_AT_10
        );
    }

    #[test]
    fn test_insert_throughput() {
        let vectors = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);
        let mut index = SimpleHnswIndex::new_default();

        let start = Instant::now();
        for vec in &vectors {
            index.add(vec.clone());
        }
        let elapsed = start.elapsed();

        let throughput = vectors.len() as f64 / elapsed.as_secs_f64();
        println!("Insert throughput: {:.0} vectors/sec", throughput);

        // Note: This is a simplified index, real HNSW should achieve higher throughput
    }
}
