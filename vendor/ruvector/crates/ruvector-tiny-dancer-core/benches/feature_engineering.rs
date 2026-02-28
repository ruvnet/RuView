use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruvector_tiny_dancer_core::{
    feature_engineering::{FeatureConfig, FeatureEngineer},
    Candidate,
};
use std::collections::HashMap;

fn bench_cosine_similarity(c: &mut Criterion) {
    let engineer = FeatureEngineer::new();

    c.bench_function("cosine_similarity_384d", |b| {
        let a = vec![0.5; 384];
        let b = vec![0.4; 384];

        let candidate = Candidate {
            id: "test".to_string(),
            embedding: b.clone(),
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
            access_count: 10,
            success_rate: 0.9,
        };

        b.iter(|| {
            engineer
                .extract_features(black_box(&a), black_box(&candidate), None)
                .unwrap()
        });
    });
}

fn bench_feature_weights(c: &mut Criterion) {
    let mut group = c.benchmark_group("feature_weighting");

    let configs = vec![
        ("balanced", FeatureConfig::default()),
        (
            "similarity_heavy",
            FeatureConfig {
                similarity_weight: 0.7,
                recency_weight: 0.1,
                frequency_weight: 0.1,
                success_weight: 0.05,
                metadata_weight: 0.05,
                ..Default::default()
            },
        ),
        (
            "recency_heavy",
            FeatureConfig {
                similarity_weight: 0.2,
                recency_weight: 0.5,
                frequency_weight: 0.1,
                success_weight: 0.1,
                metadata_weight: 0.1,
                ..Default::default()
            },
        ),
    ];

    for (name, config) in configs {
        group.bench_function(name, |b| {
            let engineer = FeatureEngineer::with_config(config);
            let query = vec![0.5; 128];
            let candidate = Candidate {
                id: "test".to_string(),
                embedding: vec![0.4; 128],
                metadata: HashMap::new(),
                created_at: chrono::Utc::now().timestamp(),
                access_count: 100,
                success_rate: 0.95,
            };

            b.iter(|| {
                engineer
                    .extract_features(black_box(&query), black_box(&candidate), None)
                    .unwrap()
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_cosine_similarity, bench_feature_weights);
criterion_main!(benches);
