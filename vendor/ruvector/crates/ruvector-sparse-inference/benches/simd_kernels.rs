//! Benchmarks for SIMD kernel performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_sparse_inference::backend::{cpu::CpuBackend, Backend};
use ruvector_sparse_inference::sparse::ActivationType;

fn bench_dot_product(c: &mut Criterion) {
    let backend = CpuBackend;
    let mut group = c.benchmark_group("dot_product");

    for size in [128, 256, 512, 1024, 2048, 4096].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i * 2) as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| black_box(backend.dot_product(black_box(&a), black_box(&b))));
        });
    }
    group.finish();
}

fn bench_relu(c: &mut Criterion) {
    let backend = CpuBackend;
    let mut group = c.benchmark_group("relu");

    for size in [128, 256, 512, 1024, 2048, 4096].iter() {
        let data: Vec<f32> = (0..*size).map(|i| i as f32 - (*size / 2) as f32).collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let mut d = data.clone();
                backend.activation(black_box(&mut d), ActivationType::Relu);
                black_box(d);
            });
        });
    }
    group.finish();
}

fn bench_axpy(c: &mut Criterion) {
    let backend = CpuBackend;
    let mut group = c.benchmark_group("axpy");

    for size in [128, 256, 512, 1024, 2048, 4096].iter() {
        let a: Vec<f32> = (0..*size).map(|i| i as f32).collect();
        let b: Vec<f32> = (0..*size).map(|i| (i * 2) as f32).collect();
        let scalar = 2.5f32;

        group.bench_with_input(BenchmarkId::from_parameter(size), size, |bench, _| {
            bench.iter(|| {
                let mut a_copy = a.clone();
                backend.axpy(black_box(&mut a_copy), black_box(&b), black_box(scalar));
                black_box(a_copy);
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_dot_product, bench_relu, bench_axpy);
criterion_main!(benches);
