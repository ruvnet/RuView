//! Scaling benchmarks for ruQu Coherence Gate.
//!
//! Measures how performance scales with:
//! - Code distance (5, 9, 13, 17, 21)
//! - Qubit count (50, 100, 500, 1000)
//! - Tile count (10, 50, 100, 255)
//! - Graph density
//!
//! Run with: `cargo bench -p ruqu --bench scaling_bench`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box as hint_black_box;

use ruqu::filters::{FilterConfig, FilterPipeline, SystemState};
use ruqu::syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeRound};
use ruqu::tile::{GateThresholds, PatchGraph, SyndromeDelta, TileReport, TileZero, WorkerTile};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Calculate approximate detector count for a surface code distance
fn detectors_for_distance(distance: usize) -> usize {
    // For surface code, detector count is roughly d^2
    distance * distance
}

/// Calculate approximate qubit count for a surface code distance
fn qubits_for_distance(distance: usize) -> usize {
    // For surface code, data qubits = 2*d^2 - 2*d + 1
    2 * distance * distance - 2 * distance + 1
}

/// Create a worker tile sized for a given qubit count
fn create_scaled_worker_tile(tile_id: u8, qubit_count: usize) -> WorkerTile {
    let mut tile = WorkerTile::new(tile_id);

    let vertices = (qubit_count / 4).min(255) as u16; // Tile handles a fraction of qubits
    let edges_per_vertex = 4; // Surface code connectivity

    for i in 0..vertices {
        tile.patch_graph.ensure_vertex(i);
    }

    let mut edges_added = 0u16;
    let max_edges = (vertices as usize * edges_per_vertex / 2).min(1000) as u16;

    'outer: for i in 0..vertices.saturating_sub(1) {
        // Lattice-like connectivity
        let neighbors = [i + 1, i.wrapping_add(vertices / 10)];
        for &neighbor in &neighbors {
            if neighbor < vertices && neighbor != i && edges_added < max_edges {
                if tile.patch_graph.add_edge(i, neighbor, 1000).is_some() {
                    edges_added += 1;
                }
            }
            if edges_added >= max_edges {
                break 'outer;
            }
        }
    }

    tile.patch_graph.recompute_components();
    tile
}

/// Create a filter pipeline sized for a given qubit count
fn create_scaled_filter_pipeline(qubit_count: usize) -> FilterPipeline {
    let config = FilterConfig::default();
    let mut pipeline = FilterPipeline::new(config);

    let vertices = qubit_count.min(500) as u64;

    // Add graph structure proportional to qubit count
    for i in 0..vertices.saturating_sub(1) {
        let _ = pipeline.structural_mut().insert_edge(i, i + 1, 1.0);
        if i % 10 == 0 && i + 10 < vertices {
            let _ = pipeline.structural_mut().insert_edge(i, i + 10, 0.5);
        }
    }
    pipeline.structural_mut().build();

    // Warm up shift filter
    let num_regions = (qubit_count / 16).min(64);
    for region in 0..num_regions {
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

// ============================================================================
// LATENCY VS CODE DISTANCE
// ============================================================================

/// Benchmark latency scaling with code distance
fn bench_latency_vs_distance(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_vs_distance");
    group.sample_size(100);

    let distances = [5, 9, 13, 17, 21];

    for distance in distances.iter() {
        let qubit_count = qubits_for_distance(*distance);
        let detector_count = detectors_for_distance(*distance);

        // Worker tile tick latency
        group.bench_with_input(
            BenchmarkId::new("worker_tick", format!("d{}", distance)),
            &qubit_count,
            |b, &qubits| {
                let mut tile = create_scaled_worker_tile(1, qubits);
                let delta = SyndromeDelta::new(0, 1, 100);

                b.iter(|| {
                    let report = tile.tick(black_box(&delta));
                    black_box(report)
                });
            },
        );

        // Filter pipeline evaluation latency
        group.bench_with_input(
            BenchmarkId::new("filter_pipeline", format!("d{}", distance)),
            &qubit_count,
            |b, &qubits| {
                let pipeline = create_scaled_filter_pipeline(qubits);
                let state = SystemState::new(qubits);

                b.iter(|| {
                    let result = pipeline.evaluate(black_box(&state));
                    black_box(result)
                });
            },
        );

        // Full decision cycle latency
        group.bench_with_input(
            BenchmarkId::new("full_decision", format!("d{}", distance)),
            &qubit_count,
            |b, &qubits| {
                let mut tile = create_scaled_worker_tile(1, qubits);
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);

                b.iter(|| {
                    let delta = SyndromeDelta::new(0, 1, 100);
                    let report = tile.tick(&delta);
                    let reports = vec![report; 10];
                    let decision = tilezero.merge_reports(reports);
                    black_box(decision)
                });
            },
        );

        // Syndrome buffer push latency
        group.bench_with_input(
            BenchmarkId::new("syndrome_push", format!("d{}", distance)),
            &detector_count,
            |b, &detectors| {
                let mut buffer = SyndromeBuffer::new(1024);
                let mut round_id = 0u64;

                b.iter(|| {
                    let mut bitmap = DetectorBitmap::new(detectors.min(1024));
                    for i in 0..detectors.min(1024) / 10 {
                        bitmap.set(i * 10, true);
                    }
                    let round = SyndromeRound::new(round_id, round_id, round_id * 1000, bitmap, 0);
                    buffer.push(round);
                    round_id += 1;
                    black_box(buffer.len())
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// LATENCY VS QUBIT COUNT
// ============================================================================

/// Benchmark latency scaling with qubit count
fn bench_latency_vs_qubit_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_vs_qubits");
    group.sample_size(100);

    let qubit_counts = [50, 100, 500, 1000];

    for qubit_count in qubit_counts.iter() {
        // Worker tile tick latency
        group.bench_with_input(
            BenchmarkId::new("worker_tick", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let mut tile = create_scaled_worker_tile(1, qubits);
                let delta = SyndromeDelta::new(0, 1, 100);

                b.iter(|| {
                    let report = tile.tick(black_box(&delta));
                    black_box(report)
                });
            },
        );

        // Filter pipeline evaluation
        group.bench_with_input(
            BenchmarkId::new("filter_pipeline", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let pipeline = create_scaled_filter_pipeline(qubits);
                let state = SystemState::new(qubits);

                b.iter(|| {
                    let result = pipeline.evaluate(black_box(&state));
                    black_box(result)
                });
            },
        );

        // Patch graph operations
        group.bench_with_input(
            BenchmarkId::new("patch_graph_estimate_cut", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let tile = create_scaled_worker_tile(1, qubits);

                b.iter(|| {
                    let cut = tile.patch_graph.estimate_local_cut();
                    black_box(cut)
                });
            },
        );

        // Component recomputation
        group.bench_with_input(
            BenchmarkId::new("recompute_components", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let mut tile = create_scaled_worker_tile(1, qubits);

                b.iter(|| {
                    tile.patch_graph.status |= PatchGraph::STATUS_DIRTY;
                    let count = tile.patch_graph.recompute_components();
                    black_box(count)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// LATENCY VS TILE COUNT
// ============================================================================

/// Benchmark latency scaling with tile count (TileZero merge)
fn bench_latency_vs_tile_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_vs_tiles");
    group.sample_size(100);

    let tile_counts = [10, 50, 100, 150, 200, 255];

    for tile_count in tile_counts.iter() {
        // TileZero merge latency
        group.bench_with_input(
            BenchmarkId::new("tilezero_merge", format!("t{}", tile_count)),
            tile_count,
            |b, &count| {
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);

                let reports: Vec<TileReport> = (1..=count)
                    .map(|i| {
                        let mut report = TileReport::new(i as u8);
                        report.local_cut = 10.0 + (i as f64 * 0.1);
                        report.shift_score = 0.1;
                        report.e_value = 200.0;
                        report.num_vertices = 100;
                        report.num_edges = 200;
                        report
                    })
                    .collect();

                b.iter(|| {
                    let decision = tilezero.merge_reports(black_box(reports.clone()));
                    black_box(decision)
                });
            },
        );

        // Full decision cycle with scaled tiles
        group.bench_with_input(
            BenchmarkId::new("full_decision", format!("t{}", tile_count)),
            tile_count,
            |b, &count| {
                let mut tile = create_scaled_worker_tile(1, 100);
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);

                b.iter(|| {
                    let delta = SyndromeDelta::new(0, 1, 100);
                    let report = tile.tick(&delta);
                    let reports = vec![report; count];
                    let decision = tilezero.merge_reports(reports);
                    black_box(decision)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// THROUGHPUT VS SYSTEM SIZE
// ============================================================================

/// Benchmark throughput scaling with system size
fn bench_throughput_vs_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput_vs_size");

    let qubit_counts = [50, 100, 500, 1000];

    for qubit_count in qubit_counts.iter() {
        // Syndrome ingestion throughput
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("syndrome_ingestion", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let mut buffer = SyndromeBuffer::new(4096);
                let detector_count = (qubits / 2).min(1024);
                let mut round_id = 0u64;

                b.iter(|| {
                    for _ in 0..1000 {
                        let mut bitmap = DetectorBitmap::new(detector_count);
                        for i in 0..detector_count / 10 {
                            bitmap.set(i * 10, true);
                        }
                        let round =
                            SyndromeRound::new(round_id, round_id, round_id * 1000, bitmap, 0);
                        buffer.push(round);
                        round_id += 1;
                    }
                    black_box(buffer.len())
                });
            },
        );

        // Decision throughput
        group.throughput(Throughput::Elements(100));
        group.bench_with_input(
            BenchmarkId::new("decision_throughput", format!("q{}", qubit_count)),
            qubit_count,
            |b, &qubits| {
                let mut tile = create_scaled_worker_tile(1, qubits);
                let thresholds = GateThresholds::default();
                let mut tilezero = TileZero::new(thresholds);

                b.iter(|| {
                    for i in 0..100 {
                        let delta = SyndromeDelta::new(0, 1, (i % 256) as u16);
                        let report = tile.tick(&delta);
                        let reports = vec![report; 10];
                        let decision = tilezero.merge_reports(reports);
                        hint_black_box(decision);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// GRAPH DENSITY SCALING
// ============================================================================

/// Benchmark latency scaling with graph density
fn bench_latency_vs_density(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_vs_density");
    group.sample_size(100);

    let base_vertices = 100u16;
    let densities = [
        ("sparse", base_vertices / 2),     // 0.5 edges per vertex
        ("linear", base_vertices),         // 1 edge per vertex
        ("lattice", base_vertices * 2),    // 2 edges per vertex
        ("dense", base_vertices * 4),      // 4 edges per vertex
        ("very_dense", base_vertices * 8), // 8 edges per vertex
    ];

    for (name, edge_count) in densities.iter() {
        // Worker tile tick
        group.bench_with_input(
            BenchmarkId::new("worker_tick", *name),
            edge_count,
            |b, &edges| {
                let mut tile = WorkerTile::new(1);

                for i in 0..base_vertices {
                    tile.patch_graph.ensure_vertex(i);
                }

                let mut added = 0u16;
                'outer: for i in 0..base_vertices {
                    for j in (i + 1)..base_vertices.min(i + 10) {
                        if added >= edges {
                            break 'outer;
                        }
                        if tile.patch_graph.add_edge(i, j, 1000).is_some() {
                            added += 1;
                        }
                    }
                }
                tile.patch_graph.recompute_components();

                let delta = SyndromeDelta::new(0, 1, 100);

                b.iter(|| {
                    let report = tile.tick(black_box(&delta));
                    black_box(report)
                });
            },
        );

        // Local cut estimation
        group.bench_with_input(
            BenchmarkId::new("estimate_local_cut", *name),
            edge_count,
            |b, &edges| {
                let mut graph = PatchGraph::new();

                for i in 0..base_vertices {
                    graph.ensure_vertex(i);
                }

                let mut added = 0u16;
                'outer: for i in 0..base_vertices {
                    for j in (i + 1)..base_vertices.min(i + 10) {
                        if added >= edges {
                            break 'outer;
                        }
                        if graph.add_edge(i, j, 1000).is_some() {
                            added += 1;
                        }
                    }
                }
                graph.recompute_components();

                b.iter(|| {
                    let cut = graph.estimate_local_cut();
                    black_box(cut)
                });
            },
        );

        // Component recomputation
        group.bench_with_input(
            BenchmarkId::new("recompute_components", *name),
            edge_count,
            |b, &edges| {
                let mut graph = PatchGraph::new();

                for i in 0..base_vertices {
                    graph.ensure_vertex(i);
                }

                let mut added = 0u16;
                'outer: for i in 0..base_vertices {
                    for j in (i + 1)..base_vertices.min(i + 10) {
                        if added >= edges {
                            break 'outer;
                        }
                        if graph.add_edge(i, j, 1000).is_some() {
                            added += 1;
                        }
                    }
                }

                b.iter(|| {
                    graph.status |= PatchGraph::STATUS_DIRTY;
                    let count = graph.recompute_components();
                    black_box(count)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// MEMORY PRESSURE SCALING
// ============================================================================

/// Benchmark under memory pressure (large buffers)
fn bench_memory_pressure(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pressure");
    group.sample_size(50);

    let buffer_sizes = [1024, 4096, 16384, 65536];

    for buffer_size in buffer_sizes.iter() {
        // Syndrome buffer under pressure
        group.throughput(Throughput::Elements(1000));
        group.bench_with_input(
            BenchmarkId::new("syndrome_buffer", format!("cap{}", buffer_size)),
            buffer_size,
            |b, &size| {
                let mut buffer = SyndromeBuffer::new(size);
                // Pre-fill to capacity
                for i in 0..(size as u64) {
                    let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
                    buffer.push(round);
                }

                let mut round_id = size as u64;
                b.iter(|| {
                    for _ in 0..1000 {
                        let round = SyndromeRound::new(
                            round_id,
                            round_id,
                            round_id * 1000,
                            DetectorBitmap::new(64),
                            0,
                        );
                        buffer.push(round);
                        round_id += 1;
                    }
                    black_box(buffer.len())
                });
            },
        );

        // Window extraction under pressure
        group.bench_with_input(
            BenchmarkId::new("window_extraction", format!("cap{}", buffer_size)),
            buffer_size,
            |b, &size| {
                let mut buffer = SyndromeBuffer::new(size);
                for i in 0..(size as u64) {
                    let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
                    buffer.push(round);
                }

                let window_size = (size / 10).max(10);
                b.iter(|| {
                    let window = buffer.window(window_size);
                    black_box(window)
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    scaling_benches,
    bench_latency_vs_distance,
    bench_latency_vs_qubit_count,
    bench_latency_vs_tile_count,
    bench_throughput_vs_size,
    bench_latency_vs_density,
    bench_memory_pressure,
);

criterion_main!(scaling_benches);
