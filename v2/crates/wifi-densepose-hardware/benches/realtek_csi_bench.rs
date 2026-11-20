//! Benchmarks for ADR-323 Realtek RTL8721Dx (AmebaDplus) CSI encode/decode
//! throughput, at the real hardware's own reported dimensions (52
//! subcarriers, 16-bit tone precision — see `example/wifi/wifi_csi/README.md`
//! in the vendor SDK).

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use wifi_densepose_hardware::realtek_csi::simulator::{RealtekCsiSimulator, SimulatorConfig};
use wifi_densepose_hardware::realtek_csi::CsiFrame;

fn bench_to_bytes_8bit(c: &mut Criterion) {
    let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
    let frame = sim.next_frame();
    c.bench_function("realtek_csi_to_bytes_52sc_8bit", |b| {
        b.iter(|| black_box(frame.to_bytes().unwrap()));
    });
}

fn bench_from_bytes_8bit(c: &mut Criterion) {
    let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
    let wire = sim.next_frame().to_bytes().unwrap();
    c.bench_function("realtek_csi_from_bytes_52sc_8bit", |b| {
        b.iter(|| black_box(CsiFrame::from_bytes(black_box(&wire)).unwrap()));
    });
}

fn bench_to_bytes_16bit(c: &mut Criterion) {
    let cfg = SimulatorConfig {
        num_bit_per_tone: 32,
        ..Default::default()
    };
    let mut sim = RealtekCsiSimulator::new(cfg).unwrap();
    let frame = sim.next_frame();
    c.bench_function("realtek_csi_to_bytes_52sc_16bit", |b| {
        b.iter(|| black_box(frame.to_bytes().unwrap()));
    });
}

fn bench_from_bytes_16bit(c: &mut Criterion) {
    let cfg = SimulatorConfig {
        num_bit_per_tone: 32,
        ..Default::default()
    };
    let mut sim = RealtekCsiSimulator::new(cfg).unwrap();
    let wire = sim.next_frame().to_bytes().unwrap();
    c.bench_function("realtek_csi_from_bytes_52sc_16bit", |b| {
        b.iter(|| black_box(CsiFrame::from_bytes(black_box(&wire)).unwrap()));
    });
}

fn bench_round_trip_full_datagram(c: &mut Criterion) {
    let mut sim = RealtekCsiSimulator::new(SimulatorConfig::default()).unwrap();
    c.bench_function("realtek_csi_round_trip_52sc_8bit", |b| {
        b.iter(|| {
            let frame = sim.next_frame();
            let wire = black_box(frame.to_bytes().unwrap());
            black_box(CsiFrame::from_bytes(&wire).unwrap());
        });
    });
}

criterion_group!(
    benches,
    bench_to_bytes_8bit,
    bench_from_bytes_8bit,
    bench_to_bytes_16bit,
    bench_from_bytes_16bit,
    bench_round_trip_full_datagram,
);
criterion_main!(benches);
