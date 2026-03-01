//! Benchmarks for spectrogram computation.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sevensense_audio::spectrogram::{MelSpectrogram, SpectrogramConfig};
use std::f32::consts::PI;

fn generate_sine_wave(freq: f32, duration_s: f32, sample_rate: u32) -> Vec<f32> {
    let num_samples = (duration_s * sample_rate as f32) as usize;
    (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * PI * freq * t).sin()
        })
        .collect()
}

fn benchmark_spectrogram(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectrogram");

    // Test different audio durations
    for duration in [1.0, 2.0, 5.0, 10.0] {
        let samples = generate_sine_wave(1000.0, duration, 32000);
        let config = SpectrogramConfig::default();

        group.bench_with_input(
            BenchmarkId::new("compute", format!("{}s", duration)),
            &samples,
            |b, samples| {
                b.iter(|| {
                    MelSpectrogram::compute(black_box(samples), black_box(config.clone()))
                });
            },
        );
    }

    group.finish();
}

fn benchmark_5s_segment(c: &mut Criterion) {
    let samples = generate_sine_wave(2000.0, 5.0, 32000);
    let config = SpectrogramConfig::for_5s_segment();

    c.bench_function("spectrogram_5s_segment", |b| {
        b.iter(|| MelSpectrogram::compute(black_box(&samples), black_box(config.clone())));
    });
}

fn benchmark_mel_bands(c: &mut Criterion) {
    let mut group = c.benchmark_group("mel_bands");
    let samples = generate_sine_wave(1000.0, 2.0, 32000);

    // Test different mel band counts
    for n_mels in [64, 128, 256] {
        let config = SpectrogramConfig {
            n_mels,
            ..SpectrogramConfig::default()
        };

        group.bench_with_input(
            BenchmarkId::new("n_mels", n_mels),
            &samples,
            |b, samples| {
                b.iter(|| {
                    MelSpectrogram::compute(black_box(samples), black_box(config.clone()))
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, benchmark_spectrogram, benchmark_5s_segment, benchmark_mel_bands);
criterion_main!(benches);
