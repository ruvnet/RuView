//! Benchmarks for the WiFi-DensePose training pipeline.
//!
//! All benchmark inputs are constructed from fixed, deterministic data — no
//! `rand` crate or OS entropy is used. This ensures that benchmark numbers are
//! reproducible and that the benchmark harness itself cannot introduce
//! non-determinism.
//!
//! Run with:
//!
//! ```bash
//! cargo bench -p wifi-densepose-train
//! ```
//!
//! Criterion HTML reports are written to `target/criterion/`.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ndarray::Array4;
use wifi_densepose_train::{
    config::TrainingConfig,
    dataset::{CsiDataset, SyntheticCsiDataset, SyntheticConfig},
    subcarrier::{compute_interp_weights, interpolate_subcarriers},
};

// ─────────────────────────────────────────────────────────────────────────────
// Subcarrier interpolation benchmarks
// ─────────────────────────────────────────────────────────────────────────────

/// Benchmark `interpolate_subcarriers` 114 → 56 for a batch of 32 windows.
///
/// Represents the per-batch preprocessing step during a real training epoch.
fn bench_interp_114_to_56_batch32(c: &mut Criterion) {
    let cfg = TrainingConfig::default();
    let batch_size = 32_usize;

    // Deterministic data: linear ramp across all axes.
    let arr = Array4::<f32>::from_shape_fn(
        (
            cfg.window_frames,
            cfg.num_antennas_tx * batch_size, // stack batch along tx dimension
            cfg.num_antennas_rx,
            114,
        ),
        |(t, tx, rx, k)| (t + tx + rx + k) as f32 * 0.001,
    );

    c.bench_function("interp_114_to_56_batch32", |b| {
        b.iter(|| {
            let _ = interpolate_subcarriers(black_box(&arr), black_box(56));
        });
    });
}

/// Benchmark `interpolate_subcarriers` for varying source subcarrier counts.
fn bench_interp_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("interp_scaling");
    let cfg = TrainingConfig::default();

    for src_sc in [56_usize, 114, 256, 512] {
        let arr = Array4::<f32>::from_shape_fn(
            (cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, src_sc),
            |(t, tx, rx, k)| (t + tx + rx + k) as f32 * 0.001,
        );

        group.bench_with_input(
            BenchmarkId::new("src_sc", src_sc),
            &src_sc,
            |b, &sc| {
                if sc == 56 {
                    // Identity case: the function just clones the array.
                    b.iter(|| {
                        let _ = arr.clone();
                    });
                } else {
                    b.iter(|| {
                        let _ = interpolate_subcarriers(black_box(&arr), black_box(56));
                    });
                }
            },
        );
    }

    group.finish();
}

/// Benchmark interpolation weight precomputation (called once at dataset
/// construction time).
fn bench_compute_interp_weights(c: &mut Criterion) {
    c.bench_function("compute_interp_weights_114_56", |b| {
        b.iter(|| {
            let _ = compute_interp_weights(black_box(114), black_box(56));
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// SyntheticCsiDataset benchmarks
// ─────────────────────────────────────────────────────────────────────────────

/// Benchmark a single `get()` call on the synthetic dataset.
fn bench_synthetic_get(c: &mut Criterion) {
    let dataset = SyntheticCsiDataset::new(1000, SyntheticConfig::default());

    c.bench_function("synthetic_dataset_get", |b| {
        b.iter(|| {
            let _ = dataset.get(black_box(42)).expect("sample 42 must exist");
        });
    });
}

/// Benchmark sequential full-epoch iteration at varying dataset sizes.
fn bench_synthetic_epoch(c: &mut Criterion) {
    let mut group = c.benchmark_group("synthetic_epoch");

    for n_samples in [64_usize, 256, 1024] {
        let dataset = SyntheticCsiDataset::new(n_samples, SyntheticConfig::default());

        group.bench_with_input(
            BenchmarkId::new("samples", n_samples),
            &n_samples,
            |b, &n| {
                b.iter(|| {
                    for i in 0..n {
                        let _ = dataset.get(black_box(i)).expect("sample must exist");
                    }
                });
            },
        );
    }

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Config benchmarks
// ─────────────────────────────────────────────────────────────────────────────

/// Benchmark `TrainingConfig::validate()` to ensure it stays O(1).
fn bench_config_validate(c: &mut Criterion) {
    let config = TrainingConfig::default();
    c.bench_function("config_validate", |b| {
        b.iter(|| {
            let _ = black_box(&config).validate();
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// PCK computation benchmark (pure Rust, no tch dependency)
// ─────────────────────────────────────────────────────────────────────────────

/// Inline PCK@threshold computation for a single (pred, gt) sample.
#[inline(always)]
fn compute_pck(pred: &[[f32; 2]], gt: &[[f32; 2]], threshold: f32) -> f32 {
    let n = pred.len();
    if n == 0 {
        return 0.0;
    }
    let correct = pred
        .iter()
        .zip(gt.iter())
        .filter(|(p, g)| {
            let dx = p[0] - g[0];
            let dy = p[1] - g[1];
            (dx * dx + dy * dy).sqrt() <= threshold
        })
        .count();
    correct as f32 / n as f32
}

/// Benchmark PCK computation over 100 deterministic samples.
fn bench_pck_100_samples(c: &mut Criterion) {
    let num_samples = 100_usize;
    let num_joints = 17_usize;
    let threshold = 0.05_f32;

    // Build deterministic fixed pred/gt pairs using sines for variety.
    let samples: Vec<(Vec<[f32; 2]>, Vec<[f32; 2]>)> = (0..num_samples)
        .map(|i| {
            let pred: Vec<[f32; 2]> = (0..num_joints)
                .map(|j| {
                    [
                        ((i as f32 * 0.03 + j as f32 * 0.05).sin() * 0.5 + 0.5).clamp(0.0, 1.0),
                        (j as f32 * 0.04 + 0.2_f32).clamp(0.0, 1.0),
                    ]
                })
                .collect();
            let gt: Vec<[f32; 2]> = (0..num_joints)
                .map(|j| {
                    [
                        ((i as f32 * 0.03 + j as f32 * 0.05 + 0.01).sin() * 0.5 + 0.5)
                            .clamp(0.0, 1.0),
                        (j as f32 * 0.04 + 0.2_f32).clamp(0.0, 1.0),
                    ]
                })
                .collect();
            (pred, gt)
        })
        .collect();

    c.bench_function("pck_100_samples", |b| {
        b.iter(|| {
            let total: f32 = samples
                .iter()
                .map(|(p, g)| compute_pck(black_box(p), black_box(g), threshold))
                .sum();
            let _ = total / num_samples as f32;
        });
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Criterion registration
// ─────────────────────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    // Subcarrier interpolation
    bench_interp_114_to_56_batch32,
    bench_interp_scaling,
    bench_compute_interp_weights,
    // Dataset
    bench_synthetic_get,
    bench_synthetic_epoch,
    // Config
    bench_config_validate,
    // Metrics (pure Rust, no tch)
    bench_pck_100_samples,
);
criterion_main!(benches);
