//! Benchmarks for cryptographic operations
//!
//! Target latencies:
//! - Receipt signing: < 5ms
//! - Hash chain verification for 1000 receipts: < 100ms
//! - Permit token encoding/decoding: < 1ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use cognitum_gate_tilezero::{
    GateDecision, PermitState, PermitToken, ReceiptLog, TimestampProof, WitnessReceipt,
    WitnessSummary,
};

/// Create a test permit token
fn create_test_token(sequence: u64) -> PermitToken {
    PermitToken {
        decision: GateDecision::Permit,
        action_id: format!("action-{}", sequence),
        timestamp: 1704067200_000_000_000 + sequence * 1_000_000,
        ttl_ns: 60_000_000_000,
        witness_hash: [0u8; 32],
        sequence,
        signature: [0u8; 64],
    }
}

/// Create a test witness summary
fn create_test_summary() -> WitnessSummary {
    // Use the public empty constructor and modify through serialization
    let json = serde_json::json!({
        "structural": {
            "cut_value": 10.5,
            "partition": "stable",
            "critical_edges": 15,
            "boundary": ["edge-1", "edge-2"]
        },
        "predictive": {
            "set_size": 3,
            "coverage": 0.95
        },
        "evidential": {
            "e_value": 150.0,
            "verdict": "accept"
        }
    });
    serde_json::from_value(json).unwrap()
}

/// Create a test receipt
fn create_test_receipt(sequence: u64, previous_hash: [u8; 32]) -> WitnessReceipt {
    WitnessReceipt {
        sequence,
        token: create_test_token(sequence),
        previous_hash,
        witness_summary: create_test_summary(),
        timestamp_proof: TimestampProof {
            timestamp: 1704067200_000_000_000 + sequence * 1_000_000,
            previous_receipt_hash: previous_hash,
            merkle_root: [0u8; 32],
        },
    }
}

/// Benchmark permit token signing
fn bench_token_signing(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_signing");
    group.throughput(Throughput::Elements(1));

    let state = PermitState::new();
    let token = create_test_token(0);

    group.bench_function("sign_token", |b| {
        b.iter(|| {
            let unsigned = create_test_token(black_box(0));
            black_box(state.sign_token(unsigned))
        })
    });

    // Benchmark signing with different action_id lengths
    for action_len in [10, 50, 100, 500] {
        let mut long_token = token.clone();
        long_token.action_id = "x".repeat(action_len);

        group.bench_with_input(
            BenchmarkId::new("sign_action_len", action_len),
            &long_token,
            |b, token| {
                b.iter(|| {
                    let t = token.clone();
                    black_box(state.sign_token(t))
                })
            },
        );
    }

    group.finish();
}

/// Benchmark token verification
fn bench_token_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_verification");
    group.throughput(Throughput::Elements(1));

    let state = PermitState::new();
    let verifier = state.verifier();
    let signed_token = state.sign_token(create_test_token(0));

    group.bench_function("verify_token", |b| {
        b.iter(|| black_box(verifier.verify(black_box(&signed_token))))
    });

    group.finish();
}

/// Benchmark receipt hashing
fn bench_receipt_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_hashing");
    group.throughput(Throughput::Elements(1));

    let receipt = create_test_receipt(0, [0u8; 32]);

    group.bench_function("hash_receipt", |b| b.iter(|| black_box(receipt.hash())));

    // Benchmark with different summary sizes
    for boundary_size in [0, 10, 50, 100] {
        let mut receipt = create_test_receipt(0, [0u8; 32]);
        receipt.witness_summary.structural.boundary = (0..boundary_size)
            .map(|i| format!("boundary-edge-{}", i))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("hash_boundary_size", boundary_size),
            &receipt,
            |b, receipt| b.iter(|| black_box(receipt.hash())),
        );
    }

    group.finish();
}

/// Benchmark hash chain verification (target: < 100ms for 1000 receipts)
fn bench_chain_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain_verification");

    for chain_length in [100, 500, 1000, 2000] {
        group.throughput(Throughput::Elements(chain_length as u64));

        // Build the chain
        let mut log = ReceiptLog::new();
        for i in 0..chain_length {
            let receipt = create_test_receipt(i as u64, log.last_hash());
            log.append(receipt);
        }

        group.bench_with_input(
            BenchmarkId::new("verify_chain", chain_length),
            &log,
            |b, log| b.iter(|| black_box(log.verify_chain_to((chain_length - 1) as u64))),
        );
    }

    group.finish();
}

/// Benchmark receipt log operations
fn bench_receipt_log_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_log");
    group.throughput(Throughput::Elements(1));

    // Append benchmarks
    group.bench_function("append_single", |b| {
        b.iter(|| {
            let mut log = ReceiptLog::new();
            let receipt = create_test_receipt(0, log.last_hash());
            log.append(receipt);
            black_box(log)
        })
    });

    // Benchmark appending to logs of various sizes
    for initial_size in [10, 100, 500] {
        group.bench_with_input(
            BenchmarkId::new("append_to_n", initial_size),
            &initial_size,
            |b, &size| {
                b.iter_batched(
                    || {
                        let mut log = ReceiptLog::new();
                        for i in 0..size {
                            let receipt = create_test_receipt(i as u64, log.last_hash());
                            log.append(receipt);
                        }
                        log
                    },
                    |mut log| {
                        let receipt = create_test_receipt(log.len() as u64, log.last_hash());
                        log.append(receipt);
                        black_box(log)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Get benchmarks - recreate log for each get test
    let mut existing_log = ReceiptLog::new();
    for i in 0..100 {
        let receipt = create_test_receipt(i, existing_log.last_hash());
        existing_log.append(receipt);
    }

    group.bench_function("get_receipt", |b| {
        b.iter(|| black_box(existing_log.get(black_box(50))))
    });

    group.finish();
}

/// Benchmark permit token encoding/decoding
fn bench_token_encoding(c: &mut Criterion) {
    let mut group = c.benchmark_group("token_encoding");
    group.throughput(Throughput::Elements(1));

    let state = PermitState::new();
    let signed_token = state.sign_token(create_test_token(0));
    let encoded = signed_token.encode_base64();

    group.bench_function("encode_base64", |b| {
        b.iter(|| black_box(signed_token.encode_base64()))
    });

    group.bench_function("decode_base64", |b| {
        b.iter(|| black_box(PermitToken::decode_base64(black_box(&encoded))))
    });

    group.bench_function("roundtrip", |b| {
        b.iter(|| {
            let encoded = signed_token.encode_base64();
            black_box(PermitToken::decode_base64(&encoded))
        })
    });

    // Benchmark with varying action_id lengths
    for action_len in [10, 50, 100, 500] {
        let mut token = create_test_token(0);
        token.action_id = "x".repeat(action_len);
        let signed = state.sign_token(token);

        group.bench_with_input(
            BenchmarkId::new("encode_action_len", action_len),
            &signed,
            |b, token| b.iter(|| black_box(token.encode_base64())),
        );
    }

    group.finish();
}

/// Benchmark signable content generation
fn bench_signable_content(c: &mut Criterion) {
    let mut group = c.benchmark_group("signable_content");
    group.throughput(Throughput::Elements(1));

    let token = create_test_token(0);

    group.bench_function("generate", |b| {
        b.iter(|| black_box(token.signable_content()))
    });

    // With longer action_id
    for action_len in [10, 100, 1000] {
        let mut token = create_test_token(0);
        token.action_id = "x".repeat(action_len);

        group.bench_with_input(
            BenchmarkId::new("action_len", action_len),
            &token,
            |b, token| b.iter(|| black_box(token.signable_content())),
        );
    }

    group.finish();
}

/// Benchmark witness summary hashing
fn bench_witness_summary_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("witness_summary_hash");
    group.throughput(Throughput::Elements(1));

    let summary = create_test_summary();

    group.bench_function("hash", |b| b.iter(|| black_box(summary.hash())));

    // JSON serialization (used in hash)
    group.bench_function("to_json", |b| b.iter(|| black_box(summary.to_json())));

    group.finish();
}

/// Benchmark batch signing (simulating high-throughput scenarios)
fn bench_batch_signing(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_signing");

    for batch_size in [10, 50, 100] {
        group.throughput(Throughput::Elements(batch_size as u64));

        let state = PermitState::new();
        let tokens: Vec<_> = (0..batch_size)
            .map(|i| create_test_token(i as u64))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("sequential", batch_size),
            &tokens,
            |b, tokens| {
                b.iter(|| {
                    let signed: Vec<_> = tokens
                        .iter()
                        .cloned()
                        .map(|t| state.sign_token(t))
                        .collect();
                    black_box(signed)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_token_signing,
    bench_token_verification,
    bench_receipt_hashing,
    bench_chain_verification,
    bench_receipt_log_operations,
    bench_token_encoding,
    bench_signable_content,
    bench_witness_summary_hash,
    bench_batch_signing,
);

criterion_main!(benches);
