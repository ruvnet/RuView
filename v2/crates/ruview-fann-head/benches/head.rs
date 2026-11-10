//! Scoring and preparation cost of the presence head (ADR-425).
//!
//! Engine cost only, on a synthetic 10-feature vector (two sources x five
//! features). It says nothing about presence accuracy, and nothing about the
//! helper's end-to-end latency, where tokenisation and the encoder dominate.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use ruview_fann_head::shadow::ShadowScorer;
use ruview_fann_head::train::{fit, FitConfig};
use ruview_fann_head::{FannHead, HeadContext, LinearHead};

fn context() -> HeadContext {
    HeadContext {
        feature_schema: "aether-skip/1".into(),
        sources: vec!["node-a".into(), "node-b".into()],
        room_revision: "bench".into(),
    }
}

fn fixture() -> (LinearHead, FannHead, FannHead, Vec<f32>) {
    let weights: Vec<f32> = (0..11).map(|i| (i as f32 * 0.37).sin()).collect();
    let incumbent = LinearHead::new(weights).unwrap();
    let parity = FannHead::from_linear(&incumbent, context()).unwrap();
    // 64 synthetic rows, label = sign of the first feature.
    let rows = 64;
    let features: Vec<f32> = (0..rows * 10)
        .map(|i| ((i * 7919) % 200) as f32 / 100.0 - 1.0)
        .collect();
    let labels: Vec<bool> = (0..rows).map(|r| features[r * 10] > 0.0).collect();
    let candidate = fit(&features, &labels, context(), FitConfig::candidate())
        .unwrap()
        .head;
    let x: Vec<f32> = (0..10).map(|i| (i as f32 * 0.21).cos()).collect();
    (incumbent, parity, candidate, x)
}

fn bench(c: &mut Criterion) {
    let (incumbent, parity, candidate, x) = fixture();
    let mut group = c.benchmark_group("presence_head");

    group.bench_function("incumbent_linear_score", |b| {
        b.iter(|| incumbent.probability(black_box(&x)).unwrap())
    });

    let mut scorer = parity.scorer().unwrap();
    group.bench_function("fann_parity_10x1_score", |b| {
        b.iter(|| parity.score(&mut scorer, black_box(&x)).unwrap())
    });

    let mut scorer = candidate.scorer().unwrap();
    group.bench_function("fann_candidate_10x4x1_score", |b| {
        b.iter(|| candidate.score(&mut scorer, black_box(&x)).unwrap())
    });

    let mut shadow = ShadowScorer::new(&incumbent, &candidate, &context()).unwrap();
    group.bench_function("shadow_both_heads", |b| {
        // The score borrows the scorer, so return an owned field of it.
        b.iter(|| shadow.score("obs", black_box(&x)).unwrap().decisions_agree)
    });

    let json = candidate.to_json();
    group.bench_function("prepare_from_json", |b| {
        b.iter(|| FannHead::from_json(black_box(&json)).unwrap())
    });

    group.bench_function("score_with_fresh_prepare", |b| {
        b.iter(|| {
            let head = FannHead::from_json(black_box(&json)).unwrap();
            let mut scorer = head.scorer().unwrap();
            head.score(&mut scorer, black_box(&x)).unwrap()
        })
    });
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
