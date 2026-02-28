use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_core::quantization::*;

fn bench_scalar_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalar_quantization");

    for size in [128, 384, 768, 1536].iter() {
        let vector: Vec<f32> = (0..*size).map(|i| i as f32 * 0.1).collect();

        group.bench_with_input(BenchmarkId::new("encode", size), size, |bench, _| {
            bench.iter(|| ScalarQuantized::quantize(black_box(&vector)));
        });

        let quantized = ScalarQuantized::quantize(&vector);
        group.bench_with_input(BenchmarkId::new("decode", size), size, |bench, _| {
            bench.iter(|| quantized.reconstruct());
        });

        let quantized2 = ScalarQuantized::quantize(&vector);
        group.bench_with_input(BenchmarkId::new("distance", size), size, |bench, _| {
            bench.iter(|| quantized.distance(black_box(&quantized2)));
        });
    }

    group.finish();
}

fn bench_binary_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_quantization");

    for size in [128, 384, 768, 1536].iter() {
        let vector: Vec<f32> = (0..*size)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        group.bench_with_input(BenchmarkId::new("encode", size), size, |bench, _| {
            bench.iter(|| BinaryQuantized::quantize(black_box(&vector)));
        });

        let quantized = BinaryQuantized::quantize(&vector);
        group.bench_with_input(BenchmarkId::new("decode", size), size, |bench, _| {
            bench.iter(|| quantized.reconstruct());
        });

        let quantized2 = BinaryQuantized::quantize(&vector);
        group.bench_with_input(
            BenchmarkId::new("hamming_distance", size),
            size,
            |bench, _| {
                bench.iter(|| quantized.distance(black_box(&quantized2)));
            },
        );
    }

    group.finish();
}

fn bench_quantization_compression_ratio(c: &mut Criterion) {
    let dimensions = 384;
    let vector: Vec<f32> = (0..dimensions).map(|i| i as f32 * 0.01).collect();

    c.bench_function("scalar_vs_binary_encoding", |b| {
        b.iter(|| {
            let scalar = ScalarQuantized::quantize(black_box(&vector));
            let binary = BinaryQuantized::quantize(black_box(&vector));
            (scalar, binary)
        });
    });
}

criterion_group!(
    benches,
    bench_scalar_quantization,
    bench_binary_quantization,
    bench_quantization_compression_ratio
);
criterion_main!(benches);
