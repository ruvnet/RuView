//! ADR-154 Milestone-2 perf benchmarks (§7.4 P2 "bench-first" items).
//!
//! PROOF discipline (ADR-154 §0): every P2 item is **benched before touched**.
//! A micro-opt is landed only if the bench proves the path hot; otherwise the
//! committed bench *is* the result — a MEASURED-NULL that proves the rewrite was
//! unnecessary (exactly the §5.x "already amortized" pattern). No speedup is
//! claimed without a before/after number from here.
//!
//! Reproduce (compile-only):
//!   cargo bench -p wifi-densepose-signal --no-default-features \
//!     --bench dsp_perf_bench --no-run
//!
//! Reproduce (full run, writes target/criterion/ HTML):
//!   cargo bench -p wifi-densepose-signal --no-default-features --bench dsp_perf_bench
//!
//! Groups:
//!   * `multistatic_attention` (#5) — `node_attention_weights` at 2..8 nodes ×
//!       56 subcarriers. Re-derives consensus/softmax each call; no scratch to
//!       reuse → expected MEASURED-NULL.
//!   * `tomography_reconstruct` (#6) — full ISTA solve. The two voxel buffers are
//!       allocated once per `reconstruct()` (then `.fill`-reused across
//!       iterations), so the per-solve alloc is 2×n_voxels vs an
//!       O(iters·links·voxels) compute → expected MEASURED-NULL.
//!   * `pose_kalman_update` (#7) — Kalman predict+update loop. The "gain
//!       matrices" are fixed-size **stack** arrays (`[[f32;3];6]`), not heap —
//!       nothing to reuse → expected MEASURED-NULL.
//!   * `spectrogram_multi_subcarrier` (#20) — `compute_multi_subcarrier_spectrogram`:
//!       fresh-planner-per-subcarrier (BEFORE) vs hoisted-plan (AFTER, shipped).
//!       The per-subcarrier FFT re-plan is the likely real win.
//!   * `field_model_occupancy` (#8, `eigenvalue` only) — per-call n×n
//!       eigendecomposition in `estimate_occupancy`. MEASUREMENT-ONLY: quantifies
//!       the recompute cost; incremental SVD is a sized future project, not a
//!       micro-fix.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ndarray::Array2;
use rustfft::FftPlanner;
use std::f64::consts::PI;
use std::time::Duration;

use wifi_densepose_signal::ruvsense::multistatic::node_attention_weights;
use wifi_densepose_signal::ruvsense::pose_tracker::KeypointState;
use wifi_densepose_signal::ruvsense::tomography::{
    LinkGeometry, Position3D, RfTomographer, TomographyConfig,
};
use wifi_densepose_signal::spectrogram::{
    compute_multi_subcarrier_spectrogram, compute_spectrogram, Spectrogram, SpectrogramConfig,
    WindowFunction,
};

// ---------------------------------------------------------------------------
// #5 multistatic node_attention_weights
// ---------------------------------------------------------------------------

fn make_node_amplitudes(n_nodes: usize, n_sub: usize) -> Vec<Vec<f32>> {
    (0..n_nodes)
        .map(|n| {
            (0..n_sub)
                .map(|s| {
                    let phase = (n as f32 * 0.31 + s as f32 * 0.07) % std::f32::consts::TAU;
                    0.5 + 0.4 * phase.sin()
                })
                .collect()
        })
        .collect()
}

fn bench_multistatic_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("multistatic_attention");
    group.measurement_time(Duration::from_secs(3));
    let n_sub = 56; // canonical-56 grid

    for &n_nodes in &[2usize, 4, 8] {
        let owned = make_node_amplitudes(n_nodes, n_sub);
        let refs: Vec<&[f32]> = owned.iter().map(|v| v.as_slice()).collect();
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("weights", n_nodes),
            &refs,
            |b, amplitudes| {
                b.iter(|| black_box(node_attention_weights(black_box(amplitudes), 1.0)));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// #6 tomography reconstruct (ISTA L1)
// ---------------------------------------------------------------------------

fn make_tomographer(n_links: usize) -> (RfTomographer, Vec<f64>) {
    // A modest 8x8x4 grid (256 voxels), n_links TX/RX pairs around the box.
    let config = TomographyConfig {
        nx: 8,
        ny: 8,
        nz: 4,
        bounds: [0.0, 0.0, 0.0, 4.0, 4.0, 2.0],
        lambda: 0.01,
        max_iterations: 50,
        tolerance: 1e-6,
        min_links: 8,
    };
    let mut links = Vec::with_capacity(n_links);
    for i in 0..n_links {
        let t = i as f64 / n_links as f64;
        links.push(LinkGeometry {
            tx: Position3D {
                x: 4.0 * (t * PI).cos().abs(),
                y: 0.0,
                z: 1.0,
            },
            rx: Position3D {
                x: 4.0 * (t * PI).sin().abs(),
                y: 4.0,
                z: 1.0,
            },
            link_id: i,
        });
    }
    let tomo = RfTomographer::new(config, &links).unwrap();
    // Deterministic attenuations (one occupied region in the middle).
    let attenuations: Vec<f64> = (0..n_links)
        .map(|i| 0.1 + 0.05 * ((i as f64 * 0.3).sin()))
        .collect();
    (tomo, attenuations)
}

fn bench_tomography_reconstruct(c: &mut Criterion) {
    let mut group = c.benchmark_group("tomography_reconstruct");
    group.measurement_time(Duration::from_secs(4));

    for &n_links in &[16usize, 32] {
        let (tomo, atten) = make_tomographer(n_links);
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("solve", n_links),
            &(tomo, atten),
            |b, (tomo, atten)| {
                b.iter(|| black_box(tomo.reconstruct(black_box(atten)).unwrap().occupied_count));
            },
        );
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// #7 pose tracker Kalman update loop
// ---------------------------------------------------------------------------

fn bench_pose_kalman_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("pose_kalman_update");
    group.measurement_time(Duration::from_secs(3));

    // 17 keypoints (COCO-17), N predict+update cycles — a realistic frame batch.
    for &n_updates in &[17usize, 170] {
        group.throughput(Throughput::Elements(n_updates as u64));
        group.bench_with_input(BenchmarkId::new("cycles", n_updates), &n_updates, |b, &n| {
            b.iter(|| {
                let mut acc = 0.0_f32;
                for k in 0..n {
                    let mut state = KeypointState::new(
                        (k as f32 * 0.1).sin(),
                        (k as f32 * 0.2).cos(),
                        1.0 + (k as f32 * 0.05),
                    );
                    state.predict(0.05, 0.5);
                    let meas = [
                        (k as f32 * 0.1).sin() + 0.01,
                        (k as f32 * 0.2).cos() - 0.01,
                        1.0 + (k as f32 * 0.05),
                    ];
                    state.update(&meas, 0.1, 1.0);
                    acc += state.state[0];
                }
                black_box(acc)
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// #20 multi-subcarrier spectrogram: fresh-planner vs hoisted plan
// ---------------------------------------------------------------------------

fn make_csi_temporal(n_samples: usize, n_sc: usize) -> Array2<f64> {
    Array2::from_shape_fn((n_samples, n_sc), |(t, sc)| {
        let freq = 0.7 + sc as f64 * 0.13;
        (2.0 * PI * freq * t as f64 / 100.0).sin()
            + 0.3 * (2.0 * PI * (freq * 2.1) * t as f64 / 100.0).cos()
    })
}

/// BEFORE: re-plan the FFT inside `compute_spectrogram` for every subcarrier.
/// Faithful transcription of the pre-ADR-154-M2 `compute_multi_subcarrier_spectrogram`.
fn multi_fresh_planner(
    csi: &Array2<f64>,
    sample_rate: f64,
    config: &SpectrogramConfig,
) -> Vec<Spectrogram> {
    let (_, n_sc) = csi.dim();
    (0..n_sc)
        .map(|sc| {
            let col: Vec<f64> = csi.column(sc).to_vec();
            // compute_spectrogram builds a fresh FftPlanner on every call.
            compute_spectrogram(&col, sample_rate, config).unwrap()
        })
        .collect()
}

fn bench_spectrogram_multi_subcarrier(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectrogram_multi_subcarrier");
    group.measurement_time(Duration::from_secs(5));
    let sample_rate = 100.0;

    // Realistic: 600 temporal samples (~6 s @ 100 Hz) across 56 subcarriers,
    // window 128. n_sc re-plans removed by the hoist.
    for &(n_samples, n_sc, window) in &[(600usize, 56usize, 128usize), (600, 56, 256)] {
        let csi = make_csi_temporal(n_samples, n_sc);
        let config = SpectrogramConfig {
            window_size: window,
            hop_size: 64,
            window_fn: WindowFunction::Hann,
            power: true,
        };
        group.throughput(Throughput::Elements(n_sc as u64));

        // BEFORE: fresh planner per subcarrier.
        group.bench_with_input(
            BenchmarkId::new("fresh_planner", format!("sc{n_sc}_w{window}")),
            &config,
            |b, cfg| {
                b.iter(|| black_box(multi_fresh_planner(black_box(&csi), sample_rate, cfg).len()));
            },
        );

        // AFTER: hoisted plan (the shipped `compute_multi_subcarrier_spectrogram`).
        group.bench_with_input(
            BenchmarkId::new("hoisted_plan", format!("sc{n_sc}_w{window}")),
            &config,
            |b, cfg| {
                b.iter(|| {
                    black_box(
                        compute_multi_subcarrier_spectrogram(black_box(&csi), sample_rate, cfg)
                            .unwrap()
                            .len(),
                    )
                });
            },
        );
    }
    group.finish();
}

// A standalone FftPlanner sanity micro-bench documenting the cost the hoist
// removes: building+planning a length-N forward FFT once.
fn bench_fft_plan_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("fft_plan_cost");
    group.measurement_time(Duration::from_secs(2));
    for &n in &[128usize, 256] {
        group.bench_with_input(BenchmarkId::new("plan_forward", n), &n, |b, &n| {
            b.iter(|| {
                let mut planner = FftPlanner::<f64>::new();
                black_box(planner.plan_fft_forward(black_box(n)))
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// #8 field_model SVD/eigendecomposition recompute (MEASUREMENT-ONLY)
// ---------------------------------------------------------------------------
// `estimate_occupancy` builds an n×n covariance and eigendecomposes it on every
// call (BLAS, `eigenvalue` feature). This bench quantifies that per-call cost so
// ADR-154 §7.4 #8 can record a number; incremental SVD is a sized future item,
// NOT attempted here.
#[cfg(feature = "eigenvalue")]
mod eig {
    use super::*;
    use wifi_densepose_signal::ruvsense::field_model::{FieldModel, FieldModelConfig};

    fn calibrated_model(n_sub: usize, n_links: usize) -> FieldModel {
        let config = FieldModelConfig {
            n_subcarriers: n_sub,
            n_links,
            n_modes: 3,
            min_calibration_frames: 20,
            min_calibration_duration_s: 0.0,
            baseline_expiry_s: 86_400.0,
        };
        let mut model = FieldModel::new(config).unwrap();
        // Feed deterministic calibration frames: [n_links][n_sub] per observation.
        for f in 0..30 {
            let obs: Vec<Vec<f64>> = (0..n_links)
                .map(|l| {
                    (0..n_sub)
                        .map(|s| {
                            0.5 + 0.3
                                * ((f as f64 * 0.1 + l as f64 * 0.2 + s as f64 * 0.05).sin())
                        })
                        .collect()
                })
                .collect();
            model.feed_calibration(&obs).unwrap();
        }
        model.finalize_calibration(0, 0).unwrap();
        model
    }

    pub fn bench_field_model_occupancy(c: &mut Criterion) {
        let mut group = c.benchmark_group("field_model_occupancy");
        group.measurement_time(Duration::from_secs(4));
        let n_sub = 56;
        let model = calibrated_model(n_sub, 4);
        // Sliding window of recent frames (50 ~ 2.5 s @ 20 Hz).
        let frames: Vec<Vec<f64>> = (0..50)
            .map(|t| {
                (0..n_sub)
                    .map(|s| 0.5 + 0.3 * ((t as f64 * 0.15 + s as f64 * 0.07).sin()))
                    .collect()
            })
            .collect();
        group.throughput(Throughput::Elements(1));
        group.bench_function(BenchmarkId::new("eigh", n_sub), |b| {
            b.iter(|| black_box(model.estimate_occupancy(black_box(&frames))));
        });
        group.finish();
    }
}

#[cfg(feature = "eigenvalue")]
criterion_group!(
    benches,
    bench_multistatic_attention,
    bench_tomography_reconstruct,
    bench_pose_kalman_update,
    bench_spectrogram_multi_subcarrier,
    bench_fft_plan_cost,
    eig::bench_field_model_occupancy,
);

#[cfg(not(feature = "eigenvalue"))]
criterion_group!(
    benches,
    bench_multistatic_attention,
    bench_tomography_reconstruct,
    bench_pose_kalman_update,
    bench_spectrogram_multi_subcarrier,
    bench_fft_plan_cost,
);

criterion_main!(benches);
