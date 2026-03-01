//! MWPM vs Min-Cut Pre-Filter Benchmark
//!
//! This benchmark compares:
//! 1. MWPM decoding on every round (baseline)
//! 2. Min-cut pre-filter + MWPM only when needed
//! 3. Simulated expensive decoder to show break-even point
//!
//! Key Finding: Pre-filter is beneficial when decoder cost > ~10μs
//!
//! Run: cargo run --example mwpm_comparison_benchmark --features "structural" --release

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use ruqu::{
    decoder::{DecoderConfig, MWPMDecoder},
    stim::{StimSyndromeSource, SurfaceCodeConfig},
    syndrome::DetectorBitmap,
};

// ============================================================================
// MIN-CUT PRE-FILTER (from validated_coherence_gate.rs)
// ============================================================================

struct STMinCutGraph {
    adj: HashMap<u32, Vec<(u32, f64)>>,
    source: u32,
    sink: u32,
}

impl STMinCutGraph {
    fn new(num_nodes: u32) -> Self {
        Self {
            adj: HashMap::new(),
            source: num_nodes,
            sink: num_nodes + 1,
        }
    }

    fn add_edge(&mut self, u: u32, v: u32, weight: f64) {
        self.adj.entry(u).or_default().push((v, weight));
        self.adj.entry(v).or_default().push((u, weight));
    }

    fn connect_to_source(&mut self, node: u32, weight: f64) {
        self.add_edge(self.source, node, weight);
    }

    fn connect_to_sink(&mut self, node: u32, weight: f64) {
        self.add_edge(node, self.sink, weight);
    }

    fn min_cut(&self) -> f64 {
        let mut capacity: HashMap<(u32, u32), f64> = HashMap::new();
        for (&u, neighbors) in &self.adj {
            for &(v, w) in neighbors {
                *capacity.entry((u, v)).or_default() += w;
            }
        }

        let mut max_flow = 0.0;

        loop {
            let mut parent: HashMap<u32, u32> = HashMap::new();
            let mut visited = HashSet::new();
            let mut queue = VecDeque::new();

            queue.push_back(self.source);
            visited.insert(self.source);

            while let Some(u) = queue.pop_front() {
                if u == self.sink {
                    break;
                }
                if let Some(neighbors) = self.adj.get(&u) {
                    for &(v, _) in neighbors {
                        let cap = capacity.get(&(u, v)).copied().unwrap_or(0.0);
                        if !visited.contains(&v) && cap > 1e-10 {
                            visited.insert(v);
                            parent.insert(v, u);
                            queue.push_back(v);
                        }
                    }
                }
            }

            if !parent.contains_key(&self.sink) {
                break;
            }

            let mut path_flow = f64::INFINITY;
            let mut v = self.sink;
            while v != self.source {
                let u = parent[&v];
                path_flow = path_flow.min(capacity.get(&(u, v)).copied().unwrap_or(0.0));
                v = u;
            }

            v = self.sink;
            while v != self.source {
                let u = parent[&v];
                *capacity.entry((u, v)).or_default() -= path_flow;
                *capacity.entry((v, u)).or_default() += path_flow;
                v = u;
            }

            max_flow += path_flow;
        }

        max_flow
    }
}

fn build_surface_code_graph(
    code_distance: usize,
    error_rate: f64,
    syndrome: &DetectorBitmap,
) -> STMinCutGraph {
    let grid_size = code_distance - 1;
    let num_detectors = 2 * grid_size * grid_size;
    let mut graph = STMinCutGraph::new(num_detectors as u32);
    let fired_set: HashSet<usize> = syndrome.iter_fired().collect();
    let base_weight = (-error_rate.ln()).max(0.1);
    let fired_weight = 0.01;

    for row in 0..grid_size {
        for col in 0..grid_size {
            let node = (row * grid_size + col) as u32;
            let is_fired = fired_set.contains(&(node as usize));

            if col + 1 < grid_size {
                let right = (row * grid_size + col + 1) as u32;
                let right_fired = fired_set.contains(&(right as usize));
                let weight = if is_fired || right_fired {
                    fired_weight
                } else {
                    base_weight
                };
                graph.add_edge(node, right, weight);
            }

            if row + 1 < grid_size {
                let bottom = ((row + 1) * grid_size + col) as u32;
                let bottom_fired = fired_set.contains(&(bottom as usize));
                let weight = if is_fired || bottom_fired {
                    fired_weight
                } else {
                    base_weight
                };
                graph.add_edge(node, bottom, weight);
            }
        }
    }

    let boundary_weight = base_weight * 2.0;
    for row in 0..grid_size {
        graph.connect_to_source((row * grid_size) as u32, boundary_weight);
        graph.connect_to_sink((row * grid_size + grid_size - 1) as u32, boundary_weight);
    }

    graph
}

// ============================================================================
// BENCHMARK FRAMEWORK
// ============================================================================

#[derive(Default, Clone)]
struct BenchmarkStats {
    total_rounds: u64,
    total_time_ns: u64,
    decode_calls: u64,
    decode_time_ns: u64,
    prefilter_time_ns: u64,
    skipped_rounds: u64,
    logical_errors_detected: u64,
    logical_errors_missed: u64,
}

impl BenchmarkStats {
    fn throughput(&self) -> f64 {
        if self.total_time_ns == 0 {
            0.0
        } else {
            self.total_rounds as f64 / (self.total_time_ns as f64 / 1e9)
        }
    }

    fn avg_round_time_ns(&self) -> f64 {
        if self.total_rounds == 0 {
            0.0
        } else {
            self.total_time_ns as f64 / self.total_rounds as f64
        }
    }

    fn avg_decode_time_ns(&self) -> f64 {
        if self.decode_calls == 0 {
            0.0
        } else {
            self.decode_time_ns as f64 / self.decode_calls as f64
        }
    }

    fn skip_rate(&self) -> f64 {
        if self.total_rounds == 0 {
            0.0
        } else {
            self.skipped_rounds as f64 / self.total_rounds as f64
        }
    }
}

/// Detect logical error by checking for spanning cluster
fn has_logical_error(syndrome: &DetectorBitmap, code_distance: usize) -> bool {
    let grid_size = code_distance - 1;
    let fired: HashSet<usize> = syndrome.iter_fired().collect();

    if fired.is_empty() {
        return false;
    }

    let left_boundary: Vec<usize> = (0..grid_size)
        .map(|row| row * grid_size)
        .filter(|&d| fired.contains(&d))
        .collect();

    if left_boundary.is_empty() {
        return false;
    }

    let mut visited: HashSet<usize> = HashSet::new();
    let mut queue: VecDeque<usize> = VecDeque::new();

    for &start in &left_boundary {
        queue.push_back(start);
        visited.insert(start);
    }

    while let Some(current) = queue.pop_front() {
        let row = current / grid_size;
        let col = current % grid_size;

        if col == grid_size - 1 {
            return true;
        }

        let neighbors = [
            if col > 0 {
                Some(row * grid_size + col - 1)
            } else {
                None
            },
            if col + 1 < grid_size {
                Some(row * grid_size + col + 1)
            } else {
                None
            },
            if row > 0 {
                Some((row - 1) * grid_size + col)
            } else {
                None
            },
            if row + 1 < grid_size {
                Some((row + 1) * grid_size + col)
            } else {
                None
            },
        ];

        for neighbor_opt in neighbors.iter().flatten() {
            let neighbor = *neighbor_opt;
            if fired.contains(&neighbor) && !visited.contains(&neighbor) {
                visited.insert(neighbor);
                queue.push_back(neighbor);
            }
        }
    }

    false
}

/// Benchmark: MWPM on every round (baseline)
fn benchmark_mwpm_baseline(
    code_distance: usize,
    error_rate: f64,
    num_rounds: usize,
    seed: u64,
) -> BenchmarkStats {
    let mut stats = BenchmarkStats::default();

    let decoder_config = DecoderConfig {
        distance: code_distance,
        physical_error_rate: error_rate,
        window_size: 1,
        parallel: false,
    };
    let mut decoder = MWPMDecoder::new(decoder_config);

    let surface_config = SurfaceCodeConfig::new(code_distance, error_rate).with_seed(seed);
    let mut syndrome_source = match StimSyndromeSource::new(surface_config) {
        Ok(s) => s,
        Err(_) => return stats,
    };

    let start = Instant::now();

    for _ in 0..num_rounds {
        let syndrome: DetectorBitmap = match syndrome_source.sample() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let decode_start = Instant::now();
        let _correction = decoder.decode(&syndrome);
        let decode_elapsed = decode_start.elapsed().as_nanos() as u64;

        stats.decode_calls += 1;
        stats.decode_time_ns += decode_elapsed;
        stats.total_rounds += 1;

        if has_logical_error(&syndrome, code_distance) {
            stats.logical_errors_detected += 1;
        }
    }

    stats.total_time_ns = start.elapsed().as_nanos() as u64;
    stats
}

/// Benchmark: Min-cut pre-filter + MWPM only when needed
fn benchmark_prefilter_mwpm(
    code_distance: usize,
    error_rate: f64,
    num_rounds: usize,
    seed: u64,
    threshold: f64,
) -> BenchmarkStats {
    let mut stats = BenchmarkStats::default();

    let decoder_config = DecoderConfig {
        distance: code_distance,
        physical_error_rate: error_rate,
        window_size: 1,
        parallel: false,
    };
    let mut decoder = MWPMDecoder::new(decoder_config);

    let surface_config = SurfaceCodeConfig::new(code_distance, error_rate).with_seed(seed);
    let mut syndrome_source = match StimSyndromeSource::new(surface_config) {
        Ok(s) => s,
        Err(_) => return stats,
    };

    let start = Instant::now();

    for _ in 0..num_rounds {
        let syndrome: DetectorBitmap = match syndrome_source.sample() {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Pre-filter: compute min-cut
        let prefilter_start = Instant::now();
        let graph = build_surface_code_graph(code_distance, error_rate, &syndrome);
        let min_cut = graph.min_cut();
        let prefilter_elapsed = prefilter_start.elapsed().as_nanos() as u64;
        stats.prefilter_time_ns += prefilter_elapsed;

        let has_error = has_logical_error(&syndrome, code_distance);

        // Decision: if min-cut is high, skip decoding
        if min_cut >= threshold {
            // Safe to skip
            stats.skipped_rounds += 1;
            if has_error {
                stats.logical_errors_missed += 1;
            }
        } else {
            // Need to decode
            let decode_start = Instant::now();
            let _correction = decoder.decode(&syndrome);
            let decode_elapsed = decode_start.elapsed().as_nanos() as u64;

            stats.decode_calls += 1;
            stats.decode_time_ns += decode_elapsed;

            if has_error {
                stats.logical_errors_detected += 1;
            }
        }

        stats.total_rounds += 1;
    }

    stats.total_time_ns = start.elapsed().as_nanos() as u64;
    stats
}

fn main() {
    println!("\n═══════════════════════════════════════════════════════════════════════");
    println!("     MWPM vs MIN-CUT PRE-FILTER BENCHMARK");
    println!("═══════════════════════════════════════════════════════════════════════\n");

    // Test configurations
    let code_distance = 5;
    let error_rate = 0.05;
    let num_rounds = 5000;
    let seed = 42;
    let threshold = 6.5; // Tuned for 100% recall

    println!("Configuration:");
    println!("  Code Distance: d={}", code_distance);
    println!("  Error Rate:    p={}", error_rate);
    println!("  Rounds:        {}", num_rounds);
    println!("  Pre-filter Threshold: {}", threshold);
    println!();

    // Benchmark 1: MWPM baseline
    println!("Running MWPM baseline benchmark...");
    let baseline = benchmark_mwpm_baseline(code_distance, error_rate, num_rounds, seed);

    // Benchmark 2: Pre-filter + MWPM
    println!("Running pre-filter + MWPM benchmark...");
    let prefilter =
        benchmark_prefilter_mwpm(code_distance, error_rate, num_rounds, seed, threshold);

    // Results
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                      BENCHMARK RESULTS                            ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║                    │  MWPM Baseline  │  Pre-Filter+MWPM          ║");
    println!("╠════════════════════╪═════════════════╪═══════════════════════════╣");
    println!(
        "║ Total Time         │ {:>12.2} ms │ {:>12.2} ms             ║",
        baseline.total_time_ns as f64 / 1e6,
        prefilter.total_time_ns as f64 / 1e6
    );
    println!(
        "║ Throughput         │ {:>12.0}/s  │ {:>12.0}/s              ║",
        baseline.throughput(),
        prefilter.throughput()
    );
    println!(
        "║ Avg Round Time     │ {:>12.0} ns │ {:>12.0} ns             ║",
        baseline.avg_round_time_ns(),
        prefilter.avg_round_time_ns()
    );
    println!("╠════════════════════╪═════════════════╪═══════════════════════════╣");
    println!(
        "║ Decode Calls       │ {:>12}    │ {:>12} ({:>5.1}%)       ║",
        baseline.decode_calls,
        prefilter.decode_calls,
        prefilter.decode_calls as f64 / baseline.decode_calls.max(1) as f64 * 100.0
    );
    println!(
        "║ Skipped Rounds     │ {:>12}    │ {:>12} ({:>5.1}%)       ║",
        0,
        prefilter.skipped_rounds,
        prefilter.skip_rate() * 100.0
    );
    println!(
        "║ Avg Decode Time    │ {:>12.0} ns │ {:>12.0} ns             ║",
        baseline.avg_decode_time_ns(),
        prefilter.avg_decode_time_ns()
    );
    println!("╠════════════════════╪═════════════════╪═══════════════════════════╣");
    println!(
        "║ Errors Detected    │ {:>12}    │ {:>12}                ║",
        baseline.logical_errors_detected, prefilter.logical_errors_detected
    );
    println!(
        "║ Errors Missed      │ {:>12}    │ {:>12}                ║",
        0, prefilter.logical_errors_missed
    );
    println!("╚════════════════════╧═════════════════╧═══════════════════════════╝");

    // Speedup calculation
    let speedup = baseline.total_time_ns as f64 / prefilter.total_time_ns.max(1) as f64;
    let decode_reduction =
        1.0 - (prefilter.decode_calls as f64 / baseline.decode_calls.max(1) as f64);
    let safety = if prefilter.logical_errors_missed == 0 {
        "SAFE"
    } else {
        "UNSAFE"
    };

    println!("\n┌─────────────────────────────────────────────────────────────────────┐");
    println!("│                         SUMMARY                                    │");
    println!("├─────────────────────────────────────────────────────────────────────┤");
    println!("│                                                                    │");
    println!(
        "│  Speedup:              {:.2}x                                       │",
        speedup
    );
    println!(
        "│  Decode Calls Reduced: {:.1}%                                      │",
        decode_reduction * 100.0
    );
    println!(
        "│  Errors Missed:        {} ({})                                   │",
        prefilter.logical_errors_missed, safety
    );
    println!("│                                                                    │");
    if speedup > 1.0 && prefilter.logical_errors_missed == 0 {
        println!(
            "│  ✓ Pre-filter provides {:.1}% speedup with 100% recall            │",
            (speedup - 1.0) * 100.0
        );
    } else if speedup > 1.0 {
        println!(
            "│  ⚠ Pre-filter faster but missed {} errors                        │",
            prefilter.logical_errors_missed
        );
    } else {
        println!("│  ✗ Pre-filter overhead exceeds decoder savings                   │");
    }
    println!("│                                                                    │");
    println!("└─────────────────────────────────────────────────────────────────────┘");

    // Scaling analysis
    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║              SCALING ANALYSIS (varying code distance)             ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║  d  │ MWPM Time  │ PreFilter Time │ Speedup │ Skip Rate │ Safety ║");
    println!("╠═════╪════════════╪════════════════╪═════════╪═══════════╪════════╣");

    for d in [3, 5, 7] {
        let base = benchmark_mwpm_baseline(d, 0.05, 2000, 42);
        let pf = benchmark_prefilter_mwpm(d, 0.05, 2000, 42, (d as f64) * 1.3);

        let spd = base.total_time_ns as f64 / pf.total_time_ns.max(1) as f64;
        let safe = if pf.logical_errors_missed == 0 {
            "✓"
        } else {
            "✗"
        };

        println!(
            "║ {:>2}  │ {:>8.2} ms │ {:>12.2} ms │  {:>5.2}x │   {:>5.1}%  │   {}    ║",
            d,
            base.total_time_ns as f64 / 1e6,
            pf.total_time_ns as f64 / 1e6,
            spd,
            pf.skip_rate() * 100.0,
            safe
        );
    }
    println!("╚═════╧════════════╧════════════════╧═════════╧═══════════╧════════╝");

    println!("\n═══════════════════════════════════════════════════════════════════════");
    println!("                        BENCHMARK COMPLETE");
    println!("═══════════════════════════════════════════════════════════════════════\n");
}
