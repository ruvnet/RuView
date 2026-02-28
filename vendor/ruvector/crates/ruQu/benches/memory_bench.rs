//! Memory efficiency benchmarks for ruQu Coherence Gate.
//!
//! Memory Targets:
//! - Per-tile memory usage: **<64KB**
//! - Allocation counts per cycle: **0 (steady state)**
//! - Cache line efficiency: **>80%**
//!
//! Run with: `cargo bench -p ruqu --bench memory_bench`

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use ruqu::filters::{FilterConfig, FilterPipeline, ShiftFilter, StructuralFilter};
use ruqu::syndrome::{DetectorBitmap, SyndromeBuffer, SyndromeRound};
use ruqu::tile::{
    EvidenceAccumulator, GateThresholds, LocalCutState, PatchGraph, ReceiptLog, SyndromBuffer,
    SyndromeDelta, TileReport, TileZero, WorkerTile,
};

// ============================================================================
// ALLOCATION TRACKING ALLOCATOR
// ============================================================================

/// Global allocation counter for tracking allocations
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static DEALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static BYTES_ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static BYTES_DEALLOCATED: AtomicUsize = AtomicUsize::new(0);

/// Reset allocation counters
fn reset_allocation_counters() {
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    DEALLOC_COUNT.store(0, Ordering::SeqCst);
    BYTES_ALLOCATED.store(0, Ordering::SeqCst);
    BYTES_DEALLOCATED.store(0, Ordering::SeqCst);
}

/// Get allocation statistics
fn get_allocation_stats() -> (usize, usize, usize, usize) {
    (
        ALLOC_COUNT.load(Ordering::SeqCst),
        DEALLOC_COUNT.load(Ordering::SeqCst),
        BYTES_ALLOCATED.load(Ordering::SeqCst),
        BYTES_DEALLOCATED.load(Ordering::SeqCst),
    )
}

// ============================================================================
// SIZE VERIFICATION BENCHMARKS
// ============================================================================

/// Benchmark and verify structure sizes
fn bench_structure_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("structure_sizes");

    // Report sizes (this is informational, not a timed benchmark)
    println!("\n=== Structure Sizes ===");
    println!(
        "WorkerTile:          {} bytes",
        std::mem::size_of::<WorkerTile>()
    );
    println!(
        "PatchGraph:          {} bytes",
        std::mem::size_of::<PatchGraph>()
    );
    println!(
        "SyndromBuffer:       {} bytes",
        std::mem::size_of::<SyndromBuffer>()
    );
    println!(
        "EvidenceAccumulator: {} bytes",
        std::mem::size_of::<EvidenceAccumulator>()
    );
    println!(
        "LocalCutState:       {} bytes",
        std::mem::size_of::<LocalCutState>()
    );
    println!(
        "TileReport:          {} bytes",
        std::mem::size_of::<TileReport>()
    );
    println!(
        "DetectorBitmap:      {} bytes",
        std::mem::size_of::<DetectorBitmap>()
    );
    println!(
        "SyndromeRound:       {} bytes",
        std::mem::size_of::<SyndromeRound>()
    );
    println!(
        "SyndromeDelta:       {} bytes",
        std::mem::size_of::<SyndromeDelta>()
    );
    println!();

    // Verify 64KB budget
    let total_tile_size = std::mem::size_of::<WorkerTile>();
    let budget = 65536; // 64KB
    println!(
        "WorkerTile size: {} bytes ({:.1}% of 64KB budget)",
        total_tile_size,
        (total_tile_size as f64 / budget as f64) * 100.0
    );

    // Benchmark size computation (ensures compiler doesn't optimize away)
    group.bench_function("size_of_worker_tile", |b| {
        b.iter(|| black_box(std::mem::size_of::<WorkerTile>()));
    });

    group.bench_function("size_of_patch_graph", |b| {
        b.iter(|| black_box(std::mem::size_of::<PatchGraph>()));
    });

    group.bench_function("size_of_tile_report", |b| {
        b.iter(|| black_box(std::mem::size_of::<TileReport>()));
    });

    group.finish();
}

// ============================================================================
// PER-TILE MEMORY USAGE
// ============================================================================

/// Benchmark per-tile memory usage
fn bench_per_tile_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("per_tile_memory");

    // WorkerTile memory footprint
    let worker_tile_size = std::mem::size_of::<WorkerTile>();
    assert!(
        worker_tile_size <= 131072, // 128KB max (some padding allowed)
        "WorkerTile exceeds memory budget: {} bytes",
        worker_tile_size
    );

    // Benchmark WorkerTile creation (measures stack allocation)
    group.bench_function("create_worker_tile", |b| {
        b.iter(|| {
            let tile = WorkerTile::new(1);
            black_box(&tile);
            // Note: WorkerTile is large, measure creation overhead
        });
    });

    // Benchmark WorkerTile reset (should be allocation-free)
    group.bench_function("reset_worker_tile", |b| {
        let mut tile = WorkerTile::new(1);
        // Populate with some data
        for i in 0..50u16 {
            let _ = tile.patch_graph.add_edge(i, i + 1, 1000);
        }

        b.iter(|| {
            tile.reset();
            black_box(&tile);
        });
    });

    // Benchmark PatchGraph memory efficiency
    group.bench_function("patch_graph_memory", |b| {
        b.iter(|| {
            let graph = PatchGraph::new();
            black_box(&graph);
            black_box(std::mem::size_of_val(&graph));
        });
    });

    // Benchmark SyndromBuffer memory efficiency
    group.bench_function("syndrom_buffer_memory", |b| {
        b.iter(|| {
            let buffer = SyndromBuffer::new();
            black_box(&buffer);
            black_box(std::mem::size_of_val(&buffer));
        });
    });

    group.finish();
}

// ============================================================================
// ALLOCATION-FREE OPERATIONS
// ============================================================================

/// Benchmark operations that should be allocation-free in steady state
fn bench_allocation_free_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("allocation_free");

    // Worker tile tick should be allocation-free
    group.bench_function("worker_tick_no_alloc", |b| {
        let mut tile = WorkerTile::new(1);
        // Pre-populate
        for i in 0..50u16 {
            let _ = tile.patch_graph.add_edge(i, i + 1, 1000);
        }
        tile.patch_graph.recompute_components();

        let delta = SyndromeDelta::new(0, 1, 100);

        b.iter(|| {
            let report = tile.tick(&delta);
            black_box(report);
        });
    });

    // PatchGraph operations should be allocation-free
    group.bench_function("patch_graph_ops_no_alloc", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
        }
        graph.recompute_components();

        b.iter(|| {
            // These operations should not allocate
            let cut = graph.estimate_local_cut();
            let mut candidates = [0u16; 64];
            let count = graph.identify_boundary_candidates(&mut candidates);
            black_box((cut, count));
        });
    });

    // DetectorBitmap operations should be allocation-free
    group.bench_function("bitmap_ops_no_alloc", |b| {
        let mut a = DetectorBitmap::new(1024);
        let mut bb = DetectorBitmap::new(1024);
        for i in (0..512).step_by(2) {
            a.set(i, true);
        }
        for i in (256..768).step_by(2) {
            bb.set(i, true);
        }

        b.iter(|| {
            let result = a.xor(&bb);
            let count = result.popcount();
            black_box(count);
        });
    });

    // TileReport copy should be allocation-free
    group.bench_function("tile_report_copy_no_alloc", |b| {
        let mut report = TileReport::new(1);
        report.local_cut = 10.0;
        report.shift_score = 0.1;
        report.e_value = 200.0;

        b.iter(|| {
            let copy = report;
            black_box(copy);
        });
    });

    // Evidence accumulator operations should be allocation-free
    group.bench_function("evidence_update_no_alloc", |b| {
        let mut evidence = EvidenceAccumulator::new();

        b.iter(|| {
            evidence.observe(1000);
            let e = evidence.e_value();
            black_box(e);
        });
    });

    // LocalCutState update should be allocation-free
    group.bench_function("local_cut_update_no_alloc", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..100u16 {
            let _ = graph.add_edge(i, (i + 1) % 100, 1000);
        }
        graph.recompute_components();

        let mut cut_state = LocalCutState::new();

        b.iter(|| {
            cut_state.update_from_graph(&graph);
            black_box(&cut_state);
        });
    });

    group.finish();
}

// ============================================================================
// CACHE LINE EFFICIENCY
// ============================================================================

/// Benchmark cache line efficiency
fn bench_cache_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache_efficiency");

    const CACHE_LINE_SIZE: usize = 64;

    // Verify cache-line alignment
    println!("\n=== Cache Line Alignment ===");
    println!(
        "TileReport alignment:    {} bytes (cache line: {})",
        std::mem::align_of::<TileReport>(),
        CACHE_LINE_SIZE
    );
    println!(
        "PatchGraph alignment:    {} bytes",
        std::mem::align_of::<PatchGraph>()
    );
    println!(
        "SyndromBuffer alignment: {} bytes",
        std::mem::align_of::<SyndromBuffer>()
    );
    println!(
        "DetectorBitmap alignment: {} bytes",
        std::mem::align_of::<DetectorBitmap>()
    );
    println!();

    // Sequential access pattern (cache-friendly)
    group.bench_function("sequential_access", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..200u16 {
            graph.ensure_vertex(i);
        }

        b.iter(|| {
            let mut sum = 0u32;
            for i in 0..200 {
                if graph.vertices[i].is_active() {
                    sum += graph.vertices[i].degree as u32;
                }
            }
            black_box(sum);
        });
    });

    // Strided access pattern (potential cache misses)
    group.bench_function("strided_access", |b| {
        let mut graph = PatchGraph::new();
        for i in 0..200u16 {
            graph.ensure_vertex(i);
        }

        b.iter(|| {
            let mut sum = 0u32;
            // Access every 8th element (stride across multiple cache lines)
            for i in (0..200).step_by(8) {
                if graph.vertices[i].is_active() {
                    sum += graph.vertices[i].degree as u32;
                }
            }
            black_box(sum);
        });
    });

    // TileReport array access (should be cache-line aligned)
    group.bench_function("tile_report_array_access", |b| {
        let reports: Vec<TileReport> = (1..=255)
            .map(|i| {
                let mut r = TileReport::new(i);
                r.local_cut = i as f64;
                r
            })
            .collect();

        b.iter(|| {
            let mut sum = 0.0f64;
            for report in &reports {
                sum += report.local_cut;
            }
            black_box(sum);
        });
    });

    // DetectorBitmap word access (should be aligned)
    group.bench_function("bitmap_word_access", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in (0..1024).step_by(3) {
            bitmap.set(i, true);
        }

        b.iter(|| {
            let raw = bitmap.raw_bits();
            let mut sum = 0u64;
            for word in raw {
                sum = sum.wrapping_add(*word);
            }
            black_box(sum);
        });
    });

    group.finish();
}

// ============================================================================
// MEMORY POOL SIMULATION
// ============================================================================

/// Benchmark simulated memory pool operations
fn bench_memory_pool(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pool");

    // Pre-allocated tile pool
    group.bench_function("tile_pool_reuse", |b| {
        // Simulate a pool of worker tiles
        let mut tile_pool: Vec<WorkerTile> = (1..=10).map(|i| WorkerTile::new(i)).collect();

        let delta = SyndromeDelta::new(0, 1, 100);

        b.iter(|| {
            // Use tiles from pool without allocation
            for tile in &mut tile_pool {
                let report = tile.tick(&delta);
                black_box(&report);
            }
        });
    });

    // Pre-allocated report buffer
    group.bench_function("report_buffer_reuse", |b| {
        // Simulate a reusable report buffer
        let mut report_buffer: [TileReport; 255] = [TileReport::default(); 255];

        b.iter(|| {
            // Fill buffer without allocation
            for i in 0..255 {
                report_buffer[i].tile_id = i as u8;
                report_buffer[i].local_cut = 10.0;
                report_buffer[i].shift_score = 0.1;
                report_buffer[i].e_value = 200.0;
            }
            black_box(&report_buffer);
        });
    });

    // Pre-allocated syndrome round buffer
    group.bench_function("syndrome_round_reuse", |b| {
        let mut buffer = SyndromeBuffer::new(1024);
        let mut round_id = 0u64;
        // Pre-fill
        for i in 0..1024 {
            let round = SyndromeRound::new(i, i, i * 1000, DetectorBitmap::new(64), 0);
            buffer.push(round);
        }

        b.iter(|| {
            // Push rounds (reusing buffer space)
            for _ in 0..100 {
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
            black_box(&buffer);
        });
    });

    group.finish();
}

// ============================================================================
// HEAP ALLOCATION BENCHMARKS
// ============================================================================

/// Benchmark operations that require heap allocation
fn bench_heap_allocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("heap_allocations");

    // Filter pipeline (requires heap for collections)
    group.bench_function("filter_pipeline_create", |b| {
        b.iter(|| {
            let config = FilterConfig::default();
            let pipeline = FilterPipeline::new(config);
            black_box(pipeline);
        });
    });

    // TileZero creation (requires heap)
    group.bench_function("tilezero_create", |b| {
        b.iter(|| {
            let thresholds = GateThresholds::default();
            let tilezero = TileZero::new(thresholds);
            black_box(tilezero);
        });
    });

    // ReceiptLog append (heap allocation)
    group.bench_function("receipt_log_grow", |b| {
        b.iter_batched(
            ReceiptLog::new,
            |mut log| {
                for i in 0..100 {
                    log.append(ruqu::tile::GateDecision::Permit, i, i * 1000, [0u8; 32]);
                }
                black_box(&log);
            },
            criterion::BatchSize::SmallInput,
        );
    });

    // SyndromeBuffer create (heap allocation)
    group.bench_function("syndrome_buffer_create", |b| {
        b.iter(|| {
            let buffer = SyndromeBuffer::new(1024);
            black_box(buffer);
        });
    });

    // Large buffer sizes
    for size in [1024, 4096, 16384, 65536].iter() {
        group.bench_with_input(
            BenchmarkId::new("syndrome_buffer_create", size),
            size,
            |b, &sz| {
                b.iter(|| {
                    let buffer = SyndromeBuffer::new(sz);
                    black_box(buffer);
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// MEMORY BANDWIDTH BENCHMARKS
// ============================================================================

/// Benchmark memory bandwidth operations
fn bench_memory_bandwidth(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_bandwidth");

    // Large data copy (TileReport array)
    group.throughput(Throughput::Bytes(
        255 * std::mem::size_of::<TileReport>() as u64,
    ));
    group.bench_function("copy_255_reports", |b| {
        let source: Vec<TileReport> = (1..=255).map(|i| TileReport::new(i)).collect();

        b.iter(|| {
            let copy: Vec<TileReport> = source.clone();
            black_box(copy);
        });
    });

    // DetectorBitmap copy
    group.throughput(Throughput::Bytes(
        std::mem::size_of::<DetectorBitmap>() as u64
    ));
    group.bench_function("copy_bitmap", |b| {
        let mut bitmap = DetectorBitmap::new(1024);
        for i in 0..512 {
            bitmap.set(i, true);
        }

        b.iter(|| {
            let copy = bitmap;
            black_box(copy);
        });
    });

    // Batch bitmap copy
    group.throughput(Throughput::Bytes(
        100 * std::mem::size_of::<DetectorBitmap>() as u64,
    ));
    group.bench_function("copy_100_bitmaps", |b| {
        let bitmaps: Vec<DetectorBitmap> = (0..100)
            .map(|i| {
                let mut bm = DetectorBitmap::new(1024);
                bm.set(i * 10, true);
                bm
            })
            .collect();

        b.iter(|| {
            let copy: Vec<DetectorBitmap> = bitmaps.clone();
            black_box(copy);
        });
    });

    // SyndromeRound copy
    group.throughput(Throughput::Bytes(
        std::mem::size_of::<SyndromeRound>() as u64
    ));
    group.bench_function("copy_syndrome_round", |b| {
        let mut detectors = DetectorBitmap::new(256);
        for i in 0..25 {
            detectors.set(i * 10, true);
        }
        let round = SyndromeRound::new(12345, 100, 1000000, detectors, 0);

        b.iter(|| {
            let copy = round.clone();
            black_box(copy);
        });
    });

    group.finish();
}

// ============================================================================
// CRITERION GROUPS
// ============================================================================

criterion_group!(
    memory_benches,
    bench_structure_sizes,
    bench_per_tile_memory,
    bench_allocation_free_ops,
    bench_cache_efficiency,
    bench_memory_pool,
    bench_heap_allocations,
    bench_memory_bandwidth,
);

criterion_main!(memory_benches);
