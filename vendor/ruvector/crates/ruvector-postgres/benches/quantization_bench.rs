//! Comprehensive quantization benchmarks
//!
//! Compares exact vs quantized search with different quantization methods

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use ruvector_postgres::distance::DistanceMetric;
use ruvector_postgres::types::{BinaryVec, ProductVec, RuVector, ScalarVec};

// ============================================================================
// Test Data Generation
// ============================================================================

fn generate_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    (0..n)
        .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
        .collect()
}

// ============================================================================
// Scalar Quantization (SQ8) Benchmarks
// ============================================================================

fn bench_sq8_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("sq8_quantization");

    for dims in [128, 384, 768, 1536, 3072].iter() {
        let data: Vec<f32> = (0..*dims).map(|i| (i as f32) * 0.001).collect();

        group.bench_with_input(BenchmarkId::new("encode", dims), dims, |bench, _| {
            bench.iter(|| black_box(ScalarVec::from_f32(&data)));
        });

        let encoded = ScalarVec::from_f32(&data);
        group.bench_with_input(BenchmarkId::new("decode", dims), dims, |bench, _| {
            bench.iter(|| black_box(encoded.to_f32()));
        });
    }

    group.finish();
}

fn bench_sq8_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("sq8_distance");

    for dims in [128, 384, 768, 1536, 3072].iter() {
        let a_data: Vec<f32> = (0..*dims).map(|i| i as f32 * 0.1).collect();
        let b_data: Vec<f32> = (0..*dims).map(|i| (*dims - i) as f32 * 0.1).collect();

        let a_exact = RuVector::from_slice(&a_data);
        let b_exact = RuVector::from_slice(&b_data);

        let a_sq8 = ScalarVec::from_f32(&a_data);
        let b_sq8 = ScalarVec::from_f32(&b_data);

        group.bench_with_input(BenchmarkId::new("exact", dims), dims, |bench, _| {
            bench.iter(|| black_box(a_exact.dot(&b_exact)));
        });

        group.bench_with_input(BenchmarkId::new("quantized", dims), dims, |bench, _| {
            bench.iter(|| black_box(a_sq8.distance(&b_sq8)));
        });
    }

    group.finish();
}

fn bench_sq8_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("sq8_search");

    for dims in [128, 768, 1536].iter() {
        let n = 10000;
        let vectors = generate_vectors(n, *dims, 42);
        let query = generate_vectors(1, *dims, 999)[0].clone();

        // Exact search
        let exact_vecs: Vec<RuVector> = vectors.iter().map(|v| RuVector::from_slice(v)).collect();

        let exact_query = RuVector::from_slice(&query);

        group.bench_with_input(BenchmarkId::new("exact", dims), dims, |bench, _| {
            bench.iter(|| {
                let mut distances: Vec<(usize, f32)> = exact_vecs
                    .iter()
                    .enumerate()
                    .map(|(id, vec)| {
                        let dist = exact_query.dot(vec);
                        (id, -dist) // Negative for max inner product
                    })
                    .collect();

                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let top_k: Vec<_> = distances[..10].to_vec();
                black_box(top_k)
            });
        });

        // Quantized search
        let sq8_vecs: Vec<ScalarVec> = vectors.iter().map(|v| ScalarVec::from_f32(v)).collect();

        let sq8_query = ScalarVec::from_f32(&query);

        group.bench_with_input(BenchmarkId::new("quantized", dims), dims, |bench, _| {
            bench.iter(|| {
                let mut distances: Vec<(usize, f32)> = sq8_vecs
                    .iter()
                    .enumerate()
                    .map(|(id, vec)| (id, sq8_query.distance(vec)))
                    .collect();

                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let top_k: Vec<_> = distances[..10].to_vec();
                black_box(top_k)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Binary Quantization Benchmarks
// ============================================================================

fn bench_binary_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_quantization");

    for dims in [128, 512, 1024, 2048, 4096].iter() {
        let data: Vec<f32> = (0..*dims)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        group.bench_with_input(BenchmarkId::new("encode", dims), dims, |bench, _| {
            bench.iter(|| black_box(BinaryVec::from_f32(&data)));
        });
    }

    group.finish();
}

fn bench_binary_hamming(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_hamming");

    for dims in [128, 512, 1024, 2048, 4096, 8192].iter() {
        let a_data: Vec<f32> = (0..*dims)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let b_data: Vec<f32> = (0..*dims)
            .map(|i| if i % 3 == 0 { 1.0 } else { -1.0 })
            .collect();

        let a = BinaryVec::from_f32(&a_data);
        let b = BinaryVec::from_f32(&b_data);

        group.bench_with_input(BenchmarkId::new("simd", dims), dims, |bench, _| {
            bench.iter(|| black_box(a.hamming_distance(&b)));
        });
    }

    group.finish();
}

fn bench_binary_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_search");

    for dims in [1024, 2048, 4096].iter() {
        let n = 100000;
        let vectors = generate_vectors(n, *dims, 42);
        let query = generate_vectors(1, *dims, 999)[0].clone();

        let binary_vecs: Vec<BinaryVec> = vectors.iter().map(|v| BinaryVec::from_f32(v)).collect();

        let binary_query = BinaryVec::from_f32(&query);

        group.bench_with_input(BenchmarkId::new("scan", dims), dims, |bench, _| {
            bench.iter(|| {
                let mut distances: Vec<(usize, u32)> = binary_vecs
                    .iter()
                    .enumerate()
                    .map(|(id, vec)| (id, binary_query.hamming_distance(vec)))
                    .collect();

                distances.sort_by_key(|k| k.1);
                let top_k: Vec<_> = distances[..10].to_vec();
                black_box(top_k)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Product Quantization (PQ) Benchmarks
// ============================================================================

fn bench_pq_adc_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("pq_adc_distance");

    for m in [8u8, 16, 32, 48, 64].iter() {
        let k: usize = 256; // Number of centroids
        let codes: Vec<u8> = (0..*m).map(|i| ((i * 7) % k as u8) as u8).collect();
        let pq = ProductVec::new((*m as usize * 32) as u16, *m, 255, codes);

        // Create distance table
        let mut table = Vec::with_capacity(*m as usize * k as usize);
        for i in 0..(*m as usize * k as usize) {
            table.push((i % 100) as f32 * 0.01);
        }

        group.bench_with_input(BenchmarkId::new("simd", m), m, |bench, _| {
            bench.iter(|| black_box(pq.adc_distance_simd(&table)));
        });

        group.bench_with_input(BenchmarkId::new("flat", m), m, |bench, _| {
            bench.iter(|| black_box(pq.adc_distance_flat(&table)));
        });
    }

    group.finish();
}

// ============================================================================
// Compression Ratio Benchmarks
// ============================================================================

fn bench_compression_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression_ratio");

    for dims in [384, 768, 1536, 3072].iter() {
        let data: Vec<f32> = (0..*dims).map(|i| (i as f32) * 0.001).collect();
        let original_size = dims * std::mem::size_of::<f32>();

        group.bench_with_input(BenchmarkId::new("binary", dims), dims, |bench, _| {
            bench.iter(|| {
                let binary = black_box(BinaryVec::from_f32(&data));
                let compressed = binary.memory_size();
                let ratio = original_size as f32 / compressed as f32;
                black_box(ratio)
            });
        });

        group.bench_with_input(BenchmarkId::new("scalar", dims), dims, |bench, _| {
            bench.iter(|| {
                let scalar = black_box(ScalarVec::from_f32(&data));
                let compressed = scalar.memory_size();
                let ratio = original_size as f32 / compressed as f32;
                black_box(ratio)
            });
        });

        group.bench_with_input(BenchmarkId::new("product", dims), dims, |bench, _| {
            bench.iter(|| {
                let m = (dims / 32).min(64);
                let pq = black_box(ProductVec::new(*dims as u16, m as u8, 255, vec![0; m]));
                let compressed = pq.memory_size();
                let ratio = original_size as f32 / compressed as f32;
                black_box(ratio)
            });
        });
    }

    group.finish();
}

// ============================================================================
// Speedup vs Accuracy Trade-off
// ============================================================================

fn bench_quantization_tradeoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization_tradeoff");
    group.sample_size(10);

    let dims = 768;
    let n = 10000;
    let num_queries = 100;

    let vectors = generate_vectors(n, dims, 42);
    let queries = generate_vectors(num_queries, dims, 999);

    // Compute ground truth
    let exact_vecs: Vec<RuVector> = vectors.iter().map(|v| RuVector::from_slice(v)).collect();

    let ground_truth: Vec<Vec<usize>> = queries
        .iter()
        .map(|query| {
            let query_vec = RuVector::from_slice(query);
            let mut distances: Vec<(usize, f32)> = exact_vecs
                .iter()
                .enumerate()
                .map(|(id, vec)| {
                    let diff = query_vec.sub(vec);
                    let dist = diff.norm();
                    (id, dist)
                })
                .collect();

            distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            distances.iter().take(10).map(|(id, _)| *id).collect()
        })
        .collect();

    // Benchmark SQ8
    let sq8_vecs: Vec<ScalarVec> = vectors.iter().map(|v| ScalarVec::from_f32(v)).collect();

    group.bench_function("sq8_speedup", |bench| {
        bench.iter(|| {
            for (i, query) in queries.iter().enumerate() {
                let sq8_query = ScalarVec::from_f32(query);
                let mut distances: Vec<(usize, f32)> = sq8_vecs
                    .iter()
                    .enumerate()
                    .map(|(id, vec)| (id, sq8_query.distance(vec)))
                    .collect();

                distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
                let results: Vec<usize> = distances.iter().take(10).map(|(id, _)| *id).collect();

                // Compute recall
                let hits = results
                    .iter()
                    .filter(|id| ground_truth[i].contains(id))
                    .count();

                black_box(hits as f32 / 10.0);
            }
        });
    });

    // Benchmark Binary
    let binary_vecs: Vec<BinaryVec> = vectors.iter().map(|v| BinaryVec::from_f32(v)).collect();

    group.bench_function("binary_speedup", |bench| {
        bench.iter(|| {
            for (i, query) in queries.iter().enumerate() {
                let binary_query = BinaryVec::from_f32(query);
                let mut distances: Vec<(usize, u32)> = binary_vecs
                    .iter()
                    .enumerate()
                    .map(|(id, vec)| (id, binary_query.hamming_distance(vec)))
                    .collect();

                distances.sort_by_key(|k| k.1);
                let results: Vec<usize> = distances.iter().take(10).map(|(id, _)| *id).collect();

                // Compute recall
                let hits = results
                    .iter()
                    .filter(|id| ground_truth[i].contains(id))
                    .count();

                black_box(hits as f32 / 10.0);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Throughput Comparison
// ============================================================================

fn bench_quantization_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization_throughput");

    let dims = 1536;
    let n = 100000;

    let vectors = generate_vectors(n, dims, 42);
    let query = generate_vectors(1, dims, 999)[0].clone();

    // Exact
    let exact_vecs: Vec<RuVector> = vectors.iter().map(|v| RuVector::from_slice(v)).collect();
    let exact_query = RuVector::from_slice(&query);

    group.bench_function("exact_scan", |bench| {
        bench.iter(|| {
            let mut total = 0.0f32;
            for vec in &exact_vecs {
                total += exact_query.dot(vec);
            }
            black_box(total)
        });
    });

    // SQ8
    let sq8_vecs: Vec<ScalarVec> = vectors.iter().map(|v| ScalarVec::from_f32(v)).collect();
    let sq8_query = ScalarVec::from_f32(&query);

    group.bench_function("sq8_scan", |bench| {
        bench.iter(|| {
            let mut total = 0.0f32;
            for vec in &sq8_vecs {
                total += sq8_query.distance(vec);
            }
            black_box(total)
        });
    });

    // Binary
    let binary_vecs: Vec<BinaryVec> = vectors.iter().map(|v| BinaryVec::from_f32(v)).collect();
    let binary_query = BinaryVec::from_f32(&query);

    group.bench_function("binary_scan", |bench| {
        bench.iter(|| {
            let mut total = 0u64;
            for vec in &binary_vecs {
                total += binary_query.hamming_distance(vec) as u64;
            }
            black_box(total)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_sq8_quantization,
    bench_sq8_distance,
    bench_sq8_search,
    bench_binary_quantization,
    bench_binary_hamming,
    bench_binary_search,
    bench_pq_adc_distance,
    bench_compression_comparison,
    bench_quantization_tradeoff,
    bench_quantization_throughput,
);

criterion_main!(benches);
