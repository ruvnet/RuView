//! Consolidated benchmarks for cognitum-gate-tilezero
//!
//! Target latencies:
//! - Merge 255 reports: < 10ms
//! - Full gate decision: p99 < 50ms
//! - Receipt hash: < 10us
//! - Chain verify 1000 receipts: < 100ms
//! - Permit sign: < 5ms
//! - Permit verify: < 1ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;
use std::collections::HashMap;

use cognitum_gate_tilezero::{
    merge::{EdgeSummary, MergeStrategy, NodeSummary, ReportMerger, WorkerReport},
    ActionContext, ActionMetadata, ActionTarget, EvidenceFilter, GateDecision, GateThresholds,
    PermitState, PermitToken, ReceiptLog, ReducedGraph, ThreeFilterDecision, TileId, TileZero,
    TimestampProof, WitnessReceipt, WitnessSummary,
};

// ============================================================================
// Helper Functions
// ============================================================================

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

/// Create a realistic worker report
fn create_worker_report(
    tile_id: TileId,
    epoch: u64,
    node_count: usize,
    boundary_edge_count: usize,
) -> WorkerReport {
    let mut rng = rand::thread_rng();
    let mut report = WorkerReport::new(tile_id, epoch);

    for i in 0..node_count {
        report.add_node(NodeSummary {
            id: format!("node-{}-{}", tile_id, i),
            weight: rng.gen_range(0.1..10.0),
            edge_count: rng.gen_range(5..50),
            coherence: rng.gen_range(0.7..1.0),
        });
    }

    for i in 0..boundary_edge_count {
        report.add_boundary_edge(EdgeSummary {
            source: format!("node-{}-{}", tile_id, i % node_count.max(1)),
            target: format!(
                "node-{}-{}",
                (tile_id as usize + 1) % 256,
                i % node_count.max(1)
            ),
            capacity: rng.gen_range(1.0..100.0),
            is_boundary: true,
        });
    }

    report.local_mincut = rng.gen_range(1.0..20.0);
    report.confidence = rng.gen_range(0.8..1.0);
    report.timestamp_ms = 1704067200_000 + tile_id as u64 * 100;

    report
}

/// Create all 255 tile reports
fn create_all_tile_reports(
    epoch: u64,
    nodes_per_tile: usize,
    edges_per_tile: usize,
) -> Vec<WorkerReport> {
    (1..=255u8)
        .map(|tile_id| create_worker_report(tile_id, epoch, nodes_per_tile, edges_per_tile))
        .collect()
}

/// Create action context for benchmarking
fn create_action_context(id: usize) -> ActionContext {
    ActionContext {
        action_id: format!("action-{}", id),
        action_type: "config_change".to_string(),
        target: ActionTarget {
            device: Some("router-1".to_string()),
            path: Some("/config/routing/policy".to_string()),
            extra: {
                let mut m = HashMap::new();
                m.insert("priority".to_string(), serde_json::json!(100));
                m
            },
        },
        context: ActionMetadata {
            agent_id: "agent-001".to_string(),
            session_id: Some("session-12345".to_string()),
            prior_actions: vec!["action-prev-1".to_string()],
            urgency: "normal".to_string(),
        },
    }
}

/// Create realistic graph state
fn create_realistic_graph(coherence_level: f64) -> ReducedGraph {
    let mut graph = ReducedGraph::new();

    for tile_id in 1..=255u8 {
        let tile_coherence = (coherence_level + (tile_id as f64 * 0.001) % 0.1) as f32;
        graph.update_coherence(tile_id, tile_coherence);
    }

    graph.set_global_cut(coherence_level * 15.0);
    graph.set_evidence(coherence_level * 150.0);
    graph.set_shift_pressure(0.1 * (1.0 - coherence_level));

    graph
}

// ============================================================================
// 1. Merge Reports Benchmark
// ============================================================================

/// Benchmark merging 255 tile reports (target: < 10ms)
fn bench_merge_reports(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_reports");
    group.throughput(Throughput::Elements(255));

    // Test different merge strategies
    let strategies = [
        ("simple_average", MergeStrategy::SimpleAverage),
        ("weighted_average", MergeStrategy::WeightedAverage),
        ("median", MergeStrategy::Median),
        ("maximum", MergeStrategy::Maximum),
        ("byzantine_ft", MergeStrategy::ByzantineFaultTolerant),
    ];

    // Minimal reports (baseline)
    let minimal_reports = create_all_tile_reports(0, 1, 2);

    for (name, strategy) in &strategies {
        let merger = ReportMerger::new(*strategy);

        group.bench_with_input(
            BenchmarkId::new("255_tiles_minimal", name),
            &minimal_reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    // Realistic reports (10 nodes, 5 boundary edges)
    let realistic_reports = create_all_tile_reports(0, 10, 5);
    let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

    group.bench_function("255_tiles_realistic", |b| {
        b.iter(|| black_box(merger.merge(black_box(&realistic_reports))))
    });

    // Heavy reports (50 nodes, 20 edges)
    let heavy_reports = create_all_tile_reports(0, 50, 20);

    group.bench_function("255_tiles_heavy", |b| {
        b.iter(|| black_box(merger.merge(black_box(&heavy_reports))))
    });

    group.finish();
}

// ============================================================================
// 2. Full Gate Decision Benchmark
// ============================================================================

/// Benchmark full gate decision (target: p99 < 50ms)
fn bench_decision(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("gate_decision");
    group.throughput(Throughput::Elements(1));

    // Full TileZero decision
    let thresholds = GateThresholds::default();
    let tilezero = TileZero::new(thresholds.clone());
    let ctx = create_action_context(0);

    group.bench_function("tilezero_full_decision", |b| {
        b.to_async(&rt)
            .iter(|| async { black_box(tilezero.decide(black_box(&ctx)).await) });
    });

    // Three-filter decision only (no crypto)
    let decision = ThreeFilterDecision::new(thresholds);

    let graph_states = [
        ("high_coherence", create_realistic_graph(0.95)),
        ("medium_coherence", create_realistic_graph(0.7)),
        ("low_coherence", create_realistic_graph(0.3)),
    ];

    for (name, graph) in &graph_states {
        group.bench_with_input(BenchmarkId::new("three_filter", name), graph, |b, graph| {
            b.iter(|| black_box(decision.evaluate(black_box(graph))))
        });
    }

    // Batch decisions
    for batch_size in [10, 50] {
        let contexts: Vec<_> = (0..batch_size).map(create_action_context).collect();

        group.bench_with_input(
            BenchmarkId::new("batch_sequential", batch_size),
            &contexts,
            |b, contexts| {
                b.to_async(&rt).iter(|| async {
                    for ctx in contexts {
                        black_box(tilezero.decide(ctx).await);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// 3. Receipt Hash Benchmark
// ============================================================================

/// Benchmark receipt hash computation (target: < 10us)
fn bench_receipt_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_hash");
    group.throughput(Throughput::Elements(1));

    let receipt = create_test_receipt(0, [0u8; 32]);

    // Single hash
    group.bench_function("hash_single", |b| b.iter(|| black_box(receipt.hash())));

    // Hash with varying boundary sizes
    for boundary_size in [0, 10, 50, 100] {
        let mut receipt = create_test_receipt(0, [0u8; 32]);
        receipt.witness_summary.structural.boundary = (0..boundary_size)
            .map(|i| format!("boundary-edge-{}", i))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("boundary_size", boundary_size),
            &receipt,
            |b, receipt| b.iter(|| black_box(receipt.hash())),
        );
    }

    // Witness summary hash
    let summary = create_test_summary();
    group.bench_function("witness_summary_hash", |b| {
        b.iter(|| black_box(summary.hash()))
    });

    group.finish();
}

// ============================================================================
// 4. Receipt Chain Verification Benchmark
// ============================================================================

/// Benchmark receipt chain verification (target: < 100ms for 1000 receipts)
fn bench_receipt_chain_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_chain_verify");

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

    // Chain building (append) benchmark
    group.bench_function("build_chain_1000", |b| {
        b.iter(|| {
            let mut log = ReceiptLog::new();
            for i in 0..1000 {
                let receipt = create_test_receipt(i, log.last_hash());
                log.append(receipt);
            }
            black_box(log)
        })
    });

    group.finish();
}

// ============================================================================
// 5. Permit Sign Benchmark
// ============================================================================

/// Benchmark permit token signing (target: < 5ms)
fn bench_permit_sign(c: &mut Criterion) {
    let mut group = c.benchmark_group("permit_sign");
    group.throughput(Throughput::Elements(1));

    let state = PermitState::new();

    // Single sign
    group.bench_function("sign_single", |b| {
        b.iter(|| {
            let token = create_test_token(black_box(0));
            black_box(state.sign_token(token))
        })
    });

    // Sign with varying action_id lengths
    for action_len in [10, 50, 100, 500] {
        let mut token = create_test_token(0);
        token.action_id = "x".repeat(action_len);

        group.bench_with_input(
            BenchmarkId::new("action_len", action_len),
            &token,
            |b, token| b.iter(|| black_box(state.sign_token(token.clone()))),
        );
    }

    // Batch signing
    for batch_size in [10, 50, 100] {
        let tokens: Vec<_> = (0..batch_size)
            .map(|i| create_test_token(i as u64))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("batch_sign", batch_size),
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

    // Signable content generation
    let token = create_test_token(0);
    group.bench_function("signable_content", |b| {
        b.iter(|| black_box(token.signable_content()))
    });

    group.finish();
}

// ============================================================================
// 6. Permit Verify Benchmark
// ============================================================================

/// Benchmark permit token verification (target: < 1ms)
fn bench_permit_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("permit_verify");
    group.throughput(Throughput::Elements(1));

    let state = PermitState::new();
    let verifier = state.verifier();
    let signed_token = state.sign_token(create_test_token(0));

    // Single verify
    group.bench_function("verify_single", |b| {
        b.iter(|| black_box(verifier.verify(black_box(&signed_token))))
    });

    // Token encoding/decoding (often paired with verification)
    let encoded = signed_token.encode_base64();

    group.bench_function("encode_base64", |b| {
        b.iter(|| black_box(signed_token.encode_base64()))
    });

    group.bench_function("decode_base64", |b| {
        b.iter(|| black_box(PermitToken::decode_base64(black_box(&encoded))))
    });

    group.bench_function("roundtrip_encode_decode", |b| {
        b.iter(|| {
            let encoded = signed_token.encode_base64();
            black_box(PermitToken::decode_base64(&encoded))
        })
    });

    // Batch verification
    let signed_tokens: Vec<_> = (0..100)
        .map(|i| state.sign_token(create_test_token(i)))
        .collect();

    group.bench_function("verify_batch_100", |b| {
        b.iter(|| {
            for token in &signed_tokens {
                black_box(verifier.verify(token));
            }
        })
    });

    group.finish();
}

// ============================================================================
// Additional Benchmarks
// ============================================================================

/// Benchmark E-value computation
fn bench_evalue_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("evalue_computation");
    group.throughput(Throughput::Elements(1));

    // Scalar update
    for capacity in [10, 100, 1000] {
        let mut filter = EvidenceFilter::new(capacity);
        for i in 0..capacity {
            filter.update(1.0 + (i as f64 * 0.001));
        }

        group.bench_with_input(
            BenchmarkId::new("scalar_update", capacity),
            &capacity,
            |b, _| {
                b.iter(|| {
                    filter.update(black_box(1.5));
                    black_box(filter.current())
                })
            },
        );
    }

    // SIMD-friendly aggregation patterns
    let tile_count = 255;
    let e_values: Vec<f64> = (0..tile_count).map(|i| 1.0 + (i as f64 * 0.01)).collect();

    group.bench_function("aggregate_255_scalar", |b| {
        b.iter(|| {
            let product: f64 = e_values.iter().product();
            black_box(product)
        })
    });

    // Chunked processing (SIMD-friendly)
    group.bench_function("aggregate_255_chunked_4", |b| {
        b.iter(|| {
            let mut accumulator = 1.0f64;
            for chunk in e_values.chunks(4) {
                let chunk_product: f64 = chunk.iter().product();
                accumulator *= chunk_product;
            }
            black_box(accumulator)
        })
    });

    // Log-sum pattern (numerically stable)
    group.bench_function("aggregate_255_log_sum", |b| {
        b.iter(|| {
            let log_sum: f64 = e_values.iter().map(|x| x.ln()).sum();
            black_box(log_sum.exp())
        })
    });

    // Parallel reduction
    group.bench_function("aggregate_255_parallel_8", |b| {
        b.iter(|| {
            let mut lanes = [1.0f64; 8];
            for (i, &val) in e_values.iter().enumerate() {
                lanes[i % 8] *= val;
            }
            let result: f64 = lanes.iter().product();
            black_box(result)
        })
    });

    group.finish();
}

/// Benchmark graph operations
fn bench_graph_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_operations");

    // Coherence updates
    for tile_count in [64, 128, 255] {
        group.throughput(Throughput::Elements(tile_count as u64));

        group.bench_with_input(
            BenchmarkId::new("coherence_updates", tile_count),
            &tile_count,
            |b, &count| {
                b.iter(|| {
                    let mut graph = ReducedGraph::new();
                    for tile_id in 1..=count as u8 {
                        graph.update_coherence(tile_id, black_box(0.9));
                    }
                    black_box(graph)
                })
            },
        );
    }

    // Witness summary generation
    let graph = create_realistic_graph(0.9);
    group.bench_function("witness_summary_generate", |b| {
        b.iter(|| black_box(graph.witness_summary()))
    });

    group.finish();
}

/// Benchmark log operations
fn bench_receipt_log_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_log_ops");
    group.throughput(Throughput::Elements(1));

    // Append to various log sizes
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

    // Get receipt
    let mut log = ReceiptLog::new();
    for i in 0..100 {
        let receipt = create_test_receipt(i, log.last_hash());
        log.append(receipt);
    }

    group.bench_function("get_receipt", |b| {
        b.iter(|| black_box(log.get(black_box(50))))
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(merge_benches, bench_merge_reports,);

criterion_group!(decision_benches, bench_decision,);

criterion_group!(
    crypto_benches,
    bench_receipt_hash,
    bench_receipt_chain_verify,
    bench_permit_sign,
    bench_permit_verify,
);

criterion_group!(
    additional_benches,
    bench_evalue_computation,
    bench_graph_operations,
    bench_receipt_log_operations,
);

criterion_main!(
    merge_benches,
    decision_benches,
    crypto_benches,
    additional_benches
);
