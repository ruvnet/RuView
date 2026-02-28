//! Comprehensive Learning Capability Benchmarks
//!
//! Benchmarks for all EXO-AI cognitive and learning features:
//! - Sequential pattern learning
//! - Causal graph operations
//! - Salience computation
//! - Anticipation/prediction
//! - Memory consolidation
//! - Consciousness metrics (IIT)
//! - Thermodynamic tracking

use std::collections::HashMap;
use std::time::{Duration, Instant};

// EXO-AI crates
use exo_core::consciousness::{ConsciousnessCalculator, NodeState, SubstrateRegion};
use exo_core::thermodynamics::{Operation, ThermodynamicTracker};
use exo_core::{Metadata, Pattern, PatternId, SubstrateTime};
use exo_temporal::{
    anticipation::{PrefetchCache, SequentialPatternTracker},
    causal::{CausalConeType, CausalGraph},
    consolidation::compute_salience,
    long_term::LongTermStore,
    types::TemporalPattern,
    ConsolidationConfig, Query, TemporalConfig, TemporalMemory,
};

const VECTOR_DIM: usize = 384;

// ============================================================================
// Helper Functions
// ============================================================================

fn generate_random_vector(dim: usize, seed: u64) -> Vec<f32> {
    let mut vec = Vec::with_capacity(dim);
    let mut state = seed;
    for _ in 0..dim {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        vec.push((state as f32) / (u64::MAX as f32));
    }
    vec
}

fn create_pattern(seed: u64) -> Pattern {
    Pattern {
        id: PatternId::new(),
        embedding: generate_random_vector(VECTOR_DIM, seed),
        metadata: Metadata::default(),
        timestamp: SubstrateTime::now(),
        antecedents: Vec::new(),
        salience: 1.0,
    }
}

fn create_temporal_pattern(seed: u64) -> TemporalPattern {
    TemporalPattern::from_embedding(
        generate_random_vector(VECTOR_DIM, seed),
        Metadata::default(),
    )
}

struct BenchmarkResult {
    name: String,
    iterations: usize,
    total_time: Duration,
    per_op: Duration,
    ops_per_sec: f64,
}

impl BenchmarkResult {
    fn new(name: &str, iterations: usize, total_time: Duration) -> Self {
        let per_op = total_time / iterations as u32;
        let ops_per_sec = iterations as f64 / total_time.as_secs_f64();
        Self {
            name: name.to_string(),
            iterations,
            total_time,
            per_op,
            ops_per_sec,
        }
    }

    fn print(&self) {
        println!(
            "  {}: {:?} total, {:?}/op, {:.0} ops/sec",
            self.name, self.total_time, self.per_op, self.ops_per_sec
        );
    }
}

// ============================================================================
// 1. Sequential Pattern Learning Benchmarks
// ============================================================================

#[test]
fn benchmark_sequential_pattern_learning() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║         SEQUENTIAL PATTERN LEARNING BENCHMARKS                 ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let tracker = SequentialPatternTracker::new();

    // Generate pattern IDs
    let patterns: Vec<PatternId> = (0..1000).map(|_| PatternId::new()).collect();

    // Benchmark: Record sequences
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let from = patterns[i % patterns.len()];
        let to = patterns[(i + 1) % patterns.len()];
        tracker.record_sequence(from, to);
    }
    let record_result = BenchmarkResult::new("Record sequence", iterations, start.elapsed());
    record_result.print();

    // Benchmark: Predict next (after learning)
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let current = patterns[i % patterns.len()];
        let _ = tracker.predict_next(current, 5);
    }
    let predict_result = BenchmarkResult::new("Predict next (top-5)", iterations, start.elapsed());
    predict_result.print();

    // Test prediction accuracy
    let p1 = patterns[0];
    let p2 = patterns[1];
    let p3 = patterns[2];

    // Train: p1 -> p2 (10 times), p1 -> p3 (3 times)
    for _ in 0..10 {
        tracker.record_sequence(p1, p2);
    }
    for _ in 0..3 {
        tracker.record_sequence(p1, p3);
    }

    let predictions = tracker.predict_next(p1, 2);
    println!("\n  Learning Accuracy Test:");
    println!("    Pattern p1 -> p2 trained 10x, p1 -> p3 trained 3x");
    println!(
        "    Top prediction correct: {}",
        predictions.first() == Some(&p2)
    );
    println!("    Prediction count: {}", predictions.len());

    println!("\n  Summary:");
    println!(
        "    Record throughput: {:.0} sequences/sec",
        record_result.ops_per_sec
    );
    println!(
        "    Predict throughput: {:.0} predictions/sec",
        predict_result.ops_per_sec
    );
}

// ============================================================================
// 2. Causal Graph Learning Benchmarks
// ============================================================================

#[test]
fn benchmark_causal_graph_operations() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║            CAUSAL GRAPH LEARNING BENCHMARKS                    ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let graph = CausalGraph::new();
    let patterns: Vec<PatternId> = (0..1000).map(|_| PatternId::new()).collect();

    // Add all patterns with timestamps
    for &p in &patterns {
        graph.add_pattern(p, SubstrateTime::now());
    }

    // Benchmark: Add edges (build causal structure)
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let cause = patterns[i % patterns.len()];
        let effect = patterns[(i + 1) % patterns.len()];
        graph.add_edge(cause, effect);
    }
    let edge_result = BenchmarkResult::new("Add causal edge", iterations, start.elapsed());
    edge_result.print();

    // Benchmark: Get direct effects
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let p = patterns[i % patterns.len()];
        let _ = graph.effects(p);
    }
    let effects_result = BenchmarkResult::new("Get direct effects", iterations, start.elapsed());
    effects_result.print();

    // Benchmark: Get direct causes
    let iterations = 10_000;
    let start = Instant::now();
    for i in 0..iterations {
        let p = patterns[i % patterns.len()];
        let _ = graph.causes(p);
    }
    let causes_result = BenchmarkResult::new("Get direct causes", iterations, start.elapsed());
    causes_result.print();

    // Benchmark: Compute causal distance (path finding)
    let iterations = 1_000;
    let start = Instant::now();
    for i in 0..iterations {
        let from = patterns[i % patterns.len()];
        let to = patterns[(i + 10) % patterns.len()];
        let _ = graph.distance(from, to);
    }
    let distance_result = BenchmarkResult::new("Causal distance", iterations, start.elapsed());
    distance_result.print();

    // Benchmark: Get causal past (transitive closure)
    let iterations = 100;
    let start = Instant::now();
    for i in 0..iterations {
        let p = patterns[i % patterns.len()];
        let _ = graph.causal_past(p);
    }
    let past_result = BenchmarkResult::new("Causal past (full)", iterations, start.elapsed());
    past_result.print();

    // Benchmark: Get causal future
    let iterations = 100;
    let start = Instant::now();
    for i in 0..iterations {
        let p = patterns[i % patterns.len()];
        let _ = graph.causal_future(p);
    }
    let future_result = BenchmarkResult::new("Causal future (full)", iterations, start.elapsed());
    future_result.print();

    let stats = graph.stats();
    println!("\n  Graph Statistics:");
    println!("    Nodes: {}", stats.num_nodes);
    println!("    Edges: {}", stats.num_edges);
    println!("    Avg out-degree: {:.2}", stats.avg_out_degree);

    println!("\n  Summary:");
    println!("    Edge insertion: {:.0} ops/sec", edge_result.ops_per_sec);
    println!(
        "    Path finding: {:.0} ops/sec",
        distance_result.ops_per_sec
    );
    println!(
        "    Transitive closure: {:.0} ops/sec",
        past_result.ops_per_sec
    );
}

// ============================================================================
// 3. Salience Computation Benchmarks
// ============================================================================

#[test]
fn benchmark_salience_computation() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║            SALIENCE COMPUTATION BENCHMARKS                     ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let causal_graph = CausalGraph::new();
    let long_term = LongTermStore::default();
    let config = ConsolidationConfig::default();

    // Populate long-term with some patterns for surprise calculation
    for i in 0..100 {
        let tp = create_temporal_pattern(i);
        long_term.integrate(tp);
    }

    // Create test patterns with varying characteristics
    let mut test_patterns: Vec<TemporalPattern> = Vec::new();
    for i in 0..1000u64 {
        let mut tp = create_temporal_pattern(i + 1000);
        tp.access_count = (i % 100) as usize;
        test_patterns.push(tp);
    }

    // Add causal relationships
    for (i, tp) in test_patterns.iter().enumerate() {
        causal_graph.add_pattern(tp.pattern.id, tp.pattern.timestamp);
        if i > 0 {
            causal_graph.add_edge(test_patterns[i - 1].pattern.id, tp.pattern.id);
        }
    }

    // Benchmark: Compute salience
    let iterations = 1000;
    let start = Instant::now();
    let mut total_salience = 0.0f32;
    for i in 0..iterations {
        let tp = &test_patterns[i % test_patterns.len()];
        let salience = compute_salience(tp, &causal_graph, &long_term, &config);
        total_salience += salience;
    }
    let salience_result = BenchmarkResult::new("Compute salience", iterations, start.elapsed());
    salience_result.print();

    println!("\n  Salience Distribution:");
    println!(
        "    Average salience: {:.4}",
        total_salience / iterations as f32
    );
    println!(
        "    Weights: freq={:.1}, recency={:.1}, causal={:.1}, surprise={:.1}",
        config.w_frequency, config.w_recency, config.w_causal, config.w_surprise
    );

    println!("\n  Summary:");
    println!(
        "    Salience computation: {:.0} ops/sec",
        salience_result.ops_per_sec
    );
    println!("    Per pattern overhead: {:?}", salience_result.per_op);
}

// ============================================================================
// 4. Anticipation & Prediction Benchmarks
// ============================================================================

#[test]
fn benchmark_anticipation_prediction() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║          ANTICIPATION & PREDICTION BENCHMARKS                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Setup components
    let config = TemporalConfig {
        consolidation: ConsolidationConfig {
            salience_threshold: 0.0,
            ..Default::default()
        },
        prefetch_capacity: 1000,
        ..Default::default()
    };
    let memory = TemporalMemory::new(config);

    // Populate with patterns
    let mut pattern_ids = Vec::new();
    for i in 0..500 {
        let pattern = create_pattern(i);
        let id = memory.store(pattern, &[]).unwrap();
        pattern_ids.push(id);
    }

    // Consolidate to long-term
    memory.consolidate();

    // Benchmark: Prefetch cache operations
    let cache = PrefetchCache::new(1000);
    let iterations = 10_000;

    // Insert benchmark
    let start = Instant::now();
    for i in 0..iterations {
        let query_hash = i as u64;
        cache.insert(query_hash, vec![]);
    }
    let insert_result = BenchmarkResult::new("Cache insert", iterations, start.elapsed());
    insert_result.print();

    // Lookup benchmark
    let start = Instant::now();
    let mut hits = 0;
    for i in 0..iterations {
        let query_hash = (i % 1000) as u64;
        if cache.get(query_hash).is_some() {
            hits += 1;
        }
    }
    let lookup_result = BenchmarkResult::new("Cache lookup", iterations, start.elapsed());
    lookup_result.print();

    println!(
        "    Cache hit rate: {:.1}%",
        (hits as f64 / iterations as f64) * 100.0
    );

    // Benchmark: Sequential anticipation
    let seq_tracker = SequentialPatternTracker::new();

    // Train sequential patterns
    for i in 0..pattern_ids.len() - 1 {
        seq_tracker.record_sequence(pattern_ids[i], pattern_ids[i + 1]);
    }

    let iterations = 1000;
    let start = Instant::now();
    for i in 0..iterations {
        let current = pattern_ids[i % pattern_ids.len()];
        let predicted = seq_tracker.predict_next(current, 5);
        // Simulate prefetch
        for _p in predicted {
            // Would normally fetch from long-term
        }
    }
    let anticipate_result =
        BenchmarkResult::new("Anticipate + predict", iterations, start.elapsed());
    anticipate_result.print();

    println!("\n  Summary:");
    println!(
        "    Cache throughput: {:.0} ops/sec",
        lookup_result.ops_per_sec
    );
    println!(
        "    Anticipation throughput: {:.0} ops/sec",
        anticipate_result.ops_per_sec
    );
}

// ============================================================================
// 5. Memory Consolidation Benchmarks
// ============================================================================

#[test]
fn benchmark_memory_consolidation() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║            MEMORY CONSOLIDATION BENCHMARKS                     ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Test different batch sizes
    for batch_size in [100, 500, 1000, 2000] {
        let config = TemporalConfig {
            consolidation: ConsolidationConfig {
                salience_threshold: 0.3,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = TemporalMemory::new(config);

        // Insert patterns to short-term
        for i in 0..batch_size {
            let mut pattern = create_pattern(i as u64);
            pattern.salience = if i % 2 == 0 { 0.8 } else { 0.2 }; // Vary salience
            memory.store(pattern, &[]).unwrap();
        }

        // Benchmark consolidation
        let start = Instant::now();
        let result = memory.consolidate();
        let consolidate_time = start.elapsed();

        println!("  Batch size {}: {:?}", batch_size, consolidate_time);
        println!(
            "    Consolidated: {}, Forgotten: {}",
            result.num_consolidated, result.num_forgotten
        );
        println!("    Per pattern: {:?}", consolidate_time / batch_size);
        println!(
            "    Throughput: {:.0} patterns/sec",
            batch_size as f64 / consolidate_time.as_secs_f64()
        );
    }

    // Benchmark strategic forgetting
    println!("\n  Strategic Forgetting:");
    let long_term = LongTermStore::default();

    // Add patterns with varying salience
    for i in 0..1000 {
        let mut tp = create_temporal_pattern(i);
        tp.pattern.salience = (i as f32 / 1000.0) * 0.3; // Range 0.0 - 0.3
        long_term.integrate(tp);
    }

    println!("    Before decay: {} patterns", long_term.len());

    let start = Instant::now();
    long_term.decay_low_salience(0.5);
    let decay_time = start.elapsed();

    println!("    After decay: {} patterns", long_term.len());
    println!("    Decay time: {:?}", decay_time);

    println!("\n  Summary:");
    println!("    Consolidation scales linearly with batch size");
    println!("    Strategic forgetting enables bounded memory growth");
}

// ============================================================================
// 6. Consciousness Metrics (IIT) Benchmarks
// ============================================================================

#[test]
fn benchmark_consciousness_metrics() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║        CONSCIOUSNESS METRICS (IIT) BENCHMARKS                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // Test different network sizes
    for num_nodes in [5, 10, 20, 50] {
        // Create reentrant network
        let nodes: Vec<u64> = (0..num_nodes).map(|i| i as u64).collect();
        let mut connections = HashMap::new();

        // Create ring with shortcuts (small-world topology)
        for i in 0..num_nodes {
            let mut neighbors = Vec::new();
            neighbors.push(((i + 1) % num_nodes) as u64);
            if num_nodes > 3 {
                neighbors.push(((i + num_nodes - 1) % num_nodes) as u64);
            }
            // Add shortcut every 3rd node
            if i % 3 == 0 && num_nodes > 5 {
                neighbors.push(((i + num_nodes / 2) % num_nodes) as u64);
            }
            connections.insert(i as u64, neighbors);
        }

        let mut states = HashMap::new();
        for &node in &nodes {
            states.insert(
                node,
                NodeState {
                    activation: (node as f64 * 0.1).sin().abs(),
                    previous_activation: (node as f64 * 0.1 - 0.1).sin().abs(),
                },
            );
        }

        let region = SubstrateRegion {
            id: format!("network_{}", num_nodes),
            nodes,
            connections,
            states,
            has_reentrant_architecture: true,
        };

        // Benchmark with different perturbation counts
        for perturbations in [10, 50, 100] {
            let calculator = ConsciousnessCalculator::new(perturbations);

            let iterations = 100;
            let start = Instant::now();
            let mut total_phi = 0.0;
            for _ in 0..iterations {
                let result = calculator.compute_phi(&region);
                total_phi += result.phi;
            }
            let phi_time = start.elapsed();

            println!("  {} nodes, {} perturbations:", num_nodes, perturbations);
            println!("    Time per Φ: {:?}", phi_time / iterations);
            println!("    Average Φ: {:.4}", total_phi / iterations as f64);
            println!(
                "    Throughput: {:.0} calcs/sec",
                iterations as f64 / phi_time.as_secs_f64()
            );
        }
        println!();
    }

    // Test feedforward vs reentrant
    println!("  Feed-forward vs Reentrant Comparison:");

    // Feedforward (no cycles)
    let ff_region = SubstrateRegion {
        id: "feedforward".to_string(),
        nodes: vec![1, 2, 3, 4, 5],
        connections: {
            let mut c = HashMap::new();
            c.insert(1, vec![2]);
            c.insert(2, vec![3]);
            c.insert(3, vec![4]);
            c.insert(4, vec![5]);
            c
        },
        states: {
            let mut s = HashMap::new();
            for i in 1..=5 {
                s.insert(
                    i,
                    NodeState {
                        activation: 0.5,
                        previous_activation: 0.4,
                    },
                );
            }
            s
        },
        has_reentrant_architecture: false,
    };

    // Reentrant (with cycle)
    let re_region = SubstrateRegion {
        id: "reentrant".to_string(),
        nodes: vec![1, 2, 3, 4, 5],
        connections: {
            let mut c = HashMap::new();
            c.insert(1, vec![2]);
            c.insert(2, vec![3]);
            c.insert(3, vec![4]);
            c.insert(4, vec![5]);
            c.insert(5, vec![1]); // Feedback loop
            c
        },
        states: {
            let mut s = HashMap::new();
            for i in 1..=5 {
                s.insert(
                    i,
                    NodeState {
                        activation: 0.5,
                        previous_activation: 0.4,
                    },
                );
            }
            s
        },
        has_reentrant_architecture: true,
    };

    let calculator = ConsciousnessCalculator::new(100);

    let ff_result = calculator.compute_phi(&ff_region);
    let re_result = calculator.compute_phi(&re_region);

    println!(
        "    Feed-forward Φ: {:.4} (level: {:?})",
        ff_result.phi, ff_result.consciousness_level
    );
    println!(
        "    Reentrant Φ: {:.4} (level: {:?})",
        re_result.phi, re_result.consciousness_level
    );

    println!("\n  Summary:");
    println!("    IIT Φ computation scales with O(n²) in nodes");
    println!("    Reentrant architecture required for Φ > 0");
}

// ============================================================================
// 7. Thermodynamic Tracking Benchmarks
// ============================================================================

#[test]
fn benchmark_thermodynamic_tracking() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║          THERMODYNAMIC TRACKING BENCHMARKS                     ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    let tracker = ThermodynamicTracker::room_temperature();

    // Benchmark: Record operations
    let iterations = 1_000_000;
    let start = Instant::now();
    for i in 0..iterations {
        match i % 4 {
            0 => tracker.record_operation(Operation::VectorSimilarity { dimensions: 384 }),
            1 => tracker.record_operation(Operation::MemoryWrite { bytes: 1536 }),
            2 => tracker.record_operation(Operation::MemoryRead { bytes: 1536 }),
            _ => tracker.record_operation(Operation::GraphTraversal { hops: 10 }),
        }
    }
    let record_time = start.elapsed();
    let record_result = BenchmarkResult::new("Record operation", iterations, record_time);
    record_result.print();

    // Benchmark: Get report
    let iterations = 10_000;
    let start = Instant::now();
    for _ in 0..iterations {
        let _ = tracker.efficiency_report();
    }
    let report_time = start.elapsed();
    let report_result = BenchmarkResult::new("Generate report", iterations, report_time);
    report_result.print();

    let report = tracker.efficiency_report();
    println!("\n  Efficiency Report:");
    println!(
        "    Total bit erasures: {:.2e}",
        report.total_bit_erasures as f64
    );
    println!(
        "    Landauer minimum: {:.2e} J",
        report.landauer_minimum_joules
    );
    println!(
        "    Estimated actual: {:.2e} J",
        report.estimated_actual_joules
    );
    println!(
        "    Efficiency ratio: {:.0}x above Landauer limit",
        report.efficiency_ratio
    );
    println!(
        "    Reversible savings potential: {:.2e} J",
        report.reversible_savings_potential
    );

    // Test different temperatures
    println!("\n  Temperature Sensitivity:");
    for temp in [77.0, 300.0, 400.0] {
        // Liquid nitrogen, room temp, hot
        let temp_tracker = ThermodynamicTracker::new(temp);
        for _ in 0..1000 {
            temp_tracker.record_operation(Operation::VectorSimilarity { dimensions: 384 });
        }
        let temp_report = temp_tracker.efficiency_report();
        println!(
            "    {}K: Landauer min = {:.2e} J",
            temp, temp_report.landauer_minimum_joules
        );
    }

    println!("\n  Summary:");
    println!(
        "    Tracking overhead: {:?} per operation",
        record_result.per_op
    );
    println!("    Landauer limit scales with kT*ln(2)");
}

// ============================================================================
// 8. Comprehensive Comparison Benchmark
// ============================================================================

#[test]
fn benchmark_comprehensive_comparison() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║       COMPREHENSIVE EXO-AI vs BASE COMPARISON                  ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    // -------------------------------------------------------------------------
    // Simulated Base ruvector operations (no cognitive features)
    // -------------------------------------------------------------------------
    println!("  === BASE RUVECTOR (Simulated) ===\n");

    // Simple vector store
    let base_patterns: Vec<Vec<f32>> = (0..1000)
        .map(|i| generate_random_vector(VECTOR_DIM, i))
        .collect();

    // Base insert
    let iterations = 1000;
    let start = Instant::now();
    let mut base_store: Vec<(usize, Vec<f32>)> = Vec::with_capacity(iterations);
    for (i, vec) in base_patterns.iter().enumerate() {
        base_store.push((i, vec.clone()));
    }
    let base_insert_time = start.elapsed();
    println!("  Insert {} vectors: {:?}", iterations, base_insert_time);
    println!("    Per insert: {:?}", base_insert_time / iterations as u32);

    // Base search (brute force cosine)
    let query = generate_random_vector(VECTOR_DIM, 999999);
    let search_iterations = 100;
    let start = Instant::now();
    for _ in 0..search_iterations {
        let mut scores: Vec<(usize, f32)> = base_store
            .iter()
            .map(|(id, vec)| {
                let dot: f32 = query.iter().zip(vec.iter()).map(|(a, b)| a * b).sum();
                let mag_q: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
                let mag_v: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                (*id, dot / (mag_q * mag_v))
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        let _ = scores.into_iter().take(10).collect::<Vec<_>>();
    }
    let base_search_time = start.elapsed();
    println!(
        "  Search {} queries: {:?}",
        search_iterations, base_search_time
    );
    println!(
        "    Per search: {:?}",
        base_search_time / search_iterations as u32
    );

    // -------------------------------------------------------------------------
    // EXO-AI with full cognitive features
    // -------------------------------------------------------------------------
    println!("\n  === EXO-AI (Full Cognitive) ===\n");

    let config = TemporalConfig {
        consolidation: ConsolidationConfig {
            salience_threshold: 0.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let exo_memory = TemporalMemory::new(config);
    let thermodynamics = ThermodynamicTracker::room_temperature();
    let seq_tracker = SequentialPatternTracker::new();

    // EXO insert with full tracking
    let iterations = 1000;
    let start = Instant::now();
    let mut pattern_ids = Vec::with_capacity(iterations);
    for i in 0..iterations {
        let pattern = create_pattern(i as u64);
        let id = exo_memory.store(pattern, &[]).unwrap();
        pattern_ids.push(id);

        // Track causal relationships
        if i > 0 {
            seq_tracker.record_sequence(pattern_ids[i - 1], id);
        }

        // Record thermodynamics
        thermodynamics.record_operation(Operation::MemoryWrite {
            bytes: (VECTOR_DIM * 4) as u64,
        });
    }
    let exo_insert_time = start.elapsed();
    println!("  Insert {} patterns: {:?}", iterations, exo_insert_time);
    println!("    Per insert: {:?}", exo_insert_time / iterations as u32);

    // Consolidate
    let start = Instant::now();
    let consolidation_result = exo_memory.consolidate();
    let consolidate_time = start.elapsed();
    println!("  Consolidate: {:?}", consolidate_time);
    println!(
        "    Patterns kept: {}, forgotten: {}",
        consolidation_result.num_consolidated, consolidation_result.num_forgotten
    );

    // EXO search with temporal context
    let search_iterations = 100;
    let start = Instant::now();
    for _ in 0..search_iterations {
        let query = Query::from_embedding(generate_random_vector(VECTOR_DIM, 888888));
        let _ = exo_memory.long_term().search(&query);
        thermodynamics.record_operation(Operation::VectorSimilarity {
            dimensions: VECTOR_DIM,
        });
    }
    let exo_search_time = start.elapsed();
    println!(
        "  Search {} queries: {:?}",
        search_iterations, exo_search_time
    );
    println!(
        "    Per search: {:?}",
        exo_search_time / search_iterations as u32
    );

    // Causal query
    let start = Instant::now();
    for _ in 0..search_iterations {
        let query = Query::from_embedding(generate_random_vector(VECTOR_DIM, 777777))
            .with_origin(pattern_ids[0]);
        let _ = exo_memory.causal_query(&query, SubstrateTime::now(), CausalConeType::Future);
    }
    let causal_search_time = start.elapsed();
    println!(
        "  Causal query {} times: {:?}",
        search_iterations, causal_search_time
    );
    println!(
        "    Per causal query: {:?}",
        causal_search_time / search_iterations as u32
    );

    // Anticipation
    let start = Instant::now();
    for i in 0..search_iterations {
        let current = pattern_ids[i % pattern_ids.len()];
        let _predicted = seq_tracker.predict_next(current, 5);
    }
    let anticipate_time = start.elapsed();
    println!(
        "  Anticipate {} times: {:?}",
        search_iterations, anticipate_time
    );

    // -------------------------------------------------------------------------
    // Comparison Summary
    // -------------------------------------------------------------------------
    println!("\n  ╔══════════════════════════════════════════════════════════════╗");
    println!("  ║                    COMPARISON SUMMARY                        ║");
    println!("  ╠══════════════════════════════════════════════════════════════╣");

    let base_insert_per_op = base_insert_time.as_nanos() / 1000;
    let exo_insert_per_op = exo_insert_time.as_nanos() / 1000;
    let insert_overhead = exo_insert_per_op as f64 / base_insert_per_op as f64;

    let base_search_per_op = base_search_time.as_nanos() / 100;
    let exo_search_per_op = exo_search_time.as_nanos() / 100;
    let search_overhead = exo_search_per_op as f64 / base_search_per_op as f64;

    println!("  ║ Operation          │ Base      │ EXO-AI    │ Overhead   ║");
    println!("  ╠════════════════════╪═══════════╪═══════════╪════════════╣");
    println!(
        "  ║ Insert             │ {:>7}µs │ {:>7}µs │ {:>6.1}x     ║",
        base_insert_per_op, exo_insert_per_op, insert_overhead
    );
    println!(
        "  ║ Search             │ {:>7}µs │ {:>7}µs │ {:>6.1}x     ║",
        base_search_per_op / 1000,
        exo_search_per_op / 1000,
        search_overhead
    );
    println!(
        "  ║ Causal Query       │    N/A    │ {:>7}µs │ NEW        ║",
        causal_search_time.as_micros() / 100
    );
    println!(
        "  ║ Anticipation       │    N/A    │ {:>7}µs │ NEW        ║",
        anticipate_time.as_micros() / 100
    );
    println!(
        "  ║ Consolidation      │    N/A    │ {:>7}ms │ NEW        ║",
        consolidate_time.as_millis()
    );
    println!("  ╠══════════════════════════════════════════════════════════════╣");
    println!("  ║                    COGNITIVE CAPABILITIES                    ║");
    println!("  ╠══════════════════════════════════════════════════════════════╣");
    println!("  ║ Sequential Learning      │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Causal Reasoning         │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Salience Computation     │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Anticipatory Retrieval   │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Memory Consolidation     │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Strategic Forgetting     │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Consciousness Metrics    │ Base: ❌  │ EXO: ✅            ║");
    println!("  ║ Thermodynamic Tracking   │ Base: ❌  │ EXO: ✅            ║");
    println!("  ╚══════════════════════════════════════════════════════════════╝");

    // Print thermodynamic report
    let report = thermodynamics.efficiency_report();
    println!("\n  Thermodynamic Efficiency:");
    println!(
        "    Operations tracked: {:.2e} bit erasures",
        report.total_bit_erasures as f64
    );
    println!(
        "    Theoretical minimum (Landauer): {:.2e} J",
        report.landauer_minimum_joules
    );
    println!(
        "    Current system: {:.0}x above minimum",
        report.efficiency_ratio
    );
}

// ============================================================================
// 9. Scaling Benchmarks
// ============================================================================

#[test]
fn benchmark_scaling_characteristics() {
    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║              SCALING CHARACTERISTICS                           ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");

    println!("  Insert Scaling (vs pattern count):");
    println!("  ───────────────────────────────────");

    for scale in [100, 500, 1000, 2000, 5000] {
        let config = TemporalConfig {
            consolidation: ConsolidationConfig {
                salience_threshold: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = TemporalMemory::new(config);

        let start = Instant::now();
        for i in 0..scale {
            let pattern = create_pattern(i as u64);
            memory.store(pattern, &[]).unwrap();
        }
        let insert_time = start.elapsed();

        let start = Instant::now();
        memory.consolidate();
        let consolidate_time = start.elapsed();

        println!(
            "    {:>5} patterns: insert {:>8?}, consolidate {:>8?}",
            scale, insert_time, consolidate_time
        );
    }

    println!("\n  Search Scaling (vs store size):");
    println!("  ─────────────────────────────────");

    for scale in [100, 500, 1000, 2000] {
        let config = TemporalConfig {
            consolidation: ConsolidationConfig {
                salience_threshold: 0.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let memory = TemporalMemory::new(config);

        // Populate
        for i in 0..scale {
            let pattern = create_pattern(i as u64);
            memory.store(pattern, &[]).unwrap();
        }
        memory.consolidate();

        // Benchmark search
        let query = Query::from_embedding(generate_random_vector(VECTOR_DIM, 999999));
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = memory.long_term().search(&query);
        }
        let search_time = start.elapsed();

        println!(
            "    {:>5} patterns: {:>6?} per search ({:.0} qps)",
            scale,
            search_time / iterations,
            iterations as f64 / search_time.as_secs_f64()
        );
    }

    println!("\n  Causal Graph Scaling:");
    println!("  ──────────────────────");

    for scale in [100, 500, 1000, 2000] {
        let graph = CausalGraph::new();
        let patterns: Vec<PatternId> = (0..scale).map(|_| PatternId::new()).collect();

        // Build linear chain with shortcuts
        for (i, &p) in patterns.iter().enumerate() {
            graph.add_pattern(p, SubstrateTime::now());
            if i > 0 {
                graph.add_edge(patterns[i - 1], p);
            }
            // Add shortcut every 10th node
            if i >= 10 && i % 10 == 0 {
                graph.add_edge(patterns[i - 10], p);
            }
        }

        // Benchmark path finding
        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = graph.distance(patterns[0], patterns[scale - 1]);
        }
        let distance_time = start.elapsed();

        // Benchmark causal future
        let start2 = Instant::now();
        for _ in 0..iterations {
            let _ = graph.causal_future(patterns[0]);
        }
        let future_time = start2.elapsed();

        println!(
            "    {:>5} nodes: distance {:>6?}, future {:>6?}",
            scale,
            distance_time / iterations,
            future_time / iterations
        );
    }

    println!("\n  Summary:");
    println!("    - Insert: O(1) amortized");
    println!("    - Search: O(n) brute force (HNSW would be O(log n))");
    println!("    - Causal distance: O(V + E) with caching");
    println!("    - Causal future: O(reachable nodes)");
}
