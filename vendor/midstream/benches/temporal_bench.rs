//! Comprehensive benchmarks for temporal-compare crate
//!
//! Benchmarks cover:
//! - DTW (Dynamic Time Warping) performance across various sequence lengths
//! - LCS (Longest Common Subsequence) performance
//! - Edit distance calculations
//! - Cache hit/miss scenarios
//! - Memory allocation patterns
//!
//! Performance targets:
//! - DTW n=100: <10ms
//! - LCS n=100: <5ms
//! - Edit distance n=100: <3ms

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_temporal_compare::{
    TemporalCompare, TemporalData, TemporalPattern, CachedCompare,
    dtw::dtw_distance,
    lcs::longest_common_subsequence,
    edit::edit_distance,
};

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_sequence(len: usize, pattern: &str) -> Vec<f64> {
    match pattern {
        "linear" => (0..len).map(|i| i as f64).collect(),
        "sine" => (0..len).map(|i| (i as f64 * 0.1).sin()).collect(),
        "random" => (0..len).map(|i| (i as f64 * 7919.0) % 100.0).collect(),
        "stepped" => (0..len).map(|i| (i / 10) as f64).collect(),
        _ => vec![0.0; len],
    }
}

fn generate_similar_sequence(base: &[f64], similarity: f64) -> Vec<f64> {
    base.iter()
        .enumerate()
        .map(|(i, &x)| {
            let noise = ((i as f64 * 31.0) % 1.0 - 0.5) * 2.0;
            x + noise * (1.0 - similarity)
        })
        .collect()
}

fn generate_string_sequence(len: usize, alphabet_size: usize) -> Vec<char> {
    (0..len)
        .map(|i| {
            let idx = (i * 7919) % alphabet_size;
            (b'a' + idx as u8) as char
        })
        .collect()
}

// ============================================================================
// DTW Benchmarks
// ============================================================================

fn bench_dtw_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dtw_performance");

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        // Linear sequences
        group.bench_with_input(
            BenchmarkId::new("linear", size),
            size,
            |b, &size| {
                let seq1 = generate_sequence(size, "linear");
                let seq2 = generate_similar_sequence(&seq1, 0.9);
                b.iter(|| {
                    black_box(dtw_distance(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );

        // Sine wave sequences
        group.bench_with_input(
            BenchmarkId::new("sine", size),
            size,
            |b, &size| {
                let seq1 = generate_sequence(size, "sine");
                let seq2 = generate_similar_sequence(&seq1, 0.9);
                b.iter(|| {
                    black_box(dtw_distance(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );

        // Random sequences (worst case)
        group.bench_with_input(
            BenchmarkId::new("random", size),
            size,
            |b, &size| {
                let seq1 = generate_sequence(size, "random");
                let seq2 = generate_sequence(size, "random");
                b.iter(|| {
                    black_box(dtw_distance(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_dtw_similarity_variations(c: &mut Criterion) {
    let mut group = c.benchmark_group("dtw_similarity");

    let base_seq = generate_sequence(100, "sine");

    for similarity in [0.5, 0.7, 0.9, 0.95, 0.99].iter() {
        group.bench_with_input(
            BenchmarkId::new("similarity", (similarity * 100.0) as u32),
            similarity,
            |b, &sim| {
                let seq2 = generate_similar_sequence(&base_seq, sim);
                b.iter(|| {
                    black_box(dtw_distance(
                        black_box(&base_seq),
                        black_box(&seq2)
                    ))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// LCS Benchmarks
// ============================================================================

fn bench_lcs_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("lcs_performance");

    for size in [10, 50, 100, 500, 1000].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("identical", size),
            size,
            |b, &size| {
                let seq = generate_string_sequence(size, 26);
                b.iter(|| {
                    black_box(longest_common_subsequence(
                        black_box(&seq),
                        black_box(&seq)
                    ))
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("similar", size),
            size,
            |b, &size| {
                let seq1 = generate_string_sequence(size, 26);
                let seq2 = generate_string_sequence(size + 10, 26);
                b.iter(|| {
                    black_box(longest_common_subsequence(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("different", size),
            size,
            |b, &size| {
                let seq1 = generate_string_sequence(size, 4);
                let seq2 = generate_string_sequence(size, 4);
                b.iter(|| {
                    black_box(longest_common_subsequence(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Edit Distance Benchmarks
// ============================================================================

fn bench_edit_distance_various_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("edit_distance_performance");

    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("small_alphabet", size),
            size,
            |b, &size| {
                let seq1 = generate_string_sequence(size, 4);
                let seq2 = generate_string_sequence(size + 5, 4);
                b.iter(|| {
                    black_box(edit_distance(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("large_alphabet", size),
            size,
            |b, &size| {
                let seq1 = generate_string_sequence(size, 26);
                let seq2 = generate_string_sequence(size + 5, 26);
                b.iter(|| {
                    black_box(edit_distance(
                        black_box(&seq1),
                        black_box(&seq2)
                    ))
                });
            }
        );
    }

    group.finish();
}

fn bench_edit_distance_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("edit_distance_operations");

    let base = generate_string_sequence(100, 26);

    // Insertion heavy
    group.bench_function("insertions", |b| {
        let mut modified = base.clone();
        for i in (0..20).rev() {
            modified.insert(i * 5, 'X');
        }
        b.iter(|| {
            black_box(edit_distance(
                black_box(&base),
                black_box(&modified)
            ))
        });
    });

    // Deletion heavy
    group.bench_function("deletions", |b| {
        let mut modified = base.clone();
        for _ in 0..20 {
            if !modified.is_empty() {
                modified.remove(modified.len() / 2);
            }
        }
        b.iter(|| {
            black_box(edit_distance(
                black_box(&base),
                black_box(&modified)
            ))
        });
    });

    // Substitution heavy
    group.bench_function("substitutions", |b| {
        let mut modified = base.clone();
        for i in (0..20).map(|x| x * 5) {
            if i < modified.len() {
                modified[i] = 'Z';
            }
        }
        b.iter(|| {
            black_box(edit_distance(
                black_box(&base),
                black_box(&modified)
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Cache Benchmarks
// ============================================================================

fn bench_cache_scenarios(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_performance");

    // Cache hit scenario
    group.bench_function("cache_hit", |b| {
        let mut cache = CachedCompare::new(1000);
        let seq1 = generate_sequence(100, "sine");
        let seq2 = generate_similar_sequence(&seq1, 0.9);

        // Warm up cache
        cache.compare_dtw(&seq1, &seq2);

        b.iter(|| {
            black_box(cache.compare_dtw(
                black_box(&seq1),
                black_box(&seq2)
            ))
        });
    });

    // Cache miss scenario
    group.bench_function("cache_miss", |b| {
        let mut cache = CachedCompare::new(10);
        let sequences: Vec<_> = (0..100)
            .map(|i| generate_sequence(100, if i % 2 == 0 { "sine" } else { "linear" }))
            .collect();

        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % sequences.len();
            let idx2 = (idx + 1) % sequences.len();
            black_box(cache.compare_dtw(
                black_box(&sequences[idx]),
                black_box(&sequences[idx2])
            ))
        });
    });

    // Cache eviction scenario
    group.bench_function("cache_eviction", |b| {
        let mut cache = CachedCompare::new(50);
        let sequences: Vec<_> = (0..100)
            .map(|i| generate_sequence(100, "sine"))
            .collect();

        let mut idx = 0;
        b.iter(|| {
            idx = (idx + 1) % sequences.len();
            let idx2 = (idx + 1) % sequences.len();
            black_box(cache.compare_dtw(
                black_box(&sequences[idx]),
                black_box(&sequences[idx2])
            ))
        });
    });

    group.finish();
}

// ============================================================================
// Memory Allocation Benchmarks
// ============================================================================

fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");

    // Small allocations
    group.bench_function("small_sequences", |b| {
        b.iter(|| {
            let seq1 = generate_sequence(10, "sine");
            let seq2 = generate_sequence(10, "linear");
            black_box(dtw_distance(&seq1, &seq2))
        });
    });

    // Large allocations
    group.bench_function("large_sequences", |b| {
        b.iter(|| {
            let seq1 = generate_sequence(1000, "sine");
            let seq2 = generate_sequence(1000, "linear");
            black_box(dtw_distance(&seq1, &seq2))
        });
    });

    // Repeated allocations
    group.bench_function("repeated_allocations", |b| {
        b.iter(|| {
            for _ in 0..10 {
                let seq1 = generate_sequence(100, "sine");
                let seq2 = generate_sequence(100, "linear");
                black_box(dtw_distance(&seq1, &seq2));
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = dtw_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10))
        .warm_up_time(std::time::Duration::from_secs(3));
    targets = bench_dtw_various_sizes, bench_dtw_similarity_variations
}

criterion_group! {
    name = lcs_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_lcs_various_sizes
}

criterion_group! {
    name = edit_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(std::time::Duration::from_secs(10));
    targets = bench_edit_distance_various_sizes, bench_edit_distance_operations
}

criterion_group! {
    name = cache_benches;
    config = Criterion::default()
        .sample_size(200)
        .measurement_time(std::time::Duration::from_secs(5));
    targets = bench_cache_scenarios
}

criterion_group! {
    name = memory_benches;
    config = Criterion::default()
        .sample_size(100);
    targets = bench_memory_patterns
}

criterion_main!(
    dtw_benches,
    lcs_benches,
    edit_benches,
    cache_benches,
    memory_benches
);
