//! Benchmarks for quantized vector distance calculations
//!
//! Compares scalar vs SIMD implementations for all quantized types

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_postgres::types::{BinaryVec, ProductVec, ScalarVec};

// ============================================================================
// BinaryVec Benchmarks
// ============================================================================

fn bench_binaryvec_hamming(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaryvec_hamming");

    for dims in [128, 512, 1024, 2048, 4096].iter() {
        let a_data: Vec<f32> = (0..*dims)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let b_data: Vec<f32> = (0..*dims)
            .map(|i| if i % 3 == 0 { 1.0 } else { -1.0 })
            .collect();

        let a = BinaryVec::from_f32(&a_data);
        let b = BinaryVec::from_f32(&b_data);

        group.bench_with_input(BenchmarkId::new("simd", dims), dims, |bencher, _| {
            bencher.iter(|| black_box(a.hamming_distance(&b)));
        });
    }

    group.finish();
}

fn bench_binaryvec_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("binaryvec_quantization");

    for dims in [128, 512, 1024, 2048, 4096].iter() {
        let data: Vec<f32> = (0..*dims).map(|i| (i as f32) * 0.01).collect();

        group.bench_with_input(BenchmarkId::new("from_f32", dims), dims, |bencher, _| {
            bencher.iter(|| black_box(BinaryVec::from_f32(&data)));
        });
    }

    group.finish();
}

// ============================================================================
// ScalarVec Benchmarks
// ============================================================================

fn bench_scalarvec_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalarvec_distance");

    for dims in [128, 512, 1024, 2048, 4096].iter() {
        let a_data: Vec<f32> = (0..*dims).map(|i| i as f32 * 0.1).collect();
        let b_data: Vec<f32> = (0..*dims).map(|i| (*dims - i) as f32 * 0.1).collect();

        let a = ScalarVec::from_f32(&a_data);
        let b = ScalarVec::from_f32(&b_data);

        group.bench_with_input(BenchmarkId::new("simd", dims), dims, |bencher, _| {
            bencher.iter(|| black_box(a.distance(&b)));
        });
    }

    group.finish();
}

fn bench_scalarvec_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalarvec_quantization");

    for dims in [128, 512, 1024, 2048, 4096].iter() {
        let data: Vec<f32> = (0..*dims).map(|i| (i as f32) * 0.01).collect();

        group.bench_with_input(BenchmarkId::new("from_f32", dims), dims, |bencher, _| {
            bencher.iter(|| black_box(ScalarVec::from_f32(&data)));
        });

        let scalar = ScalarVec::from_f32(&data);
        group.bench_with_input(BenchmarkId::new("to_f32", dims), dims, |bencher, _| {
            bencher.iter(|| black_box(scalar.to_f32()));
        });
    }

    group.finish();
}

// ============================================================================
// ProductVec Benchmarks
// ============================================================================

fn bench_productvec_adc_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("productvec_adc_distance");

    for m in [8u8, 16, 32, 48, 64].iter() {
        let k: usize = 256;
        let codes: Vec<u8> = (0..*m).map(|i| ((i * 7) % k as u8) as u8).collect();
        let pq = ProductVec::new((*m as usize * 32) as u16, *m, 255, codes);

        // Create distance table
        let mut table = Vec::with_capacity(*m as usize * k as usize);
        for i in 0..(*m as usize * k as usize) {
            table.push((i % 100) as f32 * 0.01);
        }

        group.bench_with_input(BenchmarkId::new("simd", m), m, |bencher, _| {
            bencher.iter(|| black_box(pq.adc_distance_simd(&table)));
        });

        group.bench_with_input(BenchmarkId::new("flat", m), m, |bencher, _| {
            bencher.iter(|| black_box(pq.adc_distance_flat(&table)));
        });
    }

    group.finish();
}

// ============================================================================
// Compression Benchmarks
// ============================================================================

fn bench_compression_ratios(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    let dims = 1536; // OpenAI embedding size
    let data: Vec<f32> = (0..dims).map(|i| (i as f32) * 0.001).collect();

    // Original size
    let original_size = dims * std::mem::size_of::<f32>();

    group.bench_function("binary_quantize", |bencher| {
        bencher.iter(|| {
            let binary = black_box(BinaryVec::from_f32(&data));
            let ratio = original_size as f32 / binary.memory_size() as f32;
            black_box(ratio)
        });
    });

    group.bench_function("scalar_quantize", |bencher| {
        bencher.iter(|| {
            let scalar = black_box(ScalarVec::from_f32(&data));
            let ratio = original_size as f32 / scalar.memory_size() as f32;
            black_box(ratio)
        });
    });

    group.bench_function("product_quantize", |bencher| {
        bencher.iter(|| {
            let pq = black_box(ProductVec::new(dims as u16, 48, 255, vec![0; 48]));
            let ratio = original_size as f32 / pq.memory_size() as f32;
            black_box(ratio)
        });
    });

    group.finish();
}

// ============================================================================
// Throughput Benchmarks
// ============================================================================

fn bench_throughput_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    let dims = 1024;
    let num_vectors = 1000;

    // Generate test data
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| (0..dims).map(|j| ((i * dims + j) as f32) * 0.001).collect())
        .collect();

    let query = vectors[0].clone();

    // Quantize all vectors
    let binary_vecs: Vec<BinaryVec> = vectors.iter().map(|v| BinaryVec::from_f32(v)).collect();
    let scalar_vecs: Vec<ScalarVec> = vectors.iter().map(|v| ScalarVec::from_f32(v)).collect();

    let query_binary = BinaryVec::from_f32(&query);
    let query_scalar = ScalarVec::from_f32(&query);

    group.bench_function("binary_scan", |bencher| {
        bencher.iter(|| {
            let mut total_dist = 0u32;
            for v in &binary_vecs {
                total_dist += black_box(query_binary.hamming_distance(v));
            }
            black_box(total_dist)
        });
    });

    group.bench_function("scalar_scan", |bencher| {
        bencher.iter(|| {
            let mut total_dist = 0.0f32;
            for v in &scalar_vecs {
                total_dist += black_box(query_scalar.distance(v));
            }
            black_box(total_dist)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_binaryvec_hamming,
    bench_binaryvec_quantization,
    bench_scalarvec_distance,
    bench_scalarvec_quantization,
    bench_productvec_adc_distance,
    bench_compression_ratios,
    bench_throughput_comparison,
);

criterion_main!(benches);
