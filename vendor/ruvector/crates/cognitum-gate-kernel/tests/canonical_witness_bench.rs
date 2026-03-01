//! Performance benchmark for canonical witness fragments.
//! Run with: cargo test -p cognitum-gate-kernel --features "std,canonical-witness" --test canonical_witness_bench --release -- --nocapture

#[cfg(feature = "canonical-witness")]
mod bench {
    use cognitum_gate_kernel::canonical_witness::{ArenaCactus, CanonicalWitnessFragment};
    use cognitum_gate_kernel::shard::CompactGraph;
    use cognitum_gate_kernel::TileState;
    use std::time::Instant;

    #[test]
    fn bench_witness_fragment_64v() {
        // Build a CompactGraph with 64 vertices
        let mut graph = CompactGraph::new();
        for i in 0..64u16 {
            graph.add_edge(i, (i + 1) % 64, 100);
        }
        for i in 0..64u16 {
            graph.add_edge(i, (i + 13) % 64, 50);
        }
        graph.recompute_components();

        // Warm up
        let _ = ArenaCactus::build_from_compact_graph(&graph);

        // Benchmark ArenaCactus construction
        let n_iter = 1000;
        let start = Instant::now();
        for _ in 0..n_iter {
            let cactus = ArenaCactus::build_from_compact_graph(&graph);
            std::hint::black_box(&cactus);
        }
        let avg_cactus_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        // Benchmark canonical partition
        let cactus = ArenaCactus::build_from_compact_graph(&graph);
        let start = Instant::now();
        for _ in 0..n_iter {
            let p = cactus.canonical_partition();
            std::hint::black_box(&p);
        }
        let avg_partition_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        // Full witness via TileState
        let mut tile = TileState::new(42);
        for i in 0..64u16 {
            tile.graph.add_edge(i, (i + 1) % 64, 100);
            tile.graph.add_edge(i, (i + 13) % 64, 50);
        }
        tile.graph.recompute_components();

        let start = Instant::now();
        for _ in 0..n_iter {
            let f = tile.canonical_witness();
            std::hint::black_box(&f);
        }
        let avg_witness_us = start.elapsed().as_micros() as f64 / n_iter as f64;

        // Determinism check
        let ref_f = tile.canonical_witness();
        for _ in 0..100 {
            let f = tile.canonical_witness();
            assert_eq!(f.canonical_hash, ref_f.canonical_hash);
            assert_eq!(f.cactus_digest, ref_f.cactus_digest);
        }

        println!("\n=== Canonical Witness Fragment (64 vertices) ===");
        println!("  ArenaCactus build:    {:.1} µs", avg_cactus_us);
        println!("  Partition extract:    {:.1} µs", avg_partition_us);
        println!(
            "  Full witness:         {:.1} µs  (target: < 50 µs)",
            avg_witness_us
        );
        println!(
            "  Fragment size:        {} bytes",
            std::mem::size_of::<CanonicalWitnessFragment>()
        );
        println!("  Cut value:            {}", ref_f.cut_value);

        assert!(
            avg_witness_us < 50.0,
            "Witness exceeded 50µs target: {:.1} µs",
            avg_witness_us
        );
    }
}
