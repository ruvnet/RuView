//! Simplified AIMDS response benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use aimds_response::MetaLearningEngine;
use aimds_core::{DetectionResult, ThreatSeverity, ThreatType};
use chrono::Utc;
use uuid::Uuid;

fn bench_meta_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for recursion_depth in [1, 5, 10, 15].iter() {
        let mut engine = MetaLearningEngine::new(*recursion_depth).unwrap();

        let detection = DetectionResult {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            severity: ThreatSeverity::High,
            threat_type: ThreatType::PromptInjection,
            confidence: 0.85,
            input_hash: "test_hash".to_string(),
            matched_patterns: vec!["pattern1".to_string()],
            context: serde_json::json!({}),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(recursion_depth),
            recursion_depth,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    engine.learn_from_detection(black_box(&detection)).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_mitigation_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("mitigation_strategies");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut engine = MetaLearningEngine::new(10).unwrap();

    let severities = vec![
        ("low", ThreatSeverity::Low),
        ("medium", ThreatSeverity::Medium),
        ("high", ThreatSeverity::High),
        ("critical", ThreatSeverity::Critical),
    ];

    for (name, severity) in severities {
        let detection = DetectionResult {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            severity,
            threat_type: ThreatType::PromptInjection,
            confidence: 0.9,
            input_hash: "test_hash".to_string(),
            matched_patterns: vec![],
            context: serde_json::json!({}),
        };

        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &detection,
            |b, detection| {
                b.to_async(&rt).iter(|| async {
                    engine.learn_from_detection(black_box(detection)).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_meta_learning, bench_mitigation_strategies);
criterion_main!(benches);
