//! Performance benchmark for canonical min-cut.
//! Run with: cargo test -p ruvector-mincut --features canonical --test canonical_bench --release -- --nocapture

#[cfg(feature = "canonical")]
mod bench {
    use ruvector_mincut::canonical::CactusGraph;
    use ruvector_mincut::graph::DynamicGraph;
    use std::time::Instant;

    /// Benchmark at 30 vertices (typical subgraph partition size).
    /// The CactusGraph uses Stoer-Wagner (O(n^3)), so performance scales
    /// cubically. For WASM tiles (<=256 vertices), the ArenaCactus path
    /// is used instead (measured at ~3µs in the gate-kernel benchmark).
    #[test]
    fn bench_canonical_mincut_30v() {
        let mut graph = DynamicGraph::new();
        for i in 0..30u64 {
            graph.add_vertex(i);
        }
        // Ring + cross edges (~90 edges)
        for i in 0..30u64 {
            let _ = graph.insert_edge(i, (i + 1) % 30, 1.0);
        }
        for i in 0..30u64 {
            let _ = graph.insert_edge(i, (i + 11) % 30, 0.5);
            let _ = graph.insert_edge(i, (i + 19) % 30, 0.3);
        }

        // Warm up
        let _ = CactusGraph::build_from_graph(&graph);

        // Benchmark cactus construction
        let n_iter = 100;
        let start = Instant::now();
        for _ in 0..n_iter {
            let cactus = CactusGraph::build_from_graph(&graph);
            std::hint::black_box(&cactus);
        }
        let avg_cactus_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        // Benchmark canonical cut extraction
        let cactus = CactusGraph::build_from_graph(&graph);
        let start = Instant::now();
        for _ in 0..n_iter {
            let result = cactus.canonical_cut();
            std::hint::black_box(&result);
        }
        let avg_cut_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        // Determinism: all 100 produce identical result
        let reference = cactus.canonical_cut();
        for _ in 0..100 {
            let result = cactus.canonical_cut();
            assert_eq!(result.value, reference.value);
            assert_eq!(result.canonical_key, reference.canonical_key);
        }

        let total = avg_cactus_us + avg_cut_us;
        println!("\n=== Canonical Min-Cut (30v, ~90e) ===");
        println!("  CactusGraph build:   {:.1} µs", avg_cactus_us);
        println!("  Canonical cut:       {:.1} µs", avg_cut_us);
        println!(
            "  Total:               {:.1} µs  (target: < 3000 µs native)",
            total
        );
        println!("  Cut value:           {}", reference.value);
        println!("  NOTE: WASM ArenaCactus (64v) = ~3µs (see gate-kernel bench)");

        // Native CactusGraph uses heap-allocated Stoer-Wagner (O(n^3));
        // the WASM ArenaCactus path (stack-allocated) is 500x faster.
        assert!(
            total < 3000.0,
            "Exceeded 3ms native target: {:.1} µs",
            total
        );
    }

    /// Also benchmark at 100 vertices to track scalability (informational, no assertion).
    #[test]
    fn bench_canonical_mincut_100v_info() {
        let mut graph = DynamicGraph::new();
        for i in 0..100u64 {
            graph.add_vertex(i);
        }
        for i in 0..100u64 {
            let _ = graph.insert_edge(i, (i + 1) % 100, 1.0);
        }
        for i in 0..100u64 {
            let _ = graph.insert_edge(i, (i + 37) % 100, 0.5);
            let _ = graph.insert_edge(i, (i + 73) % 100, 0.3);
        }

        let _ = CactusGraph::build_from_graph(&graph);
        let n_iter = 20;
        let start = Instant::now();
        for _ in 0..n_iter {
            let cactus = CactusGraph::build_from_graph(&graph);
            let _ = cactus.canonical_cut();
            std::hint::black_box(&cactus);
        }
        let avg_total_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        println!("\n=== Canonical Min-Cut Scalability (100v, ~300e) ===");
        println!(
            "  Total (build+cut):   {:.1} µs  (informational)",
            avg_total_us
        );
        println!("  Stoer-Wagner is O(n^3), scales cubically with graph size");
    }
}
