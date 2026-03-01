//! Throughput benchmarks for ruQu Coherence Gate.
//!
//! Performance Targets:
//! - Syndrome ingestion rate: **1M rounds/sec**
//! - Gate decisions per second: **250K decisions/sec**
//! - Permit token generation rate: **100K tokens/sec**
//!
//! Run with: `cargo bench -p ruqu --bench throughput_bench`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use ruqu::filters::{FilterConfig, FilterPipeline, SystemState};
use ruqu::syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeDelta, SyndromeRound};
use ruqu::tile::{
    GateDecision, GateThresholds, PatchGraph, PermitToken, ReceiptLog,
    SyndromeDelta as TileSyndromeDelta, TileReport, TileZero, WorkerTile,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a syndrome round with specified firing pattern
fn create_syndrome_round(round_id: u64, detector_count: usize, firing_rate: f64) -> SyndromeRound {
    let mut detectors = DetectorBitmap::new(detector_count);
    let num_fired = ((detector_count as f64) * firing_rate) as usize;
    for i in 0..num_fired {
        detectors.set(i * (detector_count / num_fired.max(1)), true);
    }
    SyndromeRound::new(round_id, round_id, round_id * 1000, detectors, 0)
}

/// Create a worker tile with pre-populated graph
fn create_worker_tile(tile_id: u8, num_vertices: u16, num_edges: u16) -> WorkerTile {
    let mut tile = WorkerTile::new(tile_id);
    for i in 0..num_vertices.min(255) {
        tile.patch_graph.ensure_vertex(i);
    }
    let mut edges_added = 0u16;
    'outer: for i in 0..num_vertices.saturating_sub(1) {
        for j in (i + 1)..num_vertices.min(i + 4) {
            if edges_added >= num_edges {
                break 'outer;
            }
            if tile.patch_graph.add_edge(i, j, 1000).is_some() {
                edges_added += 1;
            }
        }
    }
    tile.patch_graph.recompute_components();
    tile
}

// ============================================================================
// SYNDROME INGESTION THROUGHPUT
// ============================================================================

/// Benchmark syndrome ingestion rate (target: 1M rounds/sec)
fn bench_syndrome_ingestion(c: &mut Criterion) {
    let mut group = c.benchmark_group("syndrome_ingestion");

    // Single round ingestion
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_round", |b| {
        let mut buffer = SyndromeBuffer::new(4096);
        let mut round_id = 0u64;

        b.iter(|| {
            let round = create_syndrome_round(round_id, 64, 0.1);
            buffer.push(round);
            round_id += 1;
            black_box(&buffer);
        });
    });

    // Batch ingestion (1000 rounds)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000_rounds", |b| {
        let mut buffer = SyndromeBuffer::new(4096);
        let mut round_id = 0u64;

        b.iter(|| {
            for _ in 0..1000 {
                let round = create_syndrome_round(round_id, 64, 0.1);
                buffer.push(round);
                round_id += 1;
            }
            black_box(&buffer);
        });
    });

    // Large batch ingestion (10000 rounds)
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("batch_10000_rounds", |b| {
        let mut buffer = SyndromeBuffer::new(16384);
        let mut round_id = 0u64;

        b.iter(|| {
            for _ in 0..10_000 {
                let round = create_syndrome_round(round_id, 64, 0.1);
                buffer.push(round);
                round_id += 1;
            }
            black_box(&buffer);
        });
    });

    // Varying detector counts
    for detector_count in [64, 256, 512, 1024].iter() {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("batch_1000_detectors", detector_count),
            detector_count,
            |b, &count| {
                let mut buffer = SyndromeBuffer::new(4096);
                let mut round_id = 0u64;

                b.iter(|| {
                    for _ in 0..1000 {
                        let round = create_syndrome_round(round_id, count, 0.1);
                        buffer.push(round);
                        round_id += 1;
                    }
                    black_box(&buffer);
                });
            },
        );
    }

    // Varying firing rates
    for firing_rate in [0.01, 0.05, 0.1, 0.25].iter() {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new(
                "batch_1000_firing_rate",
                format!("{:.0}pct", firing_rate * 100.0),
            ),
            firing_rate,
            |b, &rate| {
                let mut buffer = SyndromeBuffer::new(4096);
                let mut round_id = 0u64;

                b.iter(|| {
                    for _ in 0..1000 {
                        let round = create_syndrome_round(round_id, 256, rate);
                        buffer.push(round);
                        round_id += 1;
                    }
                    black_box(&buffer);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// GATE DECISION THROUGHPUT
// ============================================================================

/// Benchmark gate decisions per second
fn bench_gate_decision_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_decisions");

    // Single decision
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_decision", |b| {
        let mut tile = create_worker_tile(1, 64, 128);
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        b.iter(|| {
            let delta = TileSyndromeDelta::new(0, 1, 100);
            let report = tile.tick(&delta);
            let reports = vec![report; 10];
            let decision = tilezero.merge_reports(reports);
            black_box(decision)
        });
    });

    // Batch decisions (100)
    group.throughput(Throughput::Elements(100));
    group.bench_function("batch_100_decisions", |b| {
        let mut tile = create_worker_tile(1, 64, 128);
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        b.iter(|| {
            for i in 0..100 {
                let delta = TileSyndromeDelta::new(0, 1, i as u16);
                let report = tile.tick(&delta);
                let reports = vec![report; 10];
                let decision = tilezero.merge_reports(reports);
                black_box(decision);
            }
        });
    });

    // Batch decisions (1000)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000_decisions", |b| {
        let mut tile = create_worker_tile(1, 64, 128);
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        b.iter(|| {
            for i in 0..1000 {
                let delta = TileSyndromeDelta::new(0, 1, (i % 256) as u16);
                let report = tile.tick(&delta);
                let reports = vec![report; 10];
                let decision = tilezero.merge_reports(reports);
                black_box(decision);
            }
        });
    });

    // Decisions with varying tile counts
    for tile_count in [10, 50, 100, 255].iter() {
        group.throughput(Throughput::Elements(100));
        group.bench_with_input(
            BenchmarkId::new("batch_100_tile_count", tile_count),
            tile_count,
            |b, &count| {
                let mut tile = create_worker_tile(1, 64, 128);
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);

                let base_reports: Vec<TileReport> = (1..=count)
                    .map(|i| {
                        let mut report = TileReport::new(i as u8);
                        report.local_cut = 10.0;
                        report.shift_score = 0.1;
                        report.e_value = 200.0;
                        report
                    })
                    .collect();

                b.iter(|| {
                    for _ in 0..100 {
                        let delta = TileSyndromeDelta::new(0, 1, 100);
                        let _ = tile.tick(&delta);
                        let decision = tilezero.merge_reports(base_reports.clone());
                        black_box(decision);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// PERMIT TOKEN GENERATION THROUGHPUT
// ============================================================================

/// Benchmark permit token generation rate
fn bench_permit_token_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("permit_tokens");

    // Single token
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_token", |b| {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);
        let decision = GateDecision::Permit;

        b.iter(|| {
            let token = tilezero.issue_permit(&decision);
            black_box(token)
        });
    });

    // Batch tokens (1000)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000_tokens", |b| {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);
        let decision = GateDecision::Permit;

        b.iter(|| {
            for _ in 0..1000 {
                let token = tilezero.issue_permit(&decision);
                black_box(&token);
            }
        });
    });

    // Token validation throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("validate_1000_tokens", |b| {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);
        let token = tilezero.issue_permit(&GateDecision::Permit);
        let now_ns = token.timestamp + 1000;

        b.iter(|| {
            for _ in 0..1000 {
                let valid = token.is_valid(now_ns);
                black_box(valid);
            }
        });
    });

    group.finish();
}

// ============================================================================
// RECEIPT LOG THROUGHPUT
// ============================================================================

/// Benchmark receipt log operations
fn bench_receipt_log_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("receipt_log");

    // Append throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("append_1000", |b| {
        let mut log = ReceiptLog::new();
        let witness_hash = [0u8; 32];

        b.iter(|| {
            for i in 0..1000 {
                log.append(GateDecision::Permit, i, i * 1000, witness_hash);
            }
            black_box(&log);
        });
    });

    // Lookup throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("lookup_1000", |b| {
        let mut log = ReceiptLog::new();
        let witness_hash = [0u8; 32];
        for i in 0..10000 {
            log.append(GateDecision::Permit, i, i * 1000, witness_hash);
        }

        b.iter(|| {
            for i in 0..1000 {
                let entry = log.get(i * 10);
                black_box(entry);
            }
        });
    });

    group.finish();
}

// ============================================================================
// WORKER TILE THROUGHPUT
// ============================================================================

/// Benchmark worker tile tick throughput
fn bench_worker_tile_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("worker_tile");

    // Single tick
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_tick", |b| {
        let mut tile = create_worker_tile(1, 64, 128);

        b.iter(|| {
            let delta = TileSyndromeDelta::new(0, 1, 100);
            let report = tile.tick(&delta);
            black_box(report)
        });
    });

    // Batch ticks (1000)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000_ticks", |b| {
        let mut tile = create_worker_tile(1, 64, 128);

        b.iter(|| {
            for i in 0..1000 {
                let delta = TileSyndromeDelta::new(0, 1, (i % 256) as u16);
                let report = tile.tick(&delta);
                black_box(&report);
            }
        });
    });

    // Sustained throughput (10000 ticks)
    group.throughput(Throughput::Elements(10_000));
    group.bench_function("sustained_10000_ticks", |b| {
        let mut tile = create_worker_tile(1, 64, 128);

        b.iter(|| {
            for i in 0..10_000 {
                let delta = TileSyndromeDelta::new(0, 1, (i % 256) as u16);
                let report = tile.tick(&delta);
                black_box(&report);
            }
        });
    });

    // Varying graph sizes
    for (vertices, edges) in [(32, 64), (64, 128), (128, 256), (200, 400)].iter() {
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("batch_1000_graph", format!("v{}e{}", vertices, edges)),
            &(*vertices, *edges),
            |b, &(v, e)| {
                let mut tile = create_worker_tile(1, v, e);

                b.iter(|| {
                    for i in 0..1000 {
                        let delta = TileSyndromeDelta::new(0, 1, (i % 256) as u16);
                        let report = tile.tick(&delta);
                        black_box(&report);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// FILTER PIPELINE THROUGHPUT
// ============================================================================

/// Benchmark filter pipeline throughput
fn bench_filter_pipeline_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_pipeline");

    // Create a pre-warmed pipeline
    let create_pipeline = || {
        let config = FilterConfig::default();
        let mut pipeline = FilterPipeline::new(config);

        for i in 0..50u64 {
            let _ = pipeline.structural_mut().insert_edge(i, i + 1, 1.0);
        }
        pipeline.structural_mut().build();

        for region in 0..10 {
            for _ in 0..50 {
                pipeline.shift_mut().update(region, 0.5);
            }
        }

        for _ in 0..20 {
            pipeline.evidence_mut().update(1.5);
        }

        pipeline
    };

    // Single evaluation
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_evaluation", |b| {
        let pipeline = create_pipeline();
        let state = SystemState::new(100);

        b.iter(|| {
            let result = pipeline.evaluate(&state);
            black_box(result)
        });
    });

    // Batch evaluations (1000)
    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000_evaluations", |b| {
        let pipeline = create_pipeline();
        let state = SystemState::new(100);

        b.iter(|| {
            for _ in 0..1000 {
                let result = pipeline.evaluate(&state);
                black_box(&result);
            }
        });
    });

    group.finish();
}

// ============================================================================
// SYNDROME DELTA COMPUTATION THROUGHPUT
// ============================================================================

/// Benchmark syndrome delta computation throughput
fn bench_syndrome_delta_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("syndrome_delta");

    // Create test rounds
    let create_rounds = |count: usize| -> Vec<SyndromeRound> {
        (0..count)
            .map(|i| create_syndrome_round(i as u64, 256, 0.1))
            .collect()
    };

    // Single delta computation
    group.throughput(Throughput::Elements(1));
    group.bench_function("single_delta", |b| {
        let round1 = create_syndrome_round(0, 256, 0.1);
        let round2 = create_syndrome_round(1, 256, 0.1);

        b.iter(|| {
            let delta = SyndromeDelta::compute(&round1, &round2);
            black_box(delta)
        });
    });

    // Batch delta computation (1000)
    group.throughput(Throughput::Elements(999));
    group.bench_function("batch_1000_deltas", |b| {
        let rounds = create_rounds(1000);

        b.iter(|| {
            for i in 0..999 {
                let delta = SyndromeDelta::compute(&rounds[i], &rounds[i + 1]);
                black_box(&delta);
            }
        });
    });

    // Varying detector counts
    for detector_count in [64, 256, 512, 1024].iter() {
        group.throughput(Throughput::Elements(999));
        group.bench_with_input(
            BenchmarkId::new("batch_1000_detectors", detector_count),
            detector_count,
            |b, &count| {
                let rounds: Vec<SyndromeRound> = (0..1000)
                    .map(|i| create_syndrome_round(i as u64, count, 0.1))
                    .collect();

                b.iter(|| {
                    for i in 0..999 {
                        let delta = SyndromeDelta::compute(&rounds[i], &rounds[i + 1]);
                        black_box(&delta);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// PATCH GRAPH THROUGHPUT
// ============================================================================

/// Benchmark patch graph operation throughput
fn bench_patch_graph_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("patch_graph_throughput");

    // Edge insertion throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("insert_1000_edges", |b| {
        b.iter_batched(
            PatchGraph::new,
            |mut graph| {
                for i in 0..1000u16 {
                    let v1 = i % 256;
                    let v2 = (i + 1) % 256;
                    if v1 != v2 {
                        let _ = graph.add_edge(v1, v2, 1000);
                    }
                }
                black_box(graph.num_edges)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Delta application throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("apply_1000_deltas", |b| {
        b.iter_batched(
            || {
                let mut graph = PatchGraph::new();
                for i in 0..100u16 {
                    let _ = graph.add_edge(i, (i + 1) % 100, 1000);
                }
                graph
            },
            |mut graph| {
                for i in 0..1000u16 {
                    let delta = TileSyndromeDelta::new(i % 100, (i + 1) % 100, 100);
                    graph.apply_delta(&delta);
                }
                black_box(graph.num_edges)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Component recomputation throughput
    group.throughput(Throughput::Elements(100));
    group.bench_function("recompute_100_times", |b| {
        b.iter_batched(
            || {
                let mut graph = PatchGraph::new();
                for i in 0..200u16 {
                    let _ = graph.add_edge(i, (i + 1) % 200, 1000);
                }
                graph
            },
            |mut graph| {
                let mut count = 0u16;
                for _ in 0..100 {
                    graph.status |= PatchGraph::STATUS_DIRTY;
                    count = graph.recompute_components();
                }
                black_box(count)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// DETECTOR BITMAP THROUGHPUT
// ============================================================================

/// Benchmark detector bitmap throughput
fn bench_bitmap_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("bitmap_throughput");

    // XOR throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("xor_1000", |b| {
        let mut a = DetectorBitmap::new(1024);
        let mut bb = DetectorBitmap::new(1024);
        for i in (0..512).step_by(2) {
            a.set(i, true);
        }
        for i in (256..768).step_by(2) {
            bb.set(i, true);
        }

        b.iter(|| {
            for _ in 0..1000 {
                let result = a.xor(&bb);
                black_box(&result);
            }
        });
    });

    // Popcount throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("popcount_1000", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in (0..512).step_by(2) {
            bitmap.set(i, true);
        }

        b.iter(|| {
            let mut total = 0usize;
            for _ in 0..1000 {
                total += bitmap.popcount();
            }
            black_box(total)
        });
    });

    // Iterator throughput
    group.throughput(Throughput::Elements(1000));
    group.bench_function("iter_fired_1000", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in 0..100 {
            bitmap.set(i * 10, true);
        }

        b.iter(|| {
            let mut total = 0usize;
            for _ in 0..1000 {
                total += bitmap.iter_fired().count();
            }
            black_box(total)
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    throughput_benches,
    bench_syndrome_ingestion,
    bench_gate_decision_throughput,
    bench_permit_token_throughput,
    bench_receipt_log_throughput,
    bench_worker_tile_throughput,
    bench_filter_pipeline_throughput,
    bench_syndrome_delta_throughput,
    bench_patch_graph_throughput,
    bench_bitmap_throughput,
);

criterion_main!(throughput_benches);
