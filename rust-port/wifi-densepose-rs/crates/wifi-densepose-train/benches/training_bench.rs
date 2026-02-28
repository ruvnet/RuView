//! Benchmarks for the WiFi-DensePose training pipeline.
//!
//! Run with:
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

// ---------------------------------------------------------------------------
// Dataset benchmarks
// ---------------------------------------------------------------------------

/// Benchmark synthetic sample generation for a single index.
fn bench_synthetic_get(c: &mut Criterion) {
    let syn_cfg = SyntheticConfig::default();
    let dataset = SyntheticCsiDataset::new(1000, syn_cfg);

    c.bench_function("synthetic_dataset_get", |b| {
        b.iter(|| {
            let _ = dataset.get(black_box(42)).expect("sample 42 must exist");
        });
    });
}

/// Benchmark full epoch iteration (no I/O — all in-process).
fn bench_synthetic_epoch(c: &mut Criterion) {
    let mut group = c.benchmark_group("synthetic_epoch");

    for n_samples in [64usize, 256, 1024] {
        let syn_cfg = SyntheticConfig::default();
        let dataset = SyntheticCsiDataset::new(n_samples, syn_cfg);

        group.bench_with_input(
            BenchmarkId::new("samples", n_samples),
            &n_samples,
            |b, &n| {
                b.iter(|| {
                    for i in 0..n {
                        let _ = dataset.get(black_box(i)).expect("sample exists");
                    }
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Subcarrier interpolation benchmarks
// ---------------------------------------------------------------------------

/// Benchmark `interpolate_subcarriers` for the standard 114 → 56 use-case.
fn bench_interp_114_to_56(c: &mut Criterion) {
    // Simulate a single sample worth of raw CSI from MM-Fi.
    let cfg = TrainingConfig::default();
    let arr: Array4<f32> = Array4::from_shape_fn(
        (cfg.window_frames, cfg.num_antennas_tx, cfg.num_antennas_rx, 114),
        |(t, tx, rx, k)| (t + tx + rx + k) as f32 * 0.001,
    );

    c.bench_function("interp_114_to_56", |b| {
        b.iter(|| {
            let _ = interpolate_subcarriers(black_box(&arr), black_box(56));
        });
    });
}

/// Benchmark `compute_interp_weights` to ensure it is fast enough to
/// precompute at dataset construction time.
fn bench_compute_interp_weights(c: &mut Criterion) {
    c.bench_function("compute_interp_weights_114_56", |b| {
        b.iter(|| {
            let _ = compute_interp_weights(black_box(114), black_box(56));
        });
    });
}

/// Benchmark interpolation for varying source subcarrier counts.
fn bench_interp_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("interp_scaling");
    let cfg = TrainingConfig::default();

    for src_sc in [56usize, 114, 256, 512] {
        let arr: Array4<f32> = Array4::zeros((
            cfg.window_frames,
            cfg.num_antennas_tx,
            cfg.num_antennas_rx,
            src_sc,
        ));

        group.bench_with_input(
            BenchmarkId::new("src_sc", src_sc),
            &src_sc,
            |b, &sc| {
                if sc == 56 {
                    // Identity case — skip; interpolate_subcarriers clones.
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

// ---------------------------------------------------------------------------
// Config benchmarks
// ---------------------------------------------------------------------------

/// Benchmark TrainingConfig::validate() to ensure it stays O(1).
fn bench_config_validate(c: &mut Criterion) {
    let config = TrainingConfig::default();
    c.bench_function("config_validate", |b| {
        b.iter(|| {
            let _ = black_box(&config).validate();
        });
    });
}

// ---------------------------------------------------------------------------
// Criterion main
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_synthetic_get,
    bench_synthetic_epoch,
    bench_interp_114_to_56,
    bench_compute_interp_weights,
    bench_interp_scaling,
    bench_config_validate,
);
criterion_main!(benches);
