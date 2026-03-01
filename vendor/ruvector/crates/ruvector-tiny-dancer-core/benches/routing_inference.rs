use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_tiny_dancer_core::{Candidate, Router, RouterConfig, RoutingRequest};
use std::collections::HashMap;

fn create_candidate(id: &str, dimensions: usize) -> Candidate {
    Candidate {
        id: id.to_string(),
        embedding: vec![0.5; dimensions],
        metadata: HashMap::new(),
        created_at: chrono::Utc::now().timestamp(),
        access_count: 0,
        success_rate: 0.9,
    }
}

fn bench_routing_latency(c: &mut Criterion) {
    let router = Router::default().unwrap();
    let dimensions = 128;

    let mut group = c.benchmark_group("routing_latency");

    for num_candidates in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_candidates),
            num_candidates,
            |b, &num_candidates| {
                let candidates: Vec<Candidate> = (0..num_candidates)
                    .map(|i| create_candidate(&format!("candidate-{}", i), dimensions))
                    .collect();

                let request = RoutingRequest {
                    query_embedding: vec![0.5; dimensions],
                    candidates: candidates.clone(),
                    metadata: None,
                };

                b.iter(|| router.route(black_box(request.clone())).unwrap());
            },
        );
    }

    group.finish();
}

fn bench_feature_extraction(c: &mut Criterion) {
    use ruvector_tiny_dancer_core::feature_engineering::FeatureEngineer;

    let engineer = FeatureEngineer::new();
    let dimensions = 384;

    let mut group = c.benchmark_group("feature_extraction");

    for num_candidates in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_candidates),
            num_candidates,
            |b, &num_candidates| {
                let query = vec![0.5; dimensions];
                let candidates: Vec<Candidate> = (0..num_candidates)
                    .map(|i| create_candidate(&format!("candidate-{}", i), dimensions))
                    .collect();

                b.iter(|| {
                    engineer
                        .extract_batch_features(black_box(&query), black_box(&candidates), None)
                        .unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_model_inference(c: &mut Criterion) {
    use ruvector_tiny_dancer_core::model::FastGRNN;

    let config = ruvector_tiny_dancer_core::model::FastGRNNConfig {
        input_dim: 128,
        hidden_dim: 64,
        output_dim: 1,
        ..Default::default()
    };

    let model = FastGRNN::new(config).unwrap();
    let input = vec![0.5; 128];

    c.bench_function("model_inference", |b| {
        b.iter(|| model.forward(black_box(&input), None).unwrap());
    });
}

fn bench_batch_inference(c: &mut Criterion) {
    use ruvector_tiny_dancer_core::model::FastGRNN;

    let config = ruvector_tiny_dancer_core::model::FastGRNNConfig {
        input_dim: 128,
        hidden_dim: 64,
        output_dim: 1,
        ..Default::default()
    };

    let model = FastGRNN::new(config).unwrap();

    let mut group = c.benchmark_group("batch_inference");

    for batch_size in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(batch_size),
            batch_size,
            |b, &batch_size| {
                let inputs: Vec<Vec<f32>> = (0..batch_size).map(|_| vec![0.5; 128]).collect();

                b.iter(|| model.forward_batch(black_box(&inputs)).unwrap());
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_routing_latency,
    bench_feature_extraction,
    bench_model_inference,
    bench_batch_inference
);
criterion_main!(benches);
