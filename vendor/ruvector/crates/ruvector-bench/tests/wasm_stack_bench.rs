//! Performance benchmarks for the WASM cognitive stack.
//!
//! Measures key operations against target latencies from the research:
//! - Container tick:         < 200 us native
//! - SCS full recompute:     < 5 ms (500 vertices)
//! - Canonical min-cut:      < 1 ms (100 vertices)
//! - Witness fragment:       < 50 us (64 vertices)
//!
//! Run with:
//!   cargo test --test wasm_stack_bench --release -- --nocapture

use std::time::Instant;

// =========================================================================
// (a) Canonical min-cut benchmark (ruvector-mincut, feature = "canonical")
// =========================================================================

#[test]
fn bench_canonical_mincut_100v() {
    use ruvector_mincut::canonical::CactusGraph;
    use ruvector_mincut::graph::DynamicGraph;

    let graph = DynamicGraph::new();

    // Build a graph with 100 vertices and ~300 edges
    for i in 0..100u64 {
        graph.add_vertex(i);
    }
    // Ring edges (100)
    for i in 0..100u64 {
        let _ = graph.insert_edge(i, (i + 1) % 100, 1.0);
    }
    // Cross edges for richer structure (~200 more)
    for i in 0..100u64 {
        let _ = graph.insert_edge(i, (i + 37) % 100, 0.5);
        let _ = graph.insert_edge(i, (i + 73) % 100, 0.3);
    }

    // Warm up
    let _ = CactusGraph::build_from_graph(&graph);

    // --- CactusGraph construction (100 iterations) ---
    let n_iter = 100;
    let start = Instant::now();
    for _ in 0..n_iter {
        let mut cactus = CactusGraph::build_from_graph(&graph);
        cactus.root_at_lex_smallest();
        std::hint::black_box(&cactus);
    }
    let cactus_time = start.elapsed();
    let avg_cactus_us = cactus_time.as_micros() as f64 / n_iter as f64;

    // --- Canonical cut extraction (100 iterations) ---
    let mut cactus = CactusGraph::build_from_graph(&graph);
    cactus.root_at_lex_smallest();
    println!(
        "  Cactus: {} vertices, {} edges, {} cycles",
        cactus.n_vertices,
        cactus.n_edges,
        cactus.cycles.len()
    );
    let start = Instant::now();
    for _ in 0..n_iter {
        let result = cactus.canonical_cut();
        std::hint::black_box(&result);
    }
    let cut_time = start.elapsed();
    let avg_cut_us = cut_time.as_micros() as f64 / n_iter as f64;

    // --- Determinism verification: 100 iterations produce the same result ---
    let reference = cactus.canonical_cut();
    let start = Instant::now();
    for _ in 0..100 {
        let mut c = CactusGraph::build_from_graph(&graph);
        c.root_at_lex_smallest();
        let result = c.canonical_cut();
        assert_eq!(
            result.canonical_key, reference.canonical_key,
            "Determinism violation in canonical min-cut!"
        );
    }
    let determinism_us = start.elapsed().as_micros();

    let total_us = avg_cactus_us + avg_cut_us;
    let status = if total_us < 1000.0 { "PASS" } else { "FAIL" };

    println!("\n=== (a) Canonical Min-Cut (100 vertices, ~300 edges) ===");
    println!(
        "  CactusGraph construction:  {:.1} us  (avg of {} iters)",
        avg_cactus_us, n_iter
    );
    println!(
        "  Canonical cut extraction:  {:.1} us  (avg of {} iters)",
        avg_cut_us, n_iter
    );
    println!(
        "  Total (construct + cut):   {:.1} us  [target < 1000 us] [{}]",
        total_us, status
    );
    println!("  Determinism (100x verify): {} us total", determinism_us);
    println!("  Min-cut value:             {:.4}", reference.value);
    println!("  Cut edges:                 {}", reference.cut_edges.len());
    println!(
        "  Partition sizes:           {} / {}",
        reference.partition.0.len(),
        reference.partition.1.len()
    );
}

// =========================================================================
// (b) Spectral Coherence Score benchmark (ruvector-coherence)
// =========================================================================

#[test]
fn bench_spectral_coherence_500v() {
    use ruvector_coherence::spectral::{CsrMatrixView, SpectralConfig, SpectralTracker};

    let n = 500;

    // Build a 500-node graph: ring + deterministic cross-edges (~1500 edges)
    let mut edges: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..n {
        edges.push((i, (i + 1) % n, 1.0));
    }
    for i in 0..n {
        edges.push((i, (i + 37) % n, 0.5));
        edges.push((i, (i + 127) % n, 0.3));
    }

    let lap = CsrMatrixView::build_laplacian(n, &edges);
    let config = SpectralConfig::default();

    // Warm up
    let mut tracker = SpectralTracker::new(config.clone());
    let _ = tracker.compute(&lap);

    // --- Full SCS recompute ---
    let n_iter = 20;
    let start = Instant::now();
    for _ in 0..n_iter {
        let mut t = SpectralTracker::new(config.clone());
        let score = t.compute(&lap);
        std::hint::black_box(&score);
    }
    let full_time = start.elapsed();
    let avg_full_us = full_time.as_micros() as f64 / n_iter as f64;
    let avg_full_ms = avg_full_us / 1000.0;

    // Capture one result for reporting
    let mut report_tracker = SpectralTracker::new(config.clone());
    let initial_score = report_tracker.compute(&lap);

    // --- Incremental update (single edge change) ---
    let n_incr = 100;
    let start = Instant::now();
    for i in 0..n_incr {
        report_tracker.update_edge(&lap, i % n, (i + 1) % n, 0.01);
    }
    let incr_time = start.elapsed();
    let avg_incr_us = incr_time.as_micros() as f64 / n_incr as f64;

    let status = if avg_full_ms < 5.0 { "PASS" } else { "FAIL" };

    println!("\n=== (b) Spectral Coherence Score (500 vertices, ~1500 edges) ===");
    println!(
        "  Full SCS recompute:        {:.2} ms  (avg of {} iters) [target < 5 ms] [{}]",
        avg_full_ms, n_iter, status
    );
    println!(
        "  Incremental update:        {:.1} us  (avg of {} iters)",
        avg_incr_us, n_incr
    );
    println!(
        "  Initial composite SCS:     {:.6}",
        initial_score.composite
    );
    println!("  Fiedler:                   {:.6}", initial_score.fiedler);
    println!(
        "  Spectral gap:              {:.6}",
        initial_score.spectral_gap
    );
    println!(
        "  Effective resistance:       {:.6}",
        initial_score.effective_resistance
    );
    println!(
        "  Degree regularity:         {:.6}",
        initial_score.degree_regularity
    );
}

// =========================================================================
// (c) Cognitive Container benchmark
// =========================================================================

#[test]
fn bench_cognitive_container_100_ticks() {
    use ruvector_cognitive_container::{
        CognitiveContainer, ContainerConfig, Delta, VerificationResult,
    };

    let config = ContainerConfig::default();
    let mut container = CognitiveContainer::new(config).expect("Failed to create container");

    // Build a base graph of 50 edges
    let init_deltas: Vec<Delta> = (0..50)
        .map(|i| Delta::EdgeAdd {
            u: i,
            v: (i + 1) % 50,
            weight: 1.0,
        })
        .collect();
    let _ = container.tick(&init_deltas);

    // --- 100 ticks with incremental updates ---
    let n_ticks = 100;
    let mut tick_times = Vec::with_capacity(n_ticks);

    let outer_start = Instant::now();
    for i in 0..n_ticks {
        let deltas = vec![
            Delta::EdgeAdd {
                u: i % 50,
                v: (i + 17) % 50,
                weight: 0.5 + (i as f64 * 0.01),
            },
            Delta::Observation {
                node: i % 50,
                value: 0.7 + (i as f64 * 0.001),
            },
        ];
        let t0 = Instant::now();
        let result = container.tick(&deltas).expect("Tick failed");
        let elapsed = t0.elapsed().as_micros() as u64;
        tick_times.push(elapsed);
    }
    let outer_elapsed = outer_start.elapsed();

    let avg_tick_us = tick_times.iter().sum::<u64>() as f64 / tick_times.len() as f64;
    let max_tick_us = *tick_times.iter().max().unwrap();
    let min_tick_us = *tick_times.iter().min().unwrap();
    let mut sorted_ticks = tick_times.clone();
    sorted_ticks.sort();
    let p50 = sorted_ticks[sorted_ticks.len() / 2];
    let p99 = sorted_ticks[(sorted_ticks.len() as f64 * 0.99) as usize];

    // --- Witness chain verification ---
    let verify_start = Instant::now();
    let verification = container.verify_chain();
    let verify_us = verify_start.elapsed().as_micros();

    let status = if avg_tick_us < 200.0 { "PASS" } else { "FAIL" };

    println!("\n=== (c) Cognitive Container (100 ticks, 2 deltas each) ===");
    println!(
        "  Average tick:              {:.1} us  [target < 200 us] [{}]",
        avg_tick_us, status
    );
    println!("  Median tick (p50):         {} us", p50);
    println!("  p99 tick:                  {} us", p99);
    println!(
        "  Min / Max tick:            {} / {} us",
        min_tick_us, max_tick_us
    );
    println!(
        "  Total (100 ticks):         {:.2} ms",
        outer_elapsed.as_micros() as f64 / 1000.0
    );
    println!(
        "  Chain verification:        {} us  (chain len = {})",
        verify_us,
        container.current_epoch()
    );
    println!(
        "  Chain valid:               {}",
        matches!(verification, VerificationResult::Valid { .. })
    );
}

// =========================================================================
// (d) Canonical witness / gate-kernel benchmark
// =========================================================================

#[test]
fn bench_canonical_witness_64v() {
    use cognitum_gate_kernel::canonical_witness::{ArenaCactus, CanonicalWitnessFragment};
    use cognitum_gate_kernel::shard::CompactGraph;
    use cognitum_gate_kernel::TileState;

    // Build a CompactGraph with 64 vertices and ~128 edges
    let build_graph = || {
        let mut g = CompactGraph::new();
        // Ring
        for i in 0..64u16 {
            g.add_edge(i, (i + 1) % 64, 100);
        }
        // Cross edges
        for i in 0..64u16 {
            g.add_edge(i, (i + 13) % 64, 50);
        }
        g.recompute_components();
        g
    };

    let graph = build_graph();

    // Warm up
    let _ = ArenaCactus::build_from_compact_graph(&graph);

    // --- ArenaCactus construction (1000 iterations) ---
    let n_iter = 1000;
    let start = Instant::now();
    for _ in 0..n_iter {
        let cactus = ArenaCactus::build_from_compact_graph(&graph);
        std::hint::black_box(&cactus);
    }
    let cactus_time = start.elapsed();
    let avg_cactus_us = cactus_time.as_micros() as f64 / n_iter as f64;

    // --- Canonical partition extraction (1000 iterations) ---
    let cactus = ArenaCactus::build_from_compact_graph(&graph);
    let start = Instant::now();
    for _ in 0..n_iter {
        let partition = cactus.canonical_partition();
        std::hint::black_box(&partition);
    }
    let partition_time = start.elapsed();
    let avg_partition_us = partition_time.as_micros() as f64 / n_iter as f64;

    // --- Full witness fragment via TileState (1000 iterations) ---
    let mut tile = TileState::new(42);
    for i in 0..64u16 {
        tile.graph.add_edge(i, (i + 1) % 64, 100);
        tile.graph.add_edge(i, (i + 13) % 64, 50);
    }
    tile.graph.recompute_components();

    let start = Instant::now();
    for _ in 0..n_iter {
        let fragment = tile.canonical_witness();
        std::hint::black_box(&fragment);
    }
    let witness_time = start.elapsed();
    let avg_witness_us = witness_time.as_micros() as f64 / n_iter as f64;

    // --- Determinism verification ---
    let ref_fragment = tile.canonical_witness();
    let det_start = Instant::now();
    for _ in 0..100 {
        let g = build_graph();
        let c = ArenaCactus::build_from_compact_graph(&g);
        let p = c.canonical_partition();
        assert_eq!(
            p.canonical_hash,
            {
                let c2 = ArenaCactus::build_from_compact_graph(&graph);
                c2.canonical_partition().canonical_hash
            },
            "Gate-kernel determinism violation!"
        );
    }
    let det_us = det_start.elapsed().as_micros();

    let total_us = avg_cactus_us + avg_partition_us;
    let status = if avg_witness_us < 50.0 {
        "PASS"
    } else {
        "FAIL"
    };

    println!("\n=== (d) Canonical Witness Fragment (64 vertices, ~128 edges) ===");
    println!(
        "  ArenaCactus construction:  {:.2} us  (avg of {} iters)",
        avg_cactus_us, n_iter
    );
    println!(
        "  Partition extraction:      {:.2} us  (avg of {} iters)",
        avg_partition_us, n_iter
    );
    println!(
        "  Full witness fragment:     {:.2} us  [target < 50 us] [{}]",
        avg_witness_us, status
    );
    println!(
        "  Fragment size:             {} bytes",
        std::mem::size_of::<CanonicalWitnessFragment>()
    );
    println!("  Cactus nodes:              {}", cactus.n_nodes);
    println!("  Cut value:                 {}", ref_fragment.cut_value);
    println!(
        "  Cardinality A/B:           {} / {}",
        ref_fragment.cardinality_a, ref_fragment.cardinality_b
    );
    println!("  Determinism (100x):        {} us", det_us);
}

// =========================================================================
// Summary report
// =========================================================================

#[test]
fn bench_z_summary() {
    println!("\n");
    println!("================================================================");
    println!("      WASM Cognitive Stack -- Benchmark Targets                ");
    println!("================================================================");
    println!("  Component                     Target");
    println!("  ----------------------------  ----------");
    println!("  (a) Canonical min-cut (100v)  < 1 ms");
    println!("  (b) SCS full recompute (500v) < 5 ms");
    println!("  (c) Container tick            < 200 us");
    println!("  (d) Witness fragment (64v)    < 50 us");
    println!("================================================================");
    println!("  Run: cargo test --test wasm_stack_bench --release -- --nocapture");
    println!("================================================================");
}
