//! Benchmarks for AIMDS analysis layer

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use aimds_analysis::*;
use aimds_core::{Action, State};
use std::collections::HashMap;

fn behavioral_analysis_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("behavioral_analysis");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for size in [100, 500, 1000].iter() {
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

fn policy_verification_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy_verification");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for num_policies in [1, 5, 10].iter() {
        let mut verifier = PolicyVerifier::new().unwrap();

        for i in 0..*num_policies {
            let policy = SecurityPolicy::new(
                format!("policy_{}", i),
                format!("Test policy {}", i),
                "G authenticated"
            );
            verifier.add_policy(policy);
        }

        let action = Action::default();

        group.bench_with_input(
            BenchmarkId::from_parameter(num_policies),
            num_policies,
            |b, _| {
                b.to_async(&rt).iter(|| async {
                    verifier.verify_policy(black_box(&action)).await.unwrap()
                });
            },
        );
    }

    group.finish();
}

fn ltl_checking_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("ltl_checking");

    for trace_len in [10, 50, 100].iter() {
        let checker = LTLChecker::new();
        let mut trace = Trace::new();

        for i in 0..*trace_len {
            let mut props = HashMap::new();
            props.insert("authenticated".to_string(), true);
            trace.add_state(State::default(), props);
        }

        let formula = LTLFormula::parse("G authenticated").unwrap();

        group.bench_with_input(
            BenchmarkId::from_parameter(trace_len),
            trace_len,
            |b, _| {
                b.iter(|| {
                    checker.check_formula(black_box(&formula), black_box(&trace))
                });
            },
        );
    }

    group.finish();
}

fn full_analysis_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_analysis");

    let rt = tokio::runtime::Runtime::new().unwrap();

    let engine = AnalysisEngine::new(10).unwrap();
    let sequence: Vec<f64> = (0..1000).map(|i| (i as f64 * 0.1).sin()).collect();
    let action = Action::default();

    group.bench_function("combined_analysis", |b| {
        b.to_async(&rt).iter(|| async {
            engine.analyze_full(
                black_box(&sequence),
                black_box(&action)
            ).await.unwrap()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    behavioral_analysis_benchmark,
    policy_verification_benchmark,
    ltl_checking_benchmark,
    full_analysis_benchmark
);
criterion_main!(benches);
