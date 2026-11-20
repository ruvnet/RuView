//! Criterion benchmark for the RuView streaming-engine hot path.
//!
//! The live system runs at 20 Hz → a **50 ms** wall-clock budget per cycle.
//! This measures one full [`StreamingEngine::process_cycle`] (fuse + quality
//! scoring + calibration provenance + privacy gate + WorldGraph semantic node)
//! for a 4-node / 56-subcarrier mesh — the realistic ESP32-S3 HT20 case.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use wifi_densepose_bfld::PrivacyMode;
use wifi_densepose_engine::StreamingEngine;
use wifi_densepose_geo::types::GeoRegistration;
use wifi_densepose_signal::hardware_norm::{CanonicalCsiFrame, HardwareType};
use wifi_densepose_signal::ruvsense::fusion_quality::CalibrationId;
use wifi_densepose_signal::ruvsense::{MultiBandCsiFrame, PhaseReference};

fn node_frame(node_id: u8, ts_us: u64, n_sub: usize) -> MultiBandCsiFrame {
    MultiBandCsiFrame {
        node_id,
        timestamp_us: ts_us,
        channel_frames: vec![CanonicalCsiFrame {
            amplitude: (0..n_sub).map(|i| 1.0 + 0.1 * i as f32).collect(),
            phase: (0..n_sub).map(|i| i as f32 * 0.05).collect(),
            hardware_type: HardwareType::Esp32S3,
        }],
        frequencies_mhz: vec![2412],
        coherence: 0.9,
        phase_reference: PhaseReference::NodeLocal,
    }
}

fn bench_cycle(c: &mut Criterion) {
    let frames: Vec<MultiBandCsiFrame> =
        (0..4).map(|i| node_frame(i, 1000 + u64::from(i), 56)).collect();

    c.bench_function("process_cycle_4nodes_56sc", |b| {
        b.iter_batched(
            || {
                let mut e =
                    StreamingEngine::new(PrivacyMode::PrivateHome, 1, GeoRegistration::default());
                let room = e.add_room("living_room", "Living Room");
                e.add_sensor("esp32-com9", room);
                (e, room)
            },
            |(mut e, room)| {
                e.process_cycle(&frames, CalibrationId(1), room, 0).unwrap()
            },
            BatchSize::SmallInput,
        );
    });
}

/// Mesh guard in isolation: cold build (node set appears) vs steady state
/// (identical weights next cycle → change-gated, zero graph updates) for a
/// 12-node mesh — the full ADR-029 deployment size.
fn bench_mesh_guard(c: &mut Criterion) {
    use wifi_densepose_engine::MeshGuard;
    let nodes: Vec<u8> = (0..12).collect();
    let w = |i: usize, j: usize| 0.4 + 0.01 * ((i + j) % 7) as f64;

    c.bench_function("mesh_guard_cold_build_12n", |b| {
        b.iter_batched(
            MeshGuard::default,
            |mut g| g.update(&nodes, w),
            BatchSize::SmallInput,
        );
    });

    c.bench_function("mesh_guard_steady_state_12n", |b| {
        let mut g = MeshGuard::default();
        g.update(&nodes, w); // warm
        b.iter(|| g.update(&nodes, w));
    });

    c.bench_function("mesh_guard_one_edge_change_12n", |b| {
        let mut g = MeshGuard::default();
        g.update(&nodes, w);
        let mut flip = false;
        b.iter(|| {
            flip = !flip;
            let delta = if flip { 0.2 } else { 0.0 };
            g.update(&nodes, |i, j| {
                if (i.min(j), i.max(j)) == (0, 1) { 0.4 + delta } else { w(i, j) }
            })
        });
    });
}

criterion_group!(benches, bench_cycle, bench_mesh_guard);
criterion_main!(benches);
