//! Critical path latency benchmarks for ruQu Coherence Gate.
//!
//! Primary performance target: **sub-4μs gate decision latency (p99)**
//!
//! Latency Budget (Target: <4μs p99):
//! ```text
//! Syndrome Arrival        → 0 ns
//! Ring buffer append      → +50 ns
//! Graph update            → +200 ns (amortized O(n^{o(1)}))
//! Worker Tick             → +500 ns (local cut eval)
//! Report generation       → +100 ns
//! TileZero Merge          → +500 ns (parallel from 255 tiles)
//! Global cut              → +300 ns
//! Three-filter eval       → +100 ns
//! Token signing           → +500 ns (Ed25519)
//! Receipt append          → +100 ns
//! ─────────────────────────────────
//! Total                   → ~2,350 ns
//! ```
//!
//! Run with: `cargo bench -p ruqu --bench latency_bench`

use criterion::{
    black_box, criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, BenchmarkId,
    Criterion, SamplingMode,
};

use ruqu::filters::{
    EvidenceAccumulator as FilterEvidenceAccumulator, EvidenceFilter, FilterConfig, FilterPipeline,
    ShiftFilter, StructuralFilter, SystemState,
};
use ruqu::tile::{
    GateDecision, GateThresholds, LocalCutState, PatchGraph, SyndromeDelta, TileReport, TileZero,
    WorkerTile,
};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Create a pre-populated worker tile for benchmarking
fn create_benchmark_worker_tile(tile_id: u8, num_vertices: u16, num_edges: u16) -> WorkerTile {
    let mut tile = WorkerTile::new(tile_id);

    // Add vertices and edges to the patch graph
    for i in 0..num_vertices.min(255) {
        tile.patch_graph.ensure_vertex(i);
    }

    // Add edges in a mesh pattern
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

/// Create a pre-populated filter pipeline for benchmarking
fn create_benchmark_filter_pipeline() -> FilterPipeline {
    let config = FilterConfig::default();
    let mut pipeline = FilterPipeline::new(config);

    // Add graph structure
    for i in 0..50u64 {
        let _ = pipeline.structural_mut().insert_edge(i, i + 1, 1.0);
    }
    pipeline.structural_mut().build();

    // Warm up shift filter with observations
    for region in 0..10 {
        for _ in 0..50 {
            pipeline.shift_mut().update(region, 0.5);
        }
    }

    // Warm up evidence filter
    for _ in 0..20 {
        pipeline.evidence_mut().update(1.5);
    }

    pipeline
}

/// Create benchmark tile reports
fn create_benchmark_tile_reports(count: usize) -> Vec<TileReport> {
    (1..=count)
        .map(|i| {
            let mut report = TileReport::new(i as u8);
            report.local_cut = 10.0 + (i as f64 * 0.1);
            report.shift_score = 0.1 + (i as f64 * 0.01);
            report.e_value = 100.0 + (i as f64);
            report.num_vertices = 100;
            report.num_edges = 200;
            report.num_components = 1;
            report
        })
        .collect()
}

// ============================================================================
// GATE DECISION LATENCY (Critical Path)
// ============================================================================

/// Benchmark the full decision cycle - the critical <4μs path
fn bench_gate_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("gate_decision");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Full decision cycle: worker tick + tilezero merge
    group.bench_function("full_cycle", |b| {
        let mut tile = create_benchmark_worker_tile(1, 64, 128);
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        b.iter(|| {
            // 1. Worker tick - process syndrome delta
            let delta = SyndromeDelta::new(0, 1, 100);
            let report = tile.tick(&delta);

            // 2. TileZero merge reports (simulating all 255 tiles with the same report)
            let reports = vec![report; 10]; // Reduced for single-threaded benchmark
            let decision = tilezero.merge_reports(reports);

            black_box(decision)
        });
    });

    // Worker tick only
    group.bench_function("worker_tick_only", |b| {
        let mut tile = create_benchmark_worker_tile(1, 64, 128);
        let delta = SyndromeDelta::new(0, 1, 100);

        b.iter(|| {
            let report = tile.tick(black_box(&delta));
            black_box(report)
        });
    });

    // TileZero merge only
    group.bench_function("tilezero_merge_only", |b| {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);
        let reports = create_benchmark_tile_reports(255);

        b.iter(|| {
            let decision = tilezero.merge_reports(black_box(reports.clone()));
            black_box(decision)
        });
    });

    // TileZero merge with varying tile counts
    for tile_count in [10, 50, 100, 255].iter() {
        group.bench_with_input(
            BenchmarkId::new("tilezero_merge_tiles", tile_count),
            tile_count,
            |b, &count| {
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);
                let reports = create_benchmark_tile_reports(count);

                b.iter(|| {
                    let decision = tilezero.merge_reports(black_box(reports.clone()));
                    black_box(decision)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// INDIVIDUAL FILTER EVALUATION LATENCY
// ============================================================================

/// Benchmark structural (min-cut) filter evaluation
fn bench_structural_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("structural_filter");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Basic evaluation with small graph
    group.bench_function("evaluate_small", |b| {
        let mut filter = StructuralFilter::new(2.0);
        for i in 0..20u64 {
            let _ = filter.insert_edge(i, i + 1, 1.0);
        }
        filter.build();
        let state = SystemState::new(20);

        b.iter(|| {
            let result = filter.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Evaluation with medium graph
    group.bench_function("evaluate_medium", |b| {
        let mut filter = StructuralFilter::new(2.0);
        for i in 0..100u64 {
            let _ = filter.insert_edge(i, (i + 1) % 100, 1.0);
            let _ = filter.insert_edge(i, (i + 50) % 100, 0.5);
        }
        filter.build();
        let state = SystemState::new(100);

        b.iter(|| {
            let result = filter.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Edge insertion (hot path during updates)
    group.bench_function("insert_edge", |b| {
        b.iter_batched(
            || (StructuralFilter::new(2.0), 0u64),
            |(mut filter, mut edge_id)| {
                for _ in 0..100 {
                    let u = edge_id % 256;
                    let v = (edge_id + 1) % 256;
                    let _ = filter.insert_edge(u, v, 1.0);
                    edge_id += 2;
                }
                black_box(edge_id)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Edge deletion
    group.bench_function("delete_edge", |b| {
        b.iter_batched(
            || {
                let mut filter = StructuralFilter::new(2.0);
                for i in 0..100u64 {
                    let _ = filter.insert_edge(i, i + 1, 1.0);
                }
                filter
            },
            |mut filter| {
                let result = filter.delete_edge(50, 51);
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

/// Benchmark shift (drift detection) filter evaluation
fn bench_shift_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("shift_filter");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Evaluate with warm filter
    group.bench_function("evaluate_warm", |b| {
        let mut filter = ShiftFilter::new(0.5, 100);
        // Warm up with observations
        for region in 0..64 {
            for _ in 0..100 {
                filter.update(region, 0.5 + (region as f64 * 0.001));
            }
        }
        let state = SystemState::new(100);

        b.iter(|| {
            let result = filter.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Evaluate with cold filter
    group.bench_function("evaluate_cold", |b| {
        let filter = ShiftFilter::new(0.5, 100);
        let state = SystemState::new(100);

        b.iter(|| {
            let result = filter.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Single update operation
    group.bench_function("update_single", |b| {
        let mut filter = ShiftFilter::new(0.5, 100);
        let mut i = 0usize;

        b.iter(|| {
            filter.update(black_box(i % 64), black_box(0.5));
            i += 1;
        });
    });

    // Batch update (64 regions)
    group.bench_function("update_batch_64", |b| {
        let mut filter = ShiftFilter::new(0.5, 100);

        b.iter(|| {
            for region in 0..64 {
                filter.update(black_box(region), black_box(0.5));
            }
        });
    });

    group.finish();
}

/// Benchmark evidence (e-value) filter evaluation
fn bench_evidence_filter(c: &mut Criterion) {
    let mut group = c.benchmark_group("evidence_filter");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Evaluate with accumulated evidence
    group.bench_function("evaluate_accumulated", |b| {
        let mut filter = EvidenceFilter::new(20.0, 0.05);
        for _ in 0..100 {
            filter.update(1.5);
        }
        let state = SystemState::new(100);

        b.iter(|| {
            let result = filter.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Single evidence update
    group.bench_function("update_single", |b| {
        let mut filter = EvidenceFilter::new(20.0, 0.05);

        b.iter(|| {
            filter.update(black_box(1.5));
        });
    });

    // Evidence accumulator operations
    group.bench_function("accumulator_observe", |b| {
        let mut accumulator = FilterEvidenceAccumulator::new();

        b.iter(|| {
            accumulator.update(black_box(1.5));
        });
    });

    group.bench_function("accumulator_e_value", |b| {
        let mut accumulator = FilterEvidenceAccumulator::new();
        for _ in 0..100 {
            accumulator.update(1.5);
        }

        b.iter(|| {
            let e = accumulator.e_value();
            black_box(e)
        });
    });

    group.finish();
}

// ============================================================================
// TILE PROCESSING LATENCY
// ============================================================================

/// Benchmark worker tile tick processing
fn bench_worker_tile_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("worker_tile_tick");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Tick with syndrome delta
    group.bench_function("tick_syndrome", |b| {
        let mut tile = create_benchmark_worker_tile(1, 64, 128);
        let delta = SyndromeDelta::new(0, 1, 100);

        b.iter(|| {
            let report = tile.tick(black_box(&delta));
            black_box(report)
        });
    });

    // Tick with edge addition
    group.bench_function("tick_edge_add", |b| {
        let mut tile = create_benchmark_worker_tile(1, 64, 128);
        let delta = SyndromeDelta::edge_add(10, 20, 1000);

        b.iter(|| {
            let report = tile.tick(black_box(&delta));
            black_box(report)
        });
    });

    // Tick with edge removal
    group.bench_function("tick_edge_remove", |b| {
        b.iter_batched(
            || {
                let mut tile = create_benchmark_worker_tile(1, 64, 128);
                // Add edge before removing
                let _ = tile.patch_graph.add_edge(5, 6, 1000);
                (tile, SyndromeDelta::edge_remove(5, 6))
            },
            |(mut tile, delta)| {
                let report = tile.tick(&delta);
                black_box(report)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Varying graph sizes
    for (vertices, edges) in [(32, 64), (64, 128), (128, 256), (200, 400)].iter() {
        group.bench_with_input(
            BenchmarkId::new("tick_graph_size", format!("v{}e{}", vertices, edges)),
            &(*vertices, *edges),
            |b, &(v, e)| {
                let mut tile = create_benchmark_worker_tile(1, v, e);
                let delta = SyndromeDelta::new(0, 1, 100);

                b.iter(|| {
                    let report = tile.tick(black_box(&delta));
                    black_box(report)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark TileZero merge operations
fn bench_tilezero_merge(c: &mut Criterion) {
    let mut group = c.benchmark_group("tilezero_merge");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Merge leading to PERMIT
    group.bench_function("merge_permit", |b| {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=100)
            .map(|i| {
                let mut report = TileReport::new(i as u8);
                report.local_cut = 10.0;
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        b.iter(|| {
            let decision = tilezero.merge_reports(black_box(reports.clone()));
            debug_assert_eq!(decision, GateDecision::Permit);
            black_box(decision)
        });
    });

    // Merge leading to DENY (structural)
    group.bench_function("merge_deny_structural", |b| {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=100)
            .map(|i| {
                let mut report = TileReport::new(i as u8);
                report.local_cut = 1.0; // Below threshold
                report.shift_score = 0.1;
                report.e_value = 200.0;
                report
            })
            .collect();

        b.iter(|| {
            let decision = tilezero.merge_reports(black_box(reports.clone()));
            debug_assert_eq!(decision, GateDecision::Deny);
            black_box(decision)
        });
    });

    // Merge leading to DEFER (shift)
    group.bench_function("merge_defer_shift", |b| {
        let thresholds = GateThresholds::default();
        let mut tilezero = TileZero::new(thresholds);

        let reports: Vec<TileReport> = (1..=100)
            .map(|i| {
                let mut report = TileReport::new(i as u8);
                report.local_cut = 10.0;
                report.shift_score = 0.8; // Above threshold
                report.e_value = 200.0;
                report
            })
            .collect();

        b.iter(|| {
            let decision = tilezero.merge_reports(black_box(reports.clone()));
            debug_assert_eq!(decision, GateDecision::Defer);
            black_box(decision)
        });
    });

    // Permit token issuance
    group.bench_function("issue_permit", |b| {
        let thresholds = GateThresholds::default();
        let tilezero = TileZero::new(thresholds);
        let decision = GateDecision::Permit;

        b.iter(|| {
            let token = tilezero.issue_permit(black_box(&decision));
            black_box(token)
        });
    });

    group.finish();
}

// ============================================================================
// PATCH GRAPH LATENCY
// ============================================================================

/// Benchmark patch graph operations (critical for structural filter)
fn bench_patch_graph_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("patch_graph");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Edge addition
    group.bench_function("add_edge", |b| {
        b.iter_batched(
            PatchGraph::new,
            |mut graph| {
                for edge_count in 0..100u16 {
                    let v1 = (edge_count * 2) % 256;
                    let v2 = (edge_count * 2 + 1) % 256;
                    let _ = graph.add_edge(v1, v2, 1000);
                }
                black_box(graph.num_edges)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Edge removal
    group.bench_function("remove_edge", |b| {
        b.iter_batched(
            || {
                let mut graph = PatchGraph::new();
                for i in 0..100u16 {
                    let _ = graph.add_edge(i, i + 1, 1000);
                }
                graph
            },
            |mut graph| {
                let removed = graph.remove_edge(50, 51);
                black_box(removed)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // Local cut estimation
    group.bench_function("estimate_local_cut", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
            let _ = graph.add_edge(i, (i + 50) % 100, 500);
        }
        graph.recompute_components();

        b.iter(|| {
            let cut = graph.estimate_local_cut();
            black_box(cut)
        });
    });

    // Component recomputation
    group.bench_function("recompute_components", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
        }

        b.iter(|| {
            graph.status |= PatchGraph::STATUS_DIRTY;
            let count = graph.recompute_components();
            black_box(count)
        });
    });

    // Boundary candidate identification
    group.bench_function("identify_boundary_candidates", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
        }
        graph.recompute_components();
        let mut candidates = [0u16; 64];

        b.iter(|| {
            let count = graph.identify_boundary_candidates(&mut candidates);
            black_box(count)
        });
    });

    group.finish();
}

// ============================================================================
// LOCAL CUT STATE LATENCY
// ============================================================================

/// Benchmark local cut state operations
fn bench_local_cut_state(c: &mut Criterion) {
    let mut group = c.benchmark_group("local_cut_state");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Update from graph
    group.bench_function("update_from_graph", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
        }
        graph.recompute_components();

        let mut cut_state = LocalCutState::new();

        b.iter(|| {
            cut_state.update_from_graph(&graph);
            black_box(cut_state.cut_value)
        });
    });

    group.finish();
}

// ============================================================================
// FILTER PIPELINE LATENCY
// ============================================================================

/// Benchmark full filter pipeline evaluation
fn bench_filter_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("filter_pipeline");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(1000);

    // Full evaluation
    group.bench_function("evaluate_full", |b| {
        let pipeline = create_benchmark_filter_pipeline();
        let state = SystemState::new(100);

        b.iter(|| {
            let result = pipeline.evaluate(black_box(&state));
            black_box(result)
        });
    });

    // Cold start evaluation
    group.bench_function("evaluate_cold", |b| {
        b.iter_batched(
            || {
                let config = FilterConfig::default();
                let pipeline = FilterPipeline::new(config);
                let state = SystemState::new(100);
                (pipeline, state)
            },
            |(pipeline, state)| {
                let result = pipeline.evaluate(&state);
                black_box(result)
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    latency_benches,
    bench_gate_decision,
    bench_structural_filter,
    bench_shift_filter,
    bench_evidence_filter,
    bench_worker_tile_tick,
    bench_tilezero_merge,
    bench_patch_graph_operations,
    bench_local_cut_state,
    bench_filter_pipeline,
);

criterion_main!(latency_benches);
