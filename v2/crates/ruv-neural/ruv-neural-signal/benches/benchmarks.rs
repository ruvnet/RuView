//! Criterion benchmarks for ruv-neural-signal.
//!
//! Benchmarks the performance-critical signal processing functions:
//! - Hilbert transform (FFT-based analytic signal)
//! - Power spectral density (Welch's method)
//! - Connectivity matrix (PLV for all channel pairs)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::Rng;
use std::f64::consts::PI;

use ruv_neural_core::signal::{FrequencyBand, MultiChannelTimeSeries};
use ruv_neural_signal::{compute_all_pairs, compute_psd, hilbert_transform, ConnectivityMetric};

/// Generate a synthetic multi-tone signal of the given length.
fn generate_signal(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| {
            let t = i as f64 / 1000.0;
            (2.0 * PI * 10.0 * t).sin()
                + 0.5 * (2.0 * PI * 25.0 * t).cos()
                + 0.3 * (2.0 * PI * 40.0 * t).sin()
        })
        .collect()
}

/// Generate random multi-channel data.
fn generate_multichannel(num_channels: usize, num_samples: usize) -> MultiChannelTimeSeries {
    let mut rng = rand::thread_rng();
    let data: Vec<Vec<f64>> = (0..num_channels)
        .map(|ch| {
            (0..num_samples)
                .map(|i| {
                    let t = i as f64 / 1000.0;
                    let freq = 8.0 + ch as f64 * 0.5;
                    (2.0 * PI * freq * t).sin() + rng.gen_range(-0.1..0.1)
                })
                .collect()
        })
        .collect();

    MultiChannelTimeSeries {
        data,
        sample_rate_hz: 1000.0,
        num_channels,
        num_samples,
        timestamp_start: 0.0,
    }
}

fn bench_hilbert_transform(c: &mut Criterion) {
    let mut group = c.benchmark_group("hilbert_transform");

    for &n in &[256, 1024, 4096] {
        let signal = generate_signal(n);
        group.bench_with_input(BenchmarkId::new("samples", n), &signal, |b, signal| {
            b.iter(|| hilbert_transform(black_box(signal)))
        });
    }

    group.finish();
}

fn bench_compute_psd(c: &mut Criterion) {
    let mut group = c.benchmark_group("compute_psd");

    let signal = generate_signal(1024);
    group.bench_function("1024_samples_win256", |b| {
        b.iter(|| compute_psd(black_box(&signal), black_box(1000.0), black_box(256)))
    });

    group.finish();
}

fn bench_connectivity_matrix(c: &mut Criterion) {
    let mut group = c.benchmark_group("connectivity_matrix");
    group.sample_size(10);

    for &num_channels in &[16, 32] {
        let data = generate_multichannel(num_channels, 1024);
        group.bench_with_input(
            BenchmarkId::new("plv_channels", num_channels),
            &data,
            |b, data| {
                b.iter(|| {
                    compute_all_pairs(
                        black_box(data),
                        black_box(ConnectivityMetric::Plv),
                        black_box(FrequencyBand::Alpha),
                    )
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hilbert_transform,
    bench_compute_psd,
    bench_connectivity_matrix,
);
criterion_main!(benches);
