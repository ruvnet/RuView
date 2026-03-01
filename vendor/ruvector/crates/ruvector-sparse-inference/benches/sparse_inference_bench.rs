//! Benchmark tests for sparse inference

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;
use ruvector_sparse_inference::{
    ActivationType, LowRankPredictor, Predictor, SparseFfn, SparseInferenceEngine, SparsityConfig,
};

// Test utilities
fn random_vector(dim: usize) -> Vec<f32> {
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

fn benchmark_sparse_vs_dense(c: &mut Criterion) {
    let dense_engine = SparseInferenceEngine::new_dense(512, 2048).unwrap();
    let sparse_engine = SparseInferenceEngine::new_sparse(512, 2048, 0.3).unwrap();
    let input = random_vector(512);

    let mut group = c.benchmark_group("inference");

    group.bench_function("dense", |b| {
        b.iter(|| black_box(dense_engine.infer(&input).unwrap()))
    });

    group.bench_function("sparse_70pct", |b| {
        b.iter(|| black_box(sparse_engine.infer(&input).unwrap()))
    });

    group.finish();
}

fn benchmark_predictor(c: &mut Criterion) {
    let config = SparsityConfig::with_top_k(500);
    let predictor = LowRankPredictor::new(512, 4096, 128, config).unwrap();
    let input = random_vector(512);

    c.bench_function("predictor_predict", |b| {
        b.iter(|| black_box(predictor.predict(&input).unwrap()))
    });
}

fn benchmark_predictor_top_k(c: &mut Criterion) {
    let mut group = c.benchmark_group("predictor_top_k");
    let input = random_vector(512);

    for k in [100, 500, 1000, 2000] {
        let config = SparsityConfig::with_top_k(k);
        let predictor = LowRankPredictor::new(512, 4096, 128, config).unwrap();

        group.bench_with_input(BenchmarkId::from_parameter(k), &input, |b, input| {
            b.iter(|| black_box(predictor.predict(input).unwrap()))
        });
    }

    group.finish();
}

fn benchmark_sparse_ffn(c: &mut Criterion) {
    let ffn = SparseFfn::new(512, 2048, 512, ActivationType::Silu).unwrap();
    let input = random_vector(512);

    let mut group = c.benchmark_group("sparse_ffn");

    group.bench_function("dense_forward", |b| {
        b.iter(|| black_box(ffn.forward_dense(&input).unwrap()))
    });

    let active_10pct: Vec<usize> = (0..204).collect();
    group.bench_function("sparse_10pct", |b| {
        b.iter(|| black_box(ffn.forward_sparse(&input, &active_10pct).unwrap()))
    });

    let active_50pct: Vec<usize> = (0..1024).collect();
    group.bench_function("sparse_50pct", |b| {
        b.iter(|| black_box(ffn.forward_sparse(&input, &active_50pct).unwrap()))
    });

    group.finish();
}

fn benchmark_activation_functions(c: &mut Criterion) {
    let input = random_vector(512);
    let active: Vec<usize> = (0..500).collect();

    let mut group = c.benchmark_group("activation_functions");

    for activation in [
        ActivationType::Relu,
        ActivationType::Gelu,
        ActivationType::Silu,
    ] {
        let ffn = SparseFfn::new(512, 2048, 512, activation).unwrap();
        let name = format!("{:?}", activation);

        group.bench_with_input(BenchmarkId::from_parameter(&name), &input, |b, input| {
            b.iter(|| black_box(ffn.forward_sparse(input, &active).unwrap()))
        });
    }

    group.finish();
}

fn benchmark_sparsity_levels(c: &mut Criterion) {
    let input = random_vector(512);

    let mut group = c.benchmark_group("sparsity_levels");

    for active_pct in [10, 30, 50, 70] {
        let num_active = (2048 * active_pct) / 100;
        let active: Vec<usize> = (0..num_active).collect();
        let ffn = SparseFfn::new(512, 2048, 512, ActivationType::Silu).unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{}%_active", active_pct)),
            &(&input, &active),
            |b, (input, active)| b.iter(|| black_box(ffn.forward_sparse(input, active).unwrap())),
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_sparse_vs_dense,
    benchmark_predictor,
    benchmark_predictor_top_k,
    benchmark_sparse_ffn,
    benchmark_activation_functions,
    benchmark_sparsity_levels,
);

criterion_main!(benches);
