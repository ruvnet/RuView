//! Benchmarks for mitigation execution

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use aimds_response::{AdaptiveMitigator, ResponseSystem};
use std::time::Duration;

fn bench_strategy_selection(c: &mut Criterion) {
    let mut group = c.benchmark_group("strategy_selection");

    for severity in [3, 5, 7, 9].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(severity), severity, |b, &severity| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.to_async(&runtime).iter(|| async {
                let mitigator = AdaptiveMitigator::new();
                let threat = create_test_threat(severity);

                let result = mitigator.apply_mitigation(&threat).await;
                black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_mitigation_execution(c: &mut Criterion) {
    let mut group = c.benchmark_group("mitigation_execution");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("single_mitigation", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        b.to_async(&runtime).iter(|| async {
            let system = ResponseSystem::new().await.unwrap();
            let threat = create_test_threat(7);

            let result = system.mitigate(&threat).await;
            black_box(result)
        });
    });

    group.finish();
}

fn bench_concurrent_mitigations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_mitigations");

    for concurrency in [5, 10, 20, 50].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &count| {
                let runtime = tokio::runtime::Runtime::new().unwrap();

                b.to_async(&runtime).iter(|| async move {
                    let system = ResponseSystem::new().await.unwrap();
                    let mut handles = vec![];

                    for i in 0..count {
                        let system_clone = system.clone();
                        let handle = tokio::spawn(async move {
                            let threat = create_test_threat((i % 4 + 1) * 2);
                            system_clone.mitigate(&threat).await
                        });
                        handles.push(handle);
                    }

                    let results = futures::future::join_all(handles).await;
                    black_box(results.len())
                });
            },
        );
    }

    group.finish();
}

fn bench_effectiveness_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("effectiveness_update");

    group.bench_function("update_100_strategies", |b| {
        b.iter(|| {
            let mut mitigator = AdaptiveMitigator::new();

            for i in 0..100 {
                let strategy_id = format!("strategy_{}", i % 10);
                mitigator.update_effectiveness(&strategy_id, i % 2 == 0);
            }

            black_box(mitigator.active_strategies_count())
        });
    });

    group.finish();
}

fn bench_end_to_end_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(Duration::from_secs(15));

    group.bench_function("full_mitigation_pipeline", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        b.to_async(&runtime).iter(|| async {
            let system = ResponseSystem::new().await.unwrap();

            // Apply mitigation
            let threat = create_test_threat(8);
            let outcome = system.mitigate(&threat).await.unwrap();

            // Learn from result
            system.learn_from_result(&outcome).await.unwrap();

            // Optimize
            let feedback = vec![aimds_response::FeedbackSignal {
                strategy_id: outcome.strategy_id.clone(),
                success: outcome.success,
                effectiveness_score: outcome.effectiveness_score(),
                timestamp: chrono::Utc::now(),
                context: None,
            }];

            system.optimize(&feedback).await.unwrap();

            black_box(system.metrics().await)
        });
    });

    group.finish();
}

fn bench_strategy_adaptation(c: &mut Criterion) {
    let mut group = c.benchmark_group("strategy_adaptation");

    group.bench_function("adapt_over_time", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        b.to_async(&runtime).iter(|| async {
            let mut mitigator = AdaptiveMitigator::new();

            for i in 0..50 {
                let threat = create_test_threat((i % 5 + 1) * 2);
                let outcome = mitigator.apply_mitigation(&threat).await.unwrap();

                // Update effectiveness with varying success
                mitigator.update_effectiveness(&outcome.strategy_id, i % 3 != 0);
            }

            black_box(mitigator.active_strategies_count())
        });
    });

    group.finish();
}

// Helper function
fn create_test_threat(severity: u8) -> aimds_response::meta_learning::ThreatIncident {
    use aimds_response::meta_learning::{ThreatIncident, ThreatType};

    ThreatIncident {
        id: uuid::Uuid::new_v4().to_string(),
        threat_type: ThreatType::Anomaly(0.85),
        severity,
        confidence: 0.9,
        timestamp: chrono::Utc::now(),
    }
}

criterion_group!(
    benches,
    bench_strategy_selection,
    bench_mitigation_execution,
    bench_concurrent_mitigations,
    bench_effectiveness_update,
    bench_end_to_end_pipeline,
    bench_strategy_adaptation
);
criterion_main!(benches);