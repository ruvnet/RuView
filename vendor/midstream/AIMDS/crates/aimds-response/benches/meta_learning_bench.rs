//! Benchmarks for meta-learning engine

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use aimds_response::{MetaLearningEngine, FeedbackSignal};

fn bench_pattern_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let runtime = tokio::runtime::Runtime::new().unwrap();

            b.to_async(&runtime).iter(|| async {
                let mut engine = MetaLearningEngine::new();

                for i in 0..size {
                    let incident = create_test_incident(i);
                    engine.learn_from_incident(&incident).await;
                }

                black_box(engine.learned_patterns_count())
            });
        });
    }

    group.finish();
}

fn bench_optimization_levels(c: &mut Criterion) {
    let mut group = c.benchmark_group("optimization_levels");

    for level in [1, 5, 10, 25].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(level), level, |b, &level| {
            b.iter(|| {
                let mut engine = MetaLearningEngine::new();

                let feedback: Vec<FeedbackSignal> = (0..100)
                    .map(|i| FeedbackSignal {
                        strategy_id: format!("strategy_{}", i % 5),
                        success: true,
                        effectiveness_score: 0.85,
                        timestamp: chrono::Utc::now(),
                        context: None,
                    })
                    .collect();

                for _ in 0..level {
                    engine.optimize_strategy(&feedback);
                }

                black_box(engine.current_optimization_level())
            });
        });
    }

    group.finish();
}

fn bench_feedback_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("feedback_processing");

    for feedback_count in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(feedback_count),
            feedback_count,
            |b, &count| {
                b.iter(|| {
                    let mut engine = MetaLearningEngine::new();

                    let feedback: Vec<FeedbackSignal> = (0..count)
                        .map(|i| FeedbackSignal {
                            strategy_id: format!("strategy_{}", i % 10),
                            success: i % 2 == 0,
                            effectiveness_score: 0.7 + (i as f64 * 0.001),
                            timestamp: chrono::Utc::now(),
                            context: Some(format!("context_{}", i)),
                        })
                        .collect();

                    engine.optimize_strategy(&feedback);
                    black_box(engine.current_optimization_level())
                });
            },
        );
    }

    group.finish();
}

fn bench_concurrent_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_learning");

    group.bench_function("parallel_learning_10", |b| {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        b.to_async(&runtime).iter(|| async {
            let mut handles = vec![];

            for i in 0..10 {
                let handle = tokio::spawn(async move {
                    let mut engine = MetaLearningEngine::new();
                    let incident = create_test_incident(i);
                    engine.learn_from_incident(&incident).await;
                    engine.learned_patterns_count()
                });
                handles.push(handle);
            }

            let results = futures::future::join_all(handles).await;
            black_box(results.len())
        });
    });

    group.finish();
}

// Helper function
fn create_test_incident(id: i32) -> aimds_response::meta_learning::ThreatIncident {
    use aimds_response::meta_learning::{ThreatIncident, ThreatType};

    ThreatIncident {
        id: format!("incident_{}", id),
        threat_type: ThreatType::Anomaly(0.85),
        severity: 7,
        confidence: 0.9,
        timestamp: chrono::Utc::now(),
    }
}

criterion_group!(
    benches,
    bench_pattern_learning,
    bench_optimization_levels,
    bench_feedback_processing,
    bench_concurrent_learning
);
criterion_main!(benches);