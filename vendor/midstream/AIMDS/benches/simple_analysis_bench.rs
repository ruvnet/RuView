//! Simplified AIMDS analysis benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use aimds_analysis::BehavioralAnalyzer;

fn bench_behavioral_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("behavioral_analysis");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for size in [50, 100, 500, 1000].iter() {
        let analyzer = BehavioralAnalyzer::new(10).unwrap();
        let sequence: Vec<f64> = (0..*size).map(|i| (i as f64 * 0.1).sin()).collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    analyzer.analyze_behavior(black_box(&sequence)).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

fn bench_anomaly_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("anomaly_detection");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let analyzer = BehavioralAnalyzer::new(10).unwrap();

    // Normal pattern
    let normal: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();

    // Anomalous pattern (sudden spike)
    let mut anomalous = normal.clone();
    anomalous[50] = 10.0;

    group.bench_function("normal_pattern", |b| {
        b.to_async(&rt).iter(|| async {
            analyzer.analyze_behavior(black_box(&normal)).await.unwrap()
        });
    });

    group.bench_function("anomalous_pattern", |b| {
        b.to_async(&rt).iter(|| async {
            analyzer.analyze_behavior(black_box(&anomalous)).await.unwrap()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_behavioral_analysis, bench_anomaly_detection);
criterion_main!(benches);
