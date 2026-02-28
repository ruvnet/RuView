use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_nervous_system::DentateGyrus;

fn bench_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("dentate_gyrus_encoding");

    // Benchmark different input dimensions
    for input_dim in [128, 256, 512].iter() {
        let dg = DentateGyrus::new(*input_dim, 10000, 200, 42);
        let input: Vec<f32> = (0..*input_dim).map(|i| (i as f32).sin()).collect();

        group.bench_with_input(BenchmarkId::from_parameter(input_dim), input_dim, |b, _| {
            b.iter(|| black_box(dg.encode(black_box(&input))));
        });
    }

    group.finish();
}

fn bench_similarity(c: &mut Criterion) {
    let dg = DentateGyrus::new(512, 10000, 200, 42);

    let input1: Vec<f32> = (0..512).map(|i| (i as f32).sin()).collect();
    let input2: Vec<f32> = (0..512).map(|i| (i as f32).cos()).collect();

    let sparse1 = dg.encode(&input1);
    let sparse2 = dg.encode(&input2);

    c.bench_function("jaccard_similarity", |b| {
        b.iter(|| black_box(sparse1.jaccard_similarity(black_box(&sparse2))));
    });

    c.bench_function("hamming_distance", |b| {
        b.iter(|| black_box(sparse1.hamming_distance(black_box(&sparse2))));
    });
}

fn bench_sparsity_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparsity_levels");

    for sparsity_pct in [2, 3, 5].iter() {
        let k = (10000 * sparsity_pct) / 100;
        let dg = DentateGyrus::new(512, 10000, k, 42);
        let input: Vec<f32> = (0..512).map(|i| (i as f32).sin()).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}%", sparsity_pct)),
            sparsity_pct,
            |b, _| {
                b.iter(|| black_box(dg.encode(black_box(&input))));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encoding,
    bench_similarity,
    bench_sparsity_levels
);
criterion_main!(benches);
