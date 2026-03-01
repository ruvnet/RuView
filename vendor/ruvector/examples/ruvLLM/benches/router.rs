//! Router benchmarks for RuvLLM
//!
//! Benchmarks FastGRNN router forward pass and training.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvllm::config::RouterConfig;
use ruvllm::router::FastGRNNRouter;
use ruvllm::types::RouterSample;

fn benchmark_router_forward(c: &mut Criterion) {
    let config = RouterConfig::default();
    let router = FastGRNNRouter::new(&config).unwrap();

    let features = vec![0.1f32; config.input_dim];
    let hidden = vec![0.0f32; config.hidden_dim];

    c.bench_function("router_forward", |b| {
        b.iter(|| black_box(router.forward(&features, &hidden).unwrap()))
    });
}

fn benchmark_router_forward_batch_sizes(c: &mut Criterion) {
    let config = RouterConfig::default();
    let router = FastGRNNRouter::new(&config).unwrap();
    let hidden = vec![0.0f32; config.hidden_dim];

    let mut group = c.benchmark_group("router_forward_features");
    for feature_dim in [64, 128, 256, 512] {
        let config = RouterConfig {
            input_dim: feature_dim,
            ..RouterConfig::default()
        };
        let router = FastGRNNRouter::new(&config).unwrap();
        let features = vec![0.1f32; feature_dim];

        group.bench_with_input(
            BenchmarkId::from_parameter(feature_dim),
            &features,
            |b, features| b.iter(|| black_box(router.forward(features, &hidden).unwrap())),
        );
    }
    group.finish();
}

fn benchmark_router_training(c: &mut Criterion) {
    let config = RouterConfig::default();
    let mut router = FastGRNNRouter::new(&config).unwrap();

    let samples: Vec<RouterSample> = (0..32)
        .map(|i| RouterSample {
            features: vec![0.1; config.input_dim],
            label_model: i % 4,
            label_context: i % 5,
            label_temperature: 0.7,
            label_top_p: 0.9,
            quality: 0.8,
            latency_ms: 100.0,
        })
        .collect();

    c.bench_function("router_train_batch_32", |b| {
        b.iter(|| black_box(router.train_batch(&samples, 0.001, 0.0, None, None)))
    });
}

fn benchmark_router_training_batch_sizes(c: &mut Criterion) {
    let config = RouterConfig::default();

    let mut group = c.benchmark_group("router_train_batch");
    for batch_size in [8, 16, 32, 64, 128] {
        let mut router = FastGRNNRouter::new(&config).unwrap();
        let samples: Vec<RouterSample> = (0..batch_size)
            .map(|i| RouterSample {
                features: vec![0.1; config.input_dim],
                label_model: i % 4,
                label_context: i % 5,
                label_temperature: 0.7,
                label_top_p: 0.9,
                quality: 0.8,
                latency_ms: 100.0,
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            &samples,
            |b, samples| b.iter(|| black_box(router.train_batch(samples, 0.001, 0.0, None, None))),
        );
    }
    group.finish();
}

fn benchmark_router_ewc(c: &mut Criterion) {
    let config = RouterConfig::default();
    let mut router = FastGRNNRouter::new(&config).unwrap();

    let samples: Vec<RouterSample> = (0..32)
        .map(|i| RouterSample {
            features: vec![0.1; config.input_dim],
            label_model: i % 4,
            label_context: i % 5,
            label_temperature: 0.7,
            label_top_p: 0.9,
            quality: 0.8,
            latency_ms: 100.0,
        })
        .collect();

    // Pre-compute Fisher and optimal weights
    let fisher = router.compute_fisher(&samples);
    let optimal = router.get_weights();

    c.bench_function("router_train_with_ewc", |b| {
        b.iter(|| {
            black_box(router.train_batch(&samples, 0.001, 0.4, Some(&fisher), Some(&optimal)))
        })
    });
}

fn benchmark_fisher_computation(c: &mut Criterion) {
    let config = RouterConfig::default();
    let router = FastGRNNRouter::new(&config).unwrap();

    let samples: Vec<RouterSample> = (0..100)
        .map(|i| RouterSample {
            features: vec![0.1; config.input_dim],
            label_model: i % 4,
            label_context: i % 5,
            label_temperature: 0.7,
            label_top_p: 0.9,
            quality: 0.8,
            latency_ms: 100.0,
        })
        .collect();

    c.bench_function("router_compute_fisher_100", |b| {
        b.iter(|| black_box(router.compute_fisher(&samples)))
    });
}

criterion_group!(
    benches,
    benchmark_router_forward,
    benchmark_router_forward_batch_sizes,
    benchmark_router_training,
    benchmark_router_training_batch_sizes,
    benchmark_router_ewc,
    benchmark_fisher_computation,
);
criterion_main!(benches);
