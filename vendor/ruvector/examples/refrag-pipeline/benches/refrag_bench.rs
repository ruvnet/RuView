//! REFRAG Pipeline Criterion Benchmarks

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

use refrag_pipeline_example::{
    compress::{CompressionStrategy, TensorCompressor},
    expand::Projector,
    sense::{LinearPolicy, MLPPolicy, PolicyModel, ThresholdPolicy},
    store::RefragStoreBuilder,
    types::RefragEntry,
};

fn bench_compression(c: &mut Criterion) {
    let mut group = c.benchmark_group("compression");

    for dim in [384, 768, 1024, 2048] {
        let mut rng = rand::thread_rng();
        let vector: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        for (name, strategy) in [
            ("none", CompressionStrategy::None),
            ("float16", CompressionStrategy::Float16),
            ("int8", CompressionStrategy::Int8),
            ("binary", CompressionStrategy::Binary),
        ] {
            let compressor = TensorCompressor::new(dim).with_strategy(strategy);

            group.throughput(Throughput::Elements(1));
            group.bench_with_input(BenchmarkId::new(name, dim), &vector, |b, v| {
                b.iter(|| compressor.compress(black_box(v)))
            });
        }
    }

    group.finish();
}

fn bench_policy(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy");

    for dim in [384, 768] {
        let mut rng = rand::thread_rng();
        let chunk: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let query: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        // Threshold policy
        let threshold = ThresholdPolicy::new(0.5);
        group.bench_with_input(
            BenchmarkId::new("threshold", dim),
            &(&chunk, &query),
            |b, (c, q)| b.iter(|| threshold.decide(black_box(c), black_box(q))),
        );

        // Linear policy
        let linear = LinearPolicy::new(dim, 0.5);
        group.bench_with_input(
            BenchmarkId::new("linear", dim),
            &(&chunk, &query),
            |b, (c, q)| b.iter(|| linear.decide(black_box(c), black_box(q))),
        );

        // MLP policy
        let mlp = MLPPolicy::new(dim, 32, 0.5);
        group.bench_with_input(
            BenchmarkId::new("mlp_32", dim),
            &(&chunk, &query),
            |b, (c, q)| b.iter(|| mlp.decide(black_box(c), black_box(q))),
        );
    }

    group.finish();
}

fn bench_projection(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection");

    for (source, target) in [(768, 4096), (768, 8192), (1536, 8192)] {
        let mut rng = rand::thread_rng();
        let input: Vec<f32> = (0..source).map(|_| rng.gen_range(-1.0..1.0)).collect();
        let projector = Projector::new(source, target, "test");

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new(format!("{}->{}", source, target), source),
            &input,
            |b, v| b.iter(|| projector.project(black_box(v))),
        );
    }

    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    let search_dim = 384;
    let tensor_dim = 768;

    for num_docs in [100, 1000, 10000] {
        let store = RefragStoreBuilder::new()
            .search_dimensions(search_dim)
            .tensor_dimensions(tensor_dim)
            .compress_threshold(0.5)
            .auto_project(false)
            .build()
            .unwrap();

        let mut rng = rand::thread_rng();

        // Insert documents
        for i in 0..num_docs {
            let search_vec: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_vec: Vec<f32> = (0..tensor_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();
            let tensor_bytes: Vec<u8> = tensor_vec.iter().flat_map(|f| f.to_le_bytes()).collect();

            let entry = RefragEntry::new(format!("doc_{}", i), search_vec, format!("Text {}", i))
                .with_tensor(tensor_bytes, "llama3-8b");
            store.insert(entry).unwrap();
        }

        let query: Vec<f32> = (0..search_dim).map(|_| rng.gen_range(-1.0..1.0)).collect();

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("hybrid_k10", num_docs), &query, |b, q| {
            b.iter(|| store.search_hybrid(black_box(q), 10, None))
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_compression,
    bench_policy,
    bench_projection,
    bench_search,
);
criterion_main!(benches);
