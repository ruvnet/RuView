//! Benchmarks for the explicit real-CSI preprocessing pipeline.
//!
//! All inputs are deterministic synthetic windows (unit-test fixtures, never
//! real data): the goal is to time the software stages, not to produce data
//! results. Run with:
//!
//! ```bash
//! cargo bench -p wifi-densepose-train --bench real_csi_bench
//! ```

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ndarray::Array4;
use wifi_densepose_train::public_dataset::ProcessedSampleMeta;
use wifi_densepose_train::real_csi::{RealCsiConfig, RealCsiPipeline};

fn synthetic_window(
    n_t: usize,
    n_tx: usize,
    n_rx: usize,
    n_sc: usize,
) -> (Array4<f32>, Array4<f32>) {
    let amp = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        0.5 + 0.4 * (((t * 7 + tx * 3 + rx * 2 + sc) % 17) as f32 / 17.0)
    });
    let phase = Array4::<f32>::from_shape_fn((n_t, n_tx, n_rx, n_sc), |(t, tx, rx, sc)| {
        ((t + tx + rx + sc) as f32 * 0.05).sin() * 2.0
    });
    (amp, phase)
}

fn test_meta() -> ProcessedSampleMeta {
    ProcessedSampleMeta {
        source_dataset: "mmfi-bench".to_string(),
        subject_id: 1,
        environment_id: None,
        recording_id: "S01/A01".to_string(),
        activity_id: 1,
        frame_index: 0,
        original_subcarrier_count: 0,
        processed_subcarrier_count: 0,
        antenna_configuration: "1TXx3RX".to_string(),
        sampling_rate_hz: Some(100.0),
        preprocessing_version: String::new(),
    }
}

/// Full pipeline over a 10-frame MM-Fi-shaped window (114 -> 56).
fn bench_pipeline_114_to_56(c: &mut Criterion) {
    let (amp, phase) = synthetic_window(10, 1, 3, 114);
    c.bench_function("real_csi_pipeline_10x1x3x114_to_56", |b| {
        b.iter(|| {
            let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
            pipe.process(black_box(&amp), black_box(&phase), test_meta())
                .expect("process")
        })
    });
}

/// Full pipeline at native width (sanitize + denoise + norm, no conversion).
fn bench_pipeline_native_56(c: &mut Criterion) {
    let (amp, phase) = synthetic_window(10, 1, 3, 56);
    c.bench_function("real_csi_pipeline_10x1x3x56_native", |b| {
        b.iter(|| {
            let mut pipe = RealCsiPipeline::new(RealCsiConfig::default()).expect("pipeline");
            pipe.process(black_box(&amp), black_box(&phase), test_meta())
                .expect("process")
        })
    });
}

criterion_group!(benches, bench_pipeline_114_to_56, bench_pipeline_native_56);
criterion_main!(benches);
