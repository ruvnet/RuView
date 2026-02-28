//! Benchmarks for rvf-federation crate.

use criterion::{criterion_group, criterion_main, Criterion, black_box};
use rvf_federation::*;
use rvf_federation::aggregate::{FederatedAggregator, AggregationStrategy, Contribution};
use rvf_federation::diff_privacy::{DiffPrivacyEngine, PrivacyAccountant};
use rvf_federation::pii_strip::PiiStripper;
use rvf_federation::federation::{ExportBuilder, ImportMerger};
use rvf_federation::policy::FederationPolicy;

fn bench_pii_strip(c: &mut Criterion) {
    let mut group = c.benchmark_group("pii_strip");

    group.bench_function("detect_mixed_pii", |b| {
        let stripper = PiiStripper::new();
        let input = "file at /home/alice/project/main.rs, ip 192.168.1.100, email alice@example.com, key sk-abcdefghijklmnopqrstuv12";
        b.iter(|| {
            black_box(stripper.contains_pii(black_box(input)));
        });
    });

    group.bench_function("strip_10_fields", |b| {
        let fields: Vec<(&str, &str)> = (0..10).map(|i| {
            if i % 3 == 0 {
                ("path", "/home/user/data/file.csv")
            } else if i % 3 == 1 {
                ("ip", "server at 10.0.0.1:8080")
            } else {
                ("clean", "no pii here at all")
            }
        }).collect();
        b.iter(|| {
            let mut stripper = PiiStripper::new();
            black_box(stripper.strip_fields(black_box(&fields)));
        });
    });

    group.bench_function("strip_100_fields", |b| {
        let fields: Vec<(&str, &str)> = (0..100).map(|i| {
            if i % 5 == 0 {
                ("path", "/home/user/data/file.csv")
            } else {
                ("clean", "just normal text content")
            }
        }).collect();
        b.iter(|| {
            let mut stripper = PiiStripper::new();
            black_box(stripper.strip_fields(black_box(&fields)));
        });
    });

    group.finish();
}

fn bench_diff_privacy(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_privacy");

    group.bench_function("gaussian_noise_100_params", |b| {
        b.iter(|| {
            let mut engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0).unwrap().with_seed(42);
            let mut params: Vec<f64> = (0..100).map(|i| i as f64 * 0.01).collect();
            black_box(engine.add_noise(black_box(&mut params)));
        });
    });

    group.bench_function("gaussian_noise_10000_params", |b| {
        b.iter(|| {
            let mut engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0).unwrap().with_seed(42);
            let mut params: Vec<f64> = (0..10_000).map(|i| i as f64 * 0.0001).collect();
            black_box(engine.add_noise(black_box(&mut params)));
        });
    });

    group.bench_function("gradient_clipping_1000", |b| {
        let engine = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 1.0).unwrap();
        b.iter(|| {
            let mut grads: Vec<f64> = (0..1000).map(|i| (i as f64).sin()).collect();
            engine.clip_gradients(black_box(&mut grads));
        });
    });

    group.bench_function("privacy_accountant_100_rounds", |b| {
        b.iter(|| {
            let mut acc = PrivacyAccountant::new(100.0, 1e-5);
            for _ in 0..100 {
                acc.record_gaussian(1.0, 1.0, 1e-5, 100);
            }
            black_box(acc.current_epsilon());
        });
    });

    group.finish();
}

fn bench_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("aggregation");

    group.bench_function("fedavg_10_contributors_100_dim", |b| {
        b.iter(|| {
            let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
                .with_min_contributions(2);
            for i in 0..10 {
                agg.add_contribution(Contribution {
                    contributor: format!("c_{}", i),
                    weights: (0..100).map(|j| (i as f64 + j as f64) * 0.01).collect(),
                    quality_weight: 0.8 + (i as f64) * 0.02,
                    trajectory_count: 100 + i * 10,
                });
            }
            black_box(agg.aggregate().unwrap());
        });
    });

    group.bench_function("fedavg_100_contributors_1000_dim", |b| {
        b.iter(|| {
            let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
                .with_min_contributions(2);
            for i in 0..100 {
                agg.add_contribution(Contribution {
                    contributor: format!("c_{}", i),
                    weights: (0..1000).map(|j| (i as f64 + j as f64) * 0.001).collect(),
                    quality_weight: 0.8,
                    trajectory_count: 100,
                });
            }
            black_box(agg.aggregate().unwrap());
        });
    });

    group.bench_function("byzantine_detection_50_contributors", |b| {
        b.iter(|| {
            let mut agg = FederatedAggregator::new("test".into(), AggregationStrategy::FedAvg)
                .with_min_contributions(2)
                .with_byzantine_threshold(2.0);
            for i in 0..48 {
                agg.add_contribution(Contribution {
                    contributor: format!("good_{}", i),
                    weights: vec![1.0; 50],
                    quality_weight: 0.9,
                    trajectory_count: 100,
                });
            }
            // Add 2 outliers
            agg.add_contribution(Contribution {
                contributor: "evil_1".to_string(),
                weights: vec![1000.0; 50],
                quality_weight: 0.9,
                trajectory_count: 100,
            });
            agg.add_contribution(Contribution {
                contributor: "evil_2".to_string(),
                weights: vec![-500.0; 50],
                quality_weight: 0.9,
                trajectory_count: 100,
            });
            black_box(agg.aggregate().unwrap());
        });
    });

    group.finish();
}

fn bench_export_import(c: &mut Criterion) {
    let mut group = c.benchmark_group("export_import");

    group.bench_function("full_export_pipeline", |b| {
        b.iter(|| {
            let mut dp = DiffPrivacyEngine::gaussian(1.0, 1e-5, 1.0, 10.0).unwrap().with_seed(42);
            let priors = TransferPriorSet {
                source_domain: "/home/user/my_domain".to_string(),
                entries: (0..20).map(|i| TransferPriorEntry {
                    bucket_id: format!("bucket_{}", i),
                    arm_id: format!("arm_{}", i % 4),
                    params: BetaParams::new(5.0 + i as f64, 3.0 + i as f64 * 0.5),
                    observation_count: 50 + i * 10,
                }).collect(),
                cost_ema: 0.85,
            };
            let export = ExportBuilder::new("pseudo".into(), "domain".into())
                .add_priors(priors)
                .add_weights((0..256).map(|i| i as f64 * 0.001).collect())
                .add_string_field("note".into(), "trained on /home/user/data at 192.168.1.1".into())
                .build(&mut dp)
                .unwrap();
            black_box(export);
        });
    });

    group.bench_function("merge_100_priors", |b| {
        let merger = ImportMerger::new();
        let remote: Vec<TransferPriorEntry> = (0..100).map(|i| TransferPriorEntry {
            bucket_id: format!("bucket_{}", i),
            arm_id: format!("arm_{}", i % 4),
            params: BetaParams::new(10.0, 5.0),
            observation_count: 50,
        }).collect();
        b.iter(|| {
            let mut local: Vec<TransferPriorEntry> = (0..50).map(|i| TransferPriorEntry {
                bucket_id: format!("bucket_{}", i),
                arm_id: format!("arm_{}", i % 4),
                params: BetaParams::new(5.0, 3.0),
                observation_count: 20,
            }).collect();
            merger.merge_priors(black_box(&mut local), black_box(&remote), 1);
            black_box(local);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_pii_strip, bench_diff_privacy, bench_aggregation, bench_export_import);
criterion_main!(benches);
