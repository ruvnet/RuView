//! # Exotic MinCut Examples - Comprehensive Benchmarks
//!
//! This benchmark suite measures performance across all exotic use cases:
//! - Temporal Attractors
//! - Strange Loop Swarms
//! - Causal Discovery
//! - Time Crystal Coordination
//! - Morphogenetic Networks
//! - Neural Graph Optimization
//!
//! Run with: `cargo run --release -p mincut-benchmarks`

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

// ============================================================================
// BENCHMARK INFRASTRUCTURE
// ============================================================================

/// Benchmark result with statistics
#[derive(Debug, Clone)]
struct BenchResult {
    name: String,
    iterations: usize,
    total_time: Duration,
    min_time: Duration,
    max_time: Duration,
    avg_time: Duration,
    throughput: f64, // operations per second
}

impl BenchResult {
    fn print(&self) {
        println!("  {} ({} iterations)", self.name, self.iterations);
        println!("    Total:      {:?}", self.total_time);
        println!("    Average:    {:?}", self.avg_time);
        println!("    Min:        {:?}", self.min_time);
        println!("    Max:        {:?}", self.max_time);
        println!("    Throughput: {:.2} ops/sec", self.throughput);
        println!();
    }
}

/// Run a benchmark with the given closure
fn bench<F>(name: &str, iterations: usize, mut f: F) -> BenchResult
where
    F: FnMut() -> (),
{
    let mut times = Vec::with_capacity(iterations);

    // Warmup
    for _ in 0..3 {
        f();
    }

    // Actual benchmark
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed());
    }

    let total: Duration = times.iter().sum();
    let min = *times.iter().min().unwrap();
    let max = *times.iter().max().unwrap();
    let avg = total / iterations as u32;
    let throughput = iterations as f64 / total.as_secs_f64();

    BenchResult {
        name: name.to_string(),
        iterations,
        total_time: total,
        min_time: min,
        max_time: max,
        avg_time: avg,
        throughput,
    }
}

// ============================================================================
// SIMPLE GRAPH IMPLEMENTATION FOR BENCHMARKS
// ============================================================================

/// Lightweight graph for benchmarking
#[derive(Clone)]
struct BenchGraph {
    vertices: HashSet<u64>,
    edges: HashMap<(u64, u64), f64>,
    adjacency: HashMap<u64, Vec<u64>>,
}

impl BenchGraph {
    fn new() -> Self {
        Self {
            vertices: HashSet::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    fn with_vertices(n: usize) -> Self {
        let mut g = Self::new();
        for i in 0..n as u64 {
            g.vertices.insert(i);
            g.adjacency.insert(i, Vec::new());
        }
        g
    }

    fn add_edge(&mut self, u: u64, v: u64, weight: f64) {
        if !self.vertices.contains(&u) {
            self.vertices.insert(u);
            self.adjacency.insert(u, Vec::new());
        }
        if !self.vertices.contains(&v) {
            self.vertices.insert(v);
            self.adjacency.insert(v, Vec::new());
        }

        let key = if u < v { (u, v) } else { (v, u) };
        self.edges.insert(key, weight);
        self.adjacency.get_mut(&u).unwrap().push(v);
        self.adjacency.get_mut(&v).unwrap().push(u);
    }

    fn remove_edge(&mut self, u: u64, v: u64) {
        let key = if u < v { (u, v) } else { (v, u) };
        self.edges.remove(&key);
        if let Some(adj) = self.adjacency.get_mut(&u) {
            adj.retain(|&x| x != v);
        }
        if let Some(adj) = self.adjacency.get_mut(&v) {
            adj.retain(|&x| x != u);
        }
    }

    fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    fn edge_count(&self) -> usize {
        self.edges.len()
    }

    fn degree(&self, v: u64) -> usize {
        self.adjacency.get(&v).map(|a| a.len()).unwrap_or(0)
    }

    /// Simple min-cut approximation using minimum degree
    fn approx_mincut(&self) -> f64 {
        self.vertices
            .iter()
            .map(|&v| self.degree(v) as f64)
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0)
    }
}

// ============================================================================
// BENCHMARK: TEMPORAL ATTRACTORS
// ============================================================================

fn bench_temporal_attractors() -> Vec<BenchResult> {
    println!("\n{:=^60}", " TEMPORAL ATTRACTORS ");
    let mut results = Vec::new();

    // Benchmark attractor evolution
    for size in [100, 500, 1000, 5000] {
        let result = bench(&format!("evolve_step (n={})", size), 100, || {
            let mut graph = BenchGraph::with_vertices(size);

            // Create initial ring
            for i in 0..size as u64 {
                graph.add_edge(i, (i + 1) % size as u64, 1.0);
            }

            // Evolve toward optimal attractor
            for _ in 0..10 {
                let cut = graph.approx_mincut();
                if cut < 3.0 {
                    // Strengthen weak points
                    let weak_v = (0..size as u64).min_by_key(|&v| graph.degree(v)).unwrap();
                    let target = (weak_v + size as u64 / 2) % size as u64;
                    graph.add_edge(weak_v, target, 1.0);
                }
            }
        });
        result.print();
        results.push(result);
    }

    // Benchmark convergence detection
    let result = bench("convergence_detection (100 samples)", 1000, || {
        let samples: Vec<f64> = (0..100).map(|i| 5.0 + (i as f64 * 0.01)).collect();
        let _variance: f64 = {
            let mean = samples.iter().sum::<f64>() / samples.len() as f64;
            samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / samples.len() as f64
        };
    });
    result.print();
    results.push(result);

    results
}

// ============================================================================
// BENCHMARK: STRANGE LOOP SWARMS
// ============================================================================

fn bench_strange_loop() -> Vec<BenchResult> {
    println!("\n{:=^60}", " STRANGE LOOP SWARMS ");
    let mut results = Vec::new();

    // Benchmark self-observation
    for size in [100, 500, 1000] {
        let result = bench(&format!("self_observe (n={})", size), 100, || {
            let mut graph = BenchGraph::with_vertices(size);

            // Create mesh
            for i in 0..size as u64 {
                for j in (i + 1)..std::cmp::min(i + 5, size as u64) {
                    graph.add_edge(i, j, 1.0);
                }
            }

            // Self-observation cycle
            let _mincut = graph.approx_mincut();
            let _weak_vertices: Vec<u64> =
                (0..size as u64).filter(|&v| graph.degree(v) < 3).collect();
        });
        result.print();
        results.push(result);
    }

    // Benchmark feedback loop iteration
    let result = bench("feedback_loop_iteration (n=500)", 100, || {
        let mut graph = BenchGraph::with_vertices(500);

        // Initialize
        for i in 0..500u64 {
            graph.add_edge(i, (i + 1) % 500, 1.0);
        }

        // 10 feedback iterations
        for _ in 0..10 {
            // Observe
            let cut = graph.approx_mincut();

            // Decide
            if cut < 3.0 {
                // Strengthen
                let v = (0..500u64).min_by_key(|&v| graph.degree(v)).unwrap();
                graph.add_edge(v, (v + 250) % 500, 1.0);
            }
        }
    });
    result.print();
    results.push(result);

    results
}

// ============================================================================
// BENCHMARK: CAUSAL DISCOVERY
// ============================================================================

fn bench_causal_discovery() -> Vec<BenchResult> {
    println!("\n{:=^60}", " CAUSAL DISCOVERY ");
    let mut results = Vec::new();

    // Benchmark event tracking
    let result = bench("event_tracking (1000 events)", 100, || {
        let mut events: Vec<(Instant, &str, f64)> = Vec::with_capacity(1000);
        let base = Instant::now();

        for i in 0..1000 {
            events.push((
                base,
                if i % 3 == 0 {
                    "edge_cut"
                } else {
                    "mincut_change"
                },
                i as f64,
            ));
        }
    });
    result.print();
    results.push(result);

    // Benchmark causality detection
    for event_count in [100, 500, 1000] {
        let result = bench(
            &format!("causality_detection (n={})", event_count),
            50,
            || {
                // Simulate event pairs
                let events: Vec<(u64, u64)> = (0..event_count)
                    .map(|i| (i as u64, i as u64 + 50))
                    .collect();

                // Find causal relationships
                let mut causal_pairs: HashMap<(&str, &str), Vec<u64>> = HashMap::new();

                for (t1, t2) in &events {
                    let delay = t2 - t1;
                    if delay < 200 {
                        causal_pairs
                            .entry(("A", "B"))
                            .or_insert_with(Vec::new)
                            .push(delay);
                    }
                }

                // Calculate statistics
                for (_pair, delays) in &causal_pairs {
                    let _avg: f64 = delays.iter().sum::<u64>() as f64 / delays.len() as f64;
                }
            },
        );
        result.print();
        results.push(result);
    }

    results
}

// ============================================================================
// BENCHMARK: TIME CRYSTAL COORDINATION
// ============================================================================

fn bench_time_crystal() -> Vec<BenchResult> {
    println!("\n{:=^60}", " TIME CRYSTAL COORDINATION ");
    let mut results = Vec::new();

    // Benchmark phase transitions
    for size in [50, 100, 500] {
        let result = bench(&format!("phase_transition (n={})", size), 100, || {
            let mut graph = BenchGraph::with_vertices(size);

            // Phase 1: Ring
            for i in 0..size as u64 {
                graph.add_edge(i, (i + 1) % size as u64, 1.0);
            }
            let _ring_cut = graph.approx_mincut();

            // Phase 2: Star (clear and rebuild)
            graph.edges.clear();
            for adj in graph.adjacency.values_mut() {
                adj.clear();
            }
            for i in 1..size as u64 {
                graph.add_edge(0, i, 1.0);
            }
            let _star_cut = graph.approx_mincut();

            // Phase 3: Mesh
            graph.edges.clear();
            for adj in graph.adjacency.values_mut() {
                adj.clear();
            }
            for i in 0..size as u64 {
                for j in (i + 1)..std::cmp::min(i + 4, size as u64) {
                    graph.add_edge(i, j, 1.0);
                }
            }
            let _mesh_cut = graph.approx_mincut();
        });
        result.print();
        results.push(result);
    }

    // Benchmark stability verification
    let result = bench("stability_verification (9 phases)", 200, || {
        let expected: Vec<f64> = vec![2.0, 1.0, 6.0, 2.0, 1.0, 6.0, 2.0, 1.0, 6.0];
        let actual: Vec<f64> = vec![2.0, 1.0, 6.0, 2.0, 1.0, 6.0, 2.0, 1.0, 6.0];

        let _matches: usize = expected
            .iter()
            .zip(&actual)
            .filter(|(e, a)| (*e - *a).abs() < 0.5)
            .count();
    });
    result.print();
    results.push(result);

    results
}

// ============================================================================
// BENCHMARK: MORPHOGENETIC NETWORKS
// ============================================================================

fn bench_morphogenetic() -> Vec<BenchResult> {
    println!("\n{:=^60}", " MORPHOGENETIC NETWORKS ");
    let mut results = Vec::new();

    // Benchmark growth cycle
    for initial_size in [10, 50, 100] {
        let result = bench(
            &format!("growth_cycle (start={})", initial_size),
            50,
            || {
                let mut graph = BenchGraph::with_vertices(initial_size);
                let mut signals: HashMap<u64, f64> = HashMap::new();

                // Initialize signals
                for i in 0..initial_size as u64 {
                    signals.insert(i, 1.0);
                }

                // Create initial connections
                for i in 0..initial_size as u64 {
                    graph.add_edge(i, (i + 1) % initial_size as u64, 1.0);
                }

                // 15 growth cycles
                let mut next_id = initial_size as u64;
                for _ in 0..15 {
                    // Diffuse signals
                    let mut new_signals = signals.clone();
                    for (&v, &sig) in &signals {
                        for &neighbor in graph.adjacency.get(&v).unwrap_or(&vec![]) {
                            *new_signals.entry(neighbor).or_insert(0.0) += sig * 0.1;
                        }
                    }

                    // Decay
                    for sig in new_signals.values_mut() {
                        *sig *= 0.9;
                    }
                    signals = new_signals;

                    // Growth rules
                    for v in 0..next_id {
                        if !graph.vertices.contains(&v) {
                            continue;
                        }

                        let sig = signals.get(&v).copied().unwrap_or(0.0);
                        let deg = graph.degree(v);

                        if sig > 0.5 && deg < 2 {
                            // Spawn
                            graph.add_edge(v, next_id, 1.0);
                            signals.insert(next_id, sig * 0.5);
                            next_id += 1;
                        }
                    }
                }
            },
        );
        result.print();
        results.push(result);
    }

    results
}

// ============================================================================
// BENCHMARK: NEURAL GRAPH OPTIMIZER
// ============================================================================

fn bench_neural_optimizer() -> Vec<BenchResult> {
    println!("\n{:=^60}", " NEURAL GRAPH OPTIMIZER ");
    let mut results = Vec::new();

    // Benchmark feature extraction
    for size in [50, 100, 500] {
        let result = bench(&format!("feature_extraction (n={})", size), 100, || {
            let mut graph = BenchGraph::with_vertices(size);
            for i in 0..size as u64 {
                graph.add_edge(i, (i + 1) % size as u64, 1.0);
            }

            // Extract features
            let _features: Vec<f64> = vec![
                graph.vertex_count() as f64 / 1000.0,
                graph.edge_count() as f64 / 5000.0,
                graph.approx_mincut() / 10.0,
                graph.edges.values().sum::<f64>() / graph.edge_count() as f64,
            ];
        });
        result.print();
        results.push(result);
    }

    // Benchmark neural forward pass (simulated)
    let result = bench("neural_forward_pass (4 layers)", 1000, || {
        // Simulate 4-layer network
        let input = vec![0.5, 0.3, 0.2, 0.8];
        let mut activation = input;

        for layer in 0..4 {
            let mut output = vec![0.0; 4];
            for i in 0..4 {
                for j in 0..4 {
                    output[i] += activation[j] * ((layer * 4 + i + j) as f64 * 0.1).sin();
                }
                output[i] = output[i].max(0.0); // ReLU
            }
            activation = output;
        }
    });
    result.print();
    results.push(result);

    // Benchmark optimization step
    let result = bench("optimization_step (10 candidates)", 100, || {
        let mut best_score = 0.0f64;

        for candidate in 0..10 {
            // Simulate action evaluation
            let score = (candidate as f64 * 0.1).sin() + 0.5;
            if score > best_score {
                best_score = score;
            }
        }
    });
    result.print();
    results.push(result);

    results
}

// ============================================================================
// BENCHMARK: SCALING ANALYSIS
// ============================================================================

fn bench_scaling() -> Vec<BenchResult> {
    println!("\n{:=^60}", " SCALING ANALYSIS ");
    let mut results = Vec::new();

    // Test how performance scales with graph size
    for size in [100, 500, 1000, 5000, 10000] {
        let result = bench(&format!("full_pipeline (n={})", size), 10, || {
            let mut graph = BenchGraph::with_vertices(size);

            // Build graph
            for i in 0..size as u64 {
                graph.add_edge(i, (i + 1) % size as u64, 1.0);
                if i % 10 == 0 {
                    graph.add_edge(i, (i + size as u64 / 2) % size as u64, 1.0);
                }
            }

            // Run analysis
            let _cut = graph.approx_mincut();

            // Simulate evolution
            for _ in 0..5 {
                let cut = graph.approx_mincut();
                if cut < 3.0 {
                    let v = (0..size as u64).min_by_key(|&v| graph.degree(v)).unwrap();
                    graph.add_edge(v, (v + 1) % size as u64, 1.0);
                }
            }
        });
        result.print();
        results.push(result);
    }

    results
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       EXOTIC MINCUT EXAMPLES - BENCHMARK SUITE             ║");
    println!("║       Measuring Performance Across All Use Cases           ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let mut all_results = Vec::new();

    all_results.extend(bench_temporal_attractors());
    all_results.extend(bench_strange_loop());
    all_results.extend(bench_causal_discovery());
    all_results.extend(bench_time_crystal());
    all_results.extend(bench_morphogenetic());
    all_results.extend(bench_neural_optimizer());
    all_results.extend(bench_scaling());

    // Summary
    println!("\n{:=^60}", " SUMMARY ");
    println!("\nTop 5 Fastest Operations:");
    let mut sorted = all_results.clone();
    sorted.sort_by(|a, b| a.avg_time.cmp(&b.avg_time));
    for result in sorted.iter().take(5) {
        println!("  {:50} {:>10?}", result.name, result.avg_time);
    }

    println!("\nTop 5 Highest Throughput:");
    sorted.sort_by(|a, b| b.throughput.partial_cmp(&a.throughput).unwrap());
    for result in sorted.iter().take(5) {
        println!("  {:50} {:>12.0} ops/sec", result.name, result.throughput);
    }

    println!("\nScaling Analysis:");
    for result in all_results
        .iter()
        .filter(|r| r.name.starts_with("full_pipeline"))
    {
        println!("  {:50} {:>10?}", result.name, result.avg_time);
    }

    println!("\n✅ Benchmark suite complete!");
}
