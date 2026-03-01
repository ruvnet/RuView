//! Benchmarks for report merging from 255 worker tiles
//!
//! Target latencies:
//! - Merge 255 tile reports: < 10ms

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::Rng;

use cognitum_gate_tilezero::{
    merge::{EdgeSummary, MergeStrategy, NodeSummary, ReportMerger, WorkerReport},
    TileId,
};

/// Create a realistic worker report with configurable complexity
fn create_worker_report(
    tile_id: TileId,
    epoch: u64,
    node_count: usize,
    boundary_edge_count: usize,
) -> WorkerReport {
    let mut rng = rand::thread_rng();
    let mut report = WorkerReport::new(tile_id, epoch);

    // Add nodes
    for i in 0..node_count {
        report.add_node(NodeSummary {
            id: format!("node-{}-{}", tile_id, i),
            weight: rng.gen_range(0.1..10.0),
            edge_count: rng.gen_range(5..50),
            coherence: rng.gen_range(0.7..1.0),
        });
    }

    // Add boundary edges
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

/// Create a batch of worker reports from all 255 tiles
fn create_all_tile_reports(
    epoch: u64,
    nodes_per_tile: usize,
    boundary_edges_per_tile: usize,
) -> Vec<WorkerReport> {
    (1..=255u8)
        .map(|tile_id| {
            create_worker_report(tile_id, epoch, nodes_per_tile, boundary_edges_per_tile)
        })
        .collect()
}

/// Benchmark merging 255 tile reports (target: < 10ms)
fn bench_merge_255_tiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_255_tiles");
    group.throughput(Throughput::Elements(255));

    // Test different merge strategies
    let strategies = vec![
        ("simple_average", MergeStrategy::SimpleAverage),
        ("weighted_average", MergeStrategy::WeightedAverage),
        ("median", MergeStrategy::Median),
        ("maximum", MergeStrategy::Maximum),
        ("byzantine_ft", MergeStrategy::ByzantineFaultTolerant),
    ];

    // Minimal reports (fast path)
    let minimal_reports = create_all_tile_reports(0, 1, 2);

    for (name, strategy) in &strategies {
        let merger = ReportMerger::new(*strategy);

        group.bench_with_input(
            BenchmarkId::new("minimal", name),
            &minimal_reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    // Realistic reports (10 nodes, 5 boundary edges per tile)
    let realistic_reports = create_all_tile_reports(0, 10, 5);

    for (name, strategy) in &strategies {
        let merger = ReportMerger::new(*strategy);

        group.bench_with_input(
            BenchmarkId::new("realistic", name),
            &realistic_reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    // Heavy reports (50 nodes, 20 boundary edges per tile)
    let heavy_reports = create_all_tile_reports(0, 50, 20);

    for (name, strategy) in &strategies {
        let merger = ReportMerger::new(*strategy);

        group.bench_with_input(
            BenchmarkId::new("heavy", name),
            &heavy_reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    group.finish();
}

/// Benchmark scaling with tile count
fn bench_merge_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("merge_scaling");

    for tile_count in [32, 64, 128, 192, 255] {
        group.throughput(Throughput::Elements(tile_count as u64));

        let reports: Vec<_> = (1..=tile_count as u8)
            .map(|tile_id| create_worker_report(tile_id, 0, 10, 5))
            .collect();

        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        group.bench_with_input(
            BenchmarkId::new("tiles", tile_count),
            &reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    group.finish();
}

/// Benchmark node merging specifically
fn bench_node_merging(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_merging");

    // Create reports with overlapping nodes (realistic for boundary merging)
    let create_overlapping_reports = |overlap_factor: usize| -> Vec<WorkerReport> {
        (1..=255u8)
            .map(|tile_id| {
                let mut report = WorkerReport::new(tile_id, 0);

                // Local nodes
                for i in 0..10 {
                    report.add_node(NodeSummary {
                        id: format!("local-{}-{}", tile_id, i),
                        weight: 1.0,
                        edge_count: 10,
                        coherence: 0.9,
                    });
                }

                // Shared/overlapping nodes
                for i in 0..overlap_factor {
                    report.add_node(NodeSummary {
                        id: format!("shared-{}", i),
                        weight: tile_id as f64 * 0.1,
                        edge_count: 5,
                        coherence: 0.95,
                    });
                }

                report
            })
            .collect()
    };

    for overlap in [0, 5, 10, 20] {
        let reports = create_overlapping_reports(overlap);
        let merger = ReportMerger::new(MergeStrategy::WeightedAverage);

        group.bench_with_input(
            BenchmarkId::new("overlap_nodes", overlap),
            &reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    group.finish();
}

/// Benchmark edge merging specifically
fn bench_edge_merging(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_merging");

    // Create reports with many boundary edges
    let create_edge_heavy_reports = |edges_per_tile: usize| -> Vec<WorkerReport> {
        (1..=255u8)
            .map(|tile_id| create_worker_report(tile_id, 0, 5, edges_per_tile))
            .collect()
    };

    for edge_count in [5, 10, 25, 50] {
        let reports = create_edge_heavy_reports(edge_count);
        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        // Total edges = 255 tiles * edges_per_tile
        group.throughput(Throughput::Elements((255 * edge_count) as u64));

        group.bench_with_input(
            BenchmarkId::new("edges_per_tile", edge_count),
            &reports,
            |b, reports| b.iter(|| black_box(merger.merge(black_box(reports)))),
        );
    }

    group.finish();
}

/// Benchmark state hash computation
fn bench_state_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("state_hash");
    group.throughput(Throughput::Elements(1));

    let small_report = create_worker_report(1, 0, 5, 2);
    let large_report = create_worker_report(1, 0, 100, 50);

    group.bench_function("compute_small", |b| {
        b.iter(|| {
            let mut report = small_report.clone();
            report.compute_state_hash();
            black_box(report.state_hash)
        })
    });

    group.bench_function("compute_large", |b| {
        b.iter(|| {
            let mut report = large_report.clone();
            report.compute_state_hash();
            black_box(report.state_hash)
        })
    });

    group.finish();
}

/// Benchmark global mincut estimation
fn bench_mincut_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("mincut_estimation");

    for tile_count in [64, 128, 255] {
        group.throughput(Throughput::Elements(tile_count as u64));

        let reports: Vec<_> = (1..=tile_count as u8)
            .map(|tile_id| create_worker_report(tile_id, 0, 10, 8))
            .collect();

        let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

        group.bench_with_input(
            BenchmarkId::new("tiles", tile_count),
            &reports,
            |b, reports| {
                b.iter(|| {
                    let merged = merger.merge(reports).unwrap();
                    black_box(merged.global_mincut_estimate)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark confidence aggregation
fn bench_confidence_aggregation(c: &mut Criterion) {
    let mut group = c.benchmark_group("confidence_aggregation");

    let strategies = vec![
        ("simple_average", MergeStrategy::SimpleAverage),
        ("byzantine_ft", MergeStrategy::ByzantineFaultTolerant),
    ];

    let reports = create_all_tile_reports(0, 5, 3);

    for (name, strategy) in strategies {
        let merger = ReportMerger::new(strategy);

        group.bench_with_input(
            BenchmarkId::new("strategy", name),
            &reports,
            |b, reports| {
                b.iter(|| {
                    let merged = merger.merge(reports).unwrap();
                    black_box(merged.confidence)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark epoch validation in merge
fn bench_epoch_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("epoch_validation");

    // All same epoch (should pass)
    let valid_reports = create_all_tile_reports(42, 5, 3);

    let merger = ReportMerger::new(MergeStrategy::SimpleAverage);

    group.bench_function("valid_epochs", |b| {
        b.iter(|| black_box(merger.merge(black_box(&valid_reports))))
    });

    // Mixed epochs (should fail fast)
    let mut invalid_reports = valid_reports.clone();
    invalid_reports[100] = create_worker_report(101, 43, 5, 3); // Different epoch

    group.bench_function("invalid_epochs", |b| {
        b.iter(|| black_box(merger.merge(black_box(&invalid_reports))))
    });

    group.finish();
}

/// Benchmark merged report access patterns
fn bench_merged_report_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("merged_report_access");

    let reports = create_all_tile_reports(0, 10, 5);
    let merger = ReportMerger::new(MergeStrategy::SimpleAverage);
    let merged = merger.merge(&reports).unwrap();

    group.bench_function("iterate_nodes", |b| {
        b.iter(|| {
            let sum: f64 = merged.super_nodes.values().map(|n| n.weight).sum();
            black_box(sum)
        })
    });

    group.bench_function("iterate_edges", |b| {
        b.iter(|| {
            let sum: f64 = merged.boundary_edges.iter().map(|e| e.capacity).sum();
            black_box(sum)
        })
    });

    group.bench_function("lookup_node", |b| {
        b.iter(|| black_box(merged.super_nodes.get("node-128-5")))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_merge_255_tiles,
    bench_merge_scaling,
    bench_node_merging,
    bench_edge_merging,
    bench_state_hash,
    bench_mincut_estimation,
    bench_confidence_aggregation,
    bench_epoch_validation,
    bench_merged_report_access,
);

criterion_main!(benches);
