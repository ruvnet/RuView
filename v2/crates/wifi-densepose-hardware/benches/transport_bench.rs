//! Benchmarks comparing manual crypto vs QUIC transport for TDM beacons.
//!
//! Measures:
//! - Beacon serialization (16-byte vs 28-byte vs QUIC-framed)
//! - Beacon verification throughput
//! - Replay window check performance
//! - FramedMessage encode/decode throughput

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::time::Duration;
use wifi_densepose_hardware::esp32::{
    TdmSchedule, SyncBeacon, SecurityMode, QuicTransportConfig,
    SecureTdmCoordinator, SecureTdmConfig, SecLevel,
    AuthenticatedBeacon, ReplayWindow, FramedMessage, MessageType,
};

fn make_beacon() -> SyncBeacon {
    SyncBeacon {
        cycle_id: 42,
        cycle_period: Duration::from_millis(50),
        drift_correction_us: -3,
        generated_at: std::time::Instant::now(),
    }
}

fn bench_beacon_serialize_plain(c: &mut Criterion) {
    let beacon = make_beacon();
    c.bench_function("beacon_serialize_16byte", |b| {
        b.iter(|| {
            black_box(beacon.to_bytes());
        });
    });
}

fn bench_beacon_serialize_authenticated(c: &mut Criterion) {
    let beacon = make_beacon();
    let key = [0x01u8; 16];
    let nonce = 1u32;
    let mut msg = [0u8; 20];
    msg[..16].copy_from_slice(&beacon.to_bytes());
    msg[16..20].copy_from_slice(&nonce.to_le_bytes());

    c.bench_function("beacon_serialize_28byte_auth", |b| {
        b.iter(|| {
            let tag = AuthenticatedBeacon::compute_tag(black_box(&msg), &key);
            black_box(AuthenticatedBeacon {
                beacon: beacon.clone(),
                nonce,
                hmac_tag: tag,
            }
            .to_bytes());
        });
    });
}

fn bench_beacon_serialize_quic_framed(c: &mut Criterion) {
    let beacon = make_beacon();

    c.bench_function("beacon_serialize_quic_framed", |b| {
        b.iter(|| {
            let bytes = beacon.to_bytes();
            let framed = FramedMessage::new(MessageType::Beacon, bytes.to_vec());
            black_box(framed.to_bytes());
        });
    });
}

fn bench_auth_beacon_verify(c: &mut Criterion) {
    let beacon = make_beacon();
    let key = [0x01u8; 16];
    let nonce = 1u32;
    let mut msg = [0u8; 20];
    msg[..16].copy_from_slice(&beacon.to_bytes());
    msg[16..20].copy_from_slice(&nonce.to_le_bytes());
    let tag = AuthenticatedBeacon::compute_tag(&msg, &key);
    let auth = AuthenticatedBeacon {
        beacon,
        nonce,
        hmac_tag: tag,
    };

    c.bench_function("auth_beacon_verify", |b| {
        b.iter(|| {
            black_box(auth.verify(&key)).unwrap();
        });
    });
}

fn bench_replay_window(c: &mut Criterion) {
    let mut group = c.benchmark_group("replay_window");

    for window_size in [4u32, 16, 64, 256] {
        group.bench_with_input(
            BenchmarkId::new("check_accept", window_size),
            &window_size,
            |b, &ws| {
                b.iter(|| {
                    let mut rw = ReplayWindow::new(ws);
                    for i in 0..1000u32 {
                        black_box(rw.accept(i));
                    }
                });
            },
        );
    }
    group.finish();
}

fn bench_framed_message_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("framed_message");

    for payload_size in [16usize, 128, 512, 2048] {
        let payload = vec![0xABu8; payload_size];
        let msg = FramedMessage::new(MessageType::CsiFrame, payload);
        let bytes = msg.to_bytes();

        group.bench_with_input(
            BenchmarkId::new("encode", payload_size),
            &msg,
            |b, msg| {
                b.iter(|| {
                    black_box(msg.to_bytes());
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("decode", payload_size),
            &bytes,
            |b, bytes| {
                b.iter(|| {
                    black_box(FramedMessage::from_bytes(bytes));
                });
            },
        );
    }
    group.finish();
}

fn bench_secure_coordinator_cycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("secure_tdm_cycle");

    // Manual crypto mode
    group.bench_function("manual_crypto", |b| {
        let schedule = TdmSchedule::default_4node();
        let config = SecureTdmConfig {
            security_mode: SecurityMode::ManualCrypto,
            mesh_key: Some([0x01u8; 16]),
            quic_config: QuicTransportConfig::default(),
            sec_level: SecLevel::Transitional,
        };
        let mut coord = SecureTdmCoordinator::new(schedule, config).unwrap();

        b.iter(|| {
            let output = coord.begin_secure_cycle().unwrap();
            black_box(&output.authenticated_bytes);
            for i in 0..4 {
                coord.complete_slot(i, 0.95);
            }
        });
    });

    // QUIC mode
    group.bench_function("quic_transport", |b| {
        let schedule = TdmSchedule::default_4node();
        let config = SecureTdmConfig {
            security_mode: SecurityMode::QuicTransport,
            mesh_key: Some([0x01u8; 16]),
            quic_config: QuicTransportConfig::default(),
            sec_level: SecLevel::Transitional,
        };
        let mut coord = SecureTdmCoordinator::new(schedule, config).unwrap();

        b.iter(|| {
            let output = coord.begin_secure_cycle().unwrap();
            black_box(&output.authenticated_bytes);
            for i in 0..4 {
                coord.complete_slot(i, 0.95);
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_beacon_serialize_plain,
    bench_beacon_serialize_authenticated,
    bench_beacon_serialize_quic_framed,
    bench_auth_beacon_verify,
    bench_replay_window,
    bench_framed_message_roundtrip,
    bench_secure_coordinator_cycle,
);
criterion_main!(benches);
