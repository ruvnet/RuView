//! Comprehensive benchmarks for cognitum-gate-kernel
//!
//! Target latencies:
//! - Single edge insert: < 100ns
//! - Batch 1000 edges: < 100us
//! - Single tick: < 500us
//! - Tick under 10K edges: < 5ms
//! - TileReport serialization: < 1us
//! - E-value update: < 50ns
//! - Mixture e-value (SIMD): < 500ns for 16 hypotheses

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

use cognitum_gate_kernel::{
    delta::{Delta, Observation},
    evidence::{
        f32_to_log_e, EvidenceAccumulator, HypothesisState, LogEValue, LOG_LR_CONNECTIVITY_POS,
    },
    report::TileReport,
    shard::{CompactGraph, MAX_SHARD_VERTICES},
    TileState, MAX_DELTA_BUFFER,
};

// ============================================================================
// Edge Operations Benchmarks
// ============================================================================

/// Benchmark single edge insertion
fn bench_edge_insert(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_operations");
    group.throughput(Throughput::Elements(1));

    // Benchmark on empty graph
    group.bench_function("insert_single_empty", |b| {
        b.iter_batched(
            CompactGraph::new,
            |mut graph| {
                black_box(graph.add_edge(0, 1, 100));
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark on partially filled graph
    group.bench_function("insert_single_partial", |b| {
        b.iter_batched(
            || {
                let mut graph = CompactGraph::new();
                for i in 0..100u16 {
                    graph.add_edge(i, i + 1, 100);
                }
                graph
            },
            |mut graph| {
                black_box(graph.add_edge(200, 201, 100));
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark edge removal
    group.bench_function("remove_single", |b| {
        b.iter_batched(
            || {
                let mut graph = CompactGraph::new();
                graph.add_edge(0, 1, 100);
                graph.add_edge(1, 2, 100);
                graph.add_edge(2, 3, 100);
                graph
            },
            |mut graph| {
                black_box(graph.remove_edge(1, 2));
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // Benchmark edge lookup
    group.bench_function("find_edge", |b| {
        let mut graph = CompactGraph::new();
        for i in 0..200u16 {
            graph.add_edge(i, i + 1, 100);
        }
        b.iter(|| black_box(graph.find_edge(100, 101)))
    });

    // Benchmark weight update
    group.bench_function("update_weight", |b| {
        let mut graph = CompactGraph::new();
        for i in 0..100u16 {
            graph.add_edge(i, i + 1, 100);
        }
        b.iter(|| {
            black_box(graph.update_weight(50, 51, 200));
        })
    });

    group.finish();
}

/// Benchmark batch edge insertion (1000 edges)
fn bench_edge_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_batch");

    for batch_size in [100, 500, 1000] {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("insert_batch", batch_size),
            &batch_size,
            |b, &size| {
                b.iter_batched(
                    CompactGraph::new,
                    |mut graph| {
                        for i in 0..size as u16 {
                            // Use modular arithmetic to create varied edges within bounds
                            let src = i % 200;
                            let dst = (i % 200) + 1;
                            graph.add_edge(src, dst, 100);
                        }
                        black_box(graph)
                    },
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Benchmark batch with recompute_components
    group.bench_function("batch_1000_with_components", |b| {
        b.iter_batched(
            CompactGraph::new,
            |mut graph| {
                for i in 0..500u16 {
                    let src = i % 200;
                    let dst = (i % 200) + 1;
                    graph.add_edge(src, dst, 100);
                }
                graph.recompute_components();
                black_box(graph)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// Tick Cycle Benchmarks
// ============================================================================

/// Benchmark single tick cycle
fn bench_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_cycle");
    group.throughput(Throughput::Elements(1));

    // Empty tick (no deltas)
    group.bench_function("tick_empty", |b| {
        let mut tile = TileState::new(0);
        b.iter(|| black_box(tile.tick(black_box(1))))
    });

    // Tick with small graph
    group.bench_function("tick_small_graph", |b| {
        let mut tile = TileState::new(0);
        // Add some edges
        for i in 0..10u16 {
            tile.ingest_delta(&Delta::edge_add(i, i + 1, 100));
        }
        tile.tick(0); // Initial tick to process deltas

        b.iter(|| black_box(tile.tick(black_box(1))))
    });

    // Tick with pending deltas
    group.bench_function("tick_with_deltas", |b| {
        b.iter_batched(
            || {
                let mut tile = TileState::new(0);
                for i in 0..10u16 {
                    tile.ingest_delta(&Delta::edge_add(i, i + 1, 100));
                }
                tile
            },
            |mut tile| black_box(tile.tick(1)),
            criterion::BatchSize::SmallInput,
        )
    });

    // Tick with observations
    group.bench_function("tick_with_observations", |b| {
        b.iter_batched(
            || {
                let mut tile = TileState::new(0);
                tile.evidence.add_connectivity_hypothesis(5);
                for _ in 0..5 {
                    let obs = Observation::connectivity(5, true);
                    tile.ingest_delta(&Delta::observation(obs));
                }
                tile
            },
            |mut tile| black_box(tile.tick(1)),
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// Benchmark tick under heavy load (10K edges simulated via max graph)
fn bench_tick_under_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("tick_under_load");
    group.sample_size(50); // Reduce sample size for expensive benchmarks

    // Create a densely connected graph (approaching limits)
    for edge_count in [500, 800, 1000] {
        group.throughput(Throughput::Elements(edge_count as u64));

        group.bench_with_input(
            BenchmarkId::new("edges", edge_count),
            &edge_count,
            |b, &count| {
                b.iter_batched(
                    || {
                        let mut tile = TileState::new(0);
                        // Create a connected graph
                        for i in 0..count.min(1000) as u16 {
                            let src = i % 250;
                            let dst = (i + 1) % 250;
                            if src != dst {
                                tile.ingest_delta(&Delta::edge_add(src, dst, 100));
                            }
                        }
                        tile.tick(0); // Process initial deltas

                        // Add some pending work
                        tile.ingest_delta(&Delta::edge_add(0, 100, 150));
                        tile.ingest_delta(&Delta::observation(Observation::connectivity(0, true)));
                        tile
                    },
                    |mut tile| black_box(tile.tick(1)),
                    criterion::BatchSize::SmallInput,
                )
            },
        );
    }

    // Benchmark connected components recomputation at scale
    group.bench_function("recompute_components_800", |b| {
        b.iter_batched(
            || {
                let mut graph = CompactGraph::new();
                // Create 4 disconnected clusters of 50 nodes each
                for cluster in 0..4u16 {
                    let base = cluster * 60;
                    for i in 0..50u16 {
                        graph.add_edge(base + i, base + (i + 1) % 50, 100);
                    }
                }
                graph
            },
            |mut graph| {
                black_box(graph.recompute_components());
                graph
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

// ============================================================================
// Report Serialization Benchmarks
// ============================================================================

/// Benchmark TileReport serialization
fn bench_report_serialize(c: &mut Criterion) {
    let mut group = c.benchmark_group("report_serialization");
    group.throughput(Throughput::Elements(1));

    // Create a populated tile report
    let create_report = || {
        let mut tile = TileState::new(42);
        for i in 0..20u16 {
            tile.ingest_delta(&Delta::edge_add(i, i + 1, 100));
        }
        tile.tick(1)
    };

    let report = create_report();

    // Raw memory copy (baseline)
    group.bench_function("raw_copy_64_bytes", |b| {
        let report = create_report();
        b.iter(|| {
            let mut buffer = [0u8; 64];
            unsafe {
                let src = &report as *const TileReport as *const u8;
                core::ptr::copy_nonoverlapping(src, buffer.as_mut_ptr(), 64);
            }
            black_box(buffer)
        })
    });

    // Report creation from scratch
    group.bench_function("create_new", |b| {
        b.iter(|| black_box(TileReport::new(black_box(42))))
    });

    // Report field access patterns
    group.bench_function("access_witness", |b| {
        b.iter(|| black_box(report.get_witness()))
    });

    group.bench_function("access_connected", |b| {
        b.iter(|| black_box(report.is_connected()))
    });

    group.bench_function("e_value_approx", |b| {
        b.iter(|| black_box(report.e_value_approx()))
    });

    group.finish();
}

// ============================================================================
// E-Value Computation Benchmarks
// ============================================================================

/// Benchmark e-value accumulator update
fn bench_evalue_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("evalue_update");
    group.throughput(Throughput::Elements(1));

    // Single hypothesis update
    group.bench_function("hypothesis_update_f32", |b| {
        let mut hyp = HypothesisState::new(0, HypothesisState::TYPE_CONNECTIVITY);
        b.iter(|| black_box(hyp.update(black_box(1.5))))
    });

    // Update with pre-computed log LR (faster path)
    group.bench_function("hypothesis_update_log_lr", |b| {
        let mut hyp = HypothesisState::new(0, HypothesisState::TYPE_CONNECTIVITY);
        b.iter(|| black_box(hyp.update_with_log_lr(black_box(LOG_LR_CONNECTIVITY_POS))))
    });

    // f32 to log conversion
    group.bench_function("f32_to_log_e", |b| {
        b.iter(|| black_box(f32_to_log_e(black_box(1.5))))
    });

    // f32 to log with common value (fast path)
    group.bench_function("f32_to_log_e_fast_path", |b| {
        b.iter(|| black_box(f32_to_log_e(black_box(2.0))))
    });

    // Full accumulator observation processing
    group.bench_function("accumulator_process_obs", |b| {
        let mut acc = EvidenceAccumulator::new();
        acc.add_connectivity_hypothesis(5);
        let obs = Observation::connectivity(5, true);

        b.iter(|| {
            acc.process_observation(black_box(obs), black_box(1));
        })
    });

    // Multiple hypotheses
    for hyp_count in [1, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("process_obs_hypotheses", hyp_count),
            &hyp_count,
            |b, &count| {
                let mut acc = EvidenceAccumulator::new();
                for v in 0..count as u16 {
                    acc.add_connectivity_hypothesis(v);
                }
                let obs = Observation::connectivity(0, true);

                b.iter(|| {
                    acc.process_observation(black_box(obs), black_box(1));
                })
            },
        );
    }

    group.finish();
}

/// Benchmark mixture e-value computation (potential SIMD opportunity)
fn bench_mixture_evalue(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixture_evalue");

    // Simulated mixture: aggregate multiple log e-values
    // This is where SIMD can provide significant speedup

    // Scalar baseline
    group.bench_function("aggregate_16_scalar", |b| {
        let log_e_values: [LogEValue; 16] = [
            65536, 38550, -65536, 65536, 38550, 65536, 38550, -32768, 65536, 65536, 38550, -65536,
            65536, 38550, 65536, 38550,
        ];

        b.iter(|| {
            let sum: LogEValue = log_e_values.iter().copied().sum();
            black_box(sum)
        })
    });

    // Parallel lanes pattern (SIMD-friendly)
    group.bench_function("aggregate_16_parallel_lanes", |b| {
        let log_e_values: [LogEValue; 16] = [
            65536, 38550, -65536, 65536, 38550, 65536, 38550, -32768, 65536, 65536, 38550, -65536,
            65536, 38550, 65536, 38550,
        ];

        b.iter(|| {
            // Process in 4 lanes (potential SIMD with 128-bit registers)
            let mut lanes = [0i32; 4];
            for (i, &val) in log_e_values.iter().enumerate() {
                lanes[i % 4] = lanes[i % 4].saturating_add(val);
            }
            let sum = lanes.iter().sum::<i32>();
            black_box(sum)
        })
    });

    // Chunked processing (auto-vectorization friendly)
    group.bench_function("aggregate_16_chunked", |b| {
        let log_e_values: [LogEValue; 16] = [
            65536, 38550, -65536, 65536, 38550, 65536, 38550, -32768, 65536, 65536, 38550, -65536,
            65536, 38550, 65536, 38550,
        ];

        b.iter(|| {
            let mut total = 0i32;
            for chunk in log_e_values.chunks(4) {
                let chunk_sum: i32 = chunk.iter().copied().sum();
                total = total.saturating_add(chunk_sum);
            }
            black_box(total)
        })
    });

    // Scale to 255 tiles (realistic workload)
    group.bench_function("aggregate_255_tiles", |b| {
        let log_e_values: Vec<LogEValue> = (0..255)
            .map(|i| (i as i32 % 3 - 1) * 65536) // Varying positive/negative evidence
            .collect();

        b.iter(|| {
            let sum: i64 = log_e_values.iter().map(|&v| v as i64).sum();
            black_box(sum)
        })
    });

    // Mixture with product (exp-log pattern)
    group.bench_function("mixture_product_16", |b| {
        let log_e_values: [LogEValue; 16] = [
            65536, 38550, -65536, 65536, 38550, 65536, 38550, -32768, 65536, 65536, 38550, -65536,
            65536, 38550, 65536, 38550,
        ];

        b.iter(|| {
            // For product, sum the logs, then exp
            let log_sum: i64 = log_e_values.iter().map(|&v| v as i64).sum();
            // Approximate exp2 for final result
            let approx_result = (log_sum as f64) / 65536.0;
            black_box(approx_result)
        })
    });

    group.finish();
}

// ============================================================================
// Additional Performance Benchmarks
// ============================================================================

/// Benchmark delta ingestion
fn bench_delta_ingestion(c: &mut Criterion) {
    let mut group = c.benchmark_group("delta_ingestion");
    group.throughput(Throughput::Elements(1));

    group.bench_function("ingest_single", |b| {
        let mut tile = TileState::new(0);
        let delta = Delta::edge_add(0, 1, 100);

        b.iter(|| {
            tile.reset();
            black_box(tile.ingest_delta(&delta))
        })
    });

    // Fill buffer benchmark
    group.bench_function("fill_buffer_64", |b| {
        b.iter_batched(
            || TileState::new(0),
            |mut tile| {
                for i in 0..MAX_DELTA_BUFFER as u16 {
                    tile.ingest_delta(&Delta::edge_add(i, i + 1, 100));
                }
                black_box(tile)
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

/// Benchmark neighbor iteration
fn bench_neighbor_iteration(c: &mut Criterion) {
    let mut group = c.benchmark_group("neighbor_iteration");

    // Create a graph with varying degree vertices
    let mut graph = CompactGraph::new();
    // Create a hub vertex with many neighbors
    for i in 1..25u16 {
        graph.add_edge(0, i, 100);
    }
    // Create a chain
    for i in 30..50u16 {
        graph.add_edge(i, i + 1, 100);
    }

    group.bench_function("neighbors_hub_24", |b| {
        b.iter(|| {
            let neighbors = graph.neighbors(0);
            black_box(neighbors.len())
        })
    });

    group.bench_function("neighbors_chain_2", |b| {
        b.iter(|| {
            let neighbors = graph.neighbors(35);
            black_box(neighbors.len())
        })
    });

    group.bench_function("iterate_all_neighbors", |b| {
        b.iter(|| {
            let mut total = 0usize;
            for v in 0..50u16 {
                total += graph.neighbors(v).len();
            }
            black_box(total)
        })
    });

    group.finish();
}

// ============================================================================
// Memory and Cache Benchmarks
// ============================================================================

/// Benchmark memory access patterns
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_patterns");

    // Sequential vertex access
    group.bench_function("sequential_vertex_scan", |b| {
        let mut graph = CompactGraph::new();
        for i in 0..200u16 {
            graph.add_edge(i, i + 1, 100);
        }

        b.iter(|| {
            let mut active = 0u16;
            for i in 0..256u16 {
                if graph.vertices[i as usize].is_active() {
                    active += 1;
                }
            }
            black_box(active)
        })
    });

    // Random access pattern
    group.bench_function("random_vertex_access", |b| {
        let mut graph = CompactGraph::new();
        for i in 0..200u16 {
            graph.add_edge(i, i + 1, 100);
        }

        // Pseudo-random access pattern
        let indices: Vec<u16> = (0..100).map(|i| (i * 37) % 256).collect();

        b.iter(|| {
            let mut sum = 0u8;
            for &i in &indices {
                sum = sum.wrapping_add(graph.vertices[i as usize].degree);
            }
            black_box(sum)
        })
    });

    // Edge array scan
    group.bench_function("edge_array_scan", |b| {
        let mut graph = CompactGraph::new();
        for i in 0..500u16 {
            let src = i % 200;
            let dst = (i % 200) + 1;
            if src != dst {
                graph.add_edge(src, dst, 100);
            }
        }

        b.iter(|| {
            let mut active = 0u16;
            for edge in &graph.edges {
                if edge.is_active() {
                    active += 1;
                }
            }
            black_box(active)
        })
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(edge_benches, bench_edge_insert, bench_edge_batch,);

criterion_group!(tick_benches, bench_tick, bench_tick_under_load,);

criterion_group!(evidence_benches, bench_evalue_update, bench_mixture_evalue,);

criterion_group!(
    misc_benches,
    bench_report_serialize,
    bench_delta_ingestion,
    bench_neighbor_iteration,
    bench_memory_patterns,
);

criterion_main!(edge_benches, tick_benches, evidence_benches, misc_benches);
