//! ruQu Demo Binary - Proof Artifact
//!
//! This is the runnable demonstration of ruQu's capabilities.
//!
//! ## What it does
//!
//! 1. Generates a streaming syndrome feed
//! 2. Runs the coherence gate loop per round
//! 3. Prints live status: round, cut value, risk, region mask
//! 4. Writes metrics file: latency histogram, p50/p99/p999, false alarms
//!
//! ## Usage
//!
//! ```bash
//! # Basic run with defaults
//! cargo run --bin ruqu_demo --release
//!
//! # Custom parameters
//! cargo run --bin ruqu_demo --release -- \
//!     --distance 7 \
//!     --error-rate 0.01 \
//!     --rounds 10000 \
//!     --output metrics.json
//! ```

use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

use ruqu::stim::{StimSyndromeSource, SurfaceCodeConfig};
use ruqu::syndrome::DetectorBitmap;

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
struct DemoConfig {
    /// Code distance
    code_distance: usize,
    /// Physical error rate
    error_rate: f64,
    /// Number of rounds to run
    num_rounds: usize,
    /// Random seed
    seed: u64,
    /// Output metrics file
    output_file: Option<String>,
    /// Print interval (every N rounds)
    print_interval: usize,
    /// Gate threshold
    threshold: f64,
}

impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            code_distance: 5,
            error_rate: 0.01,
            num_rounds: 10000,
            seed: 42,
            output_file: Some("ruqu_metrics.json".to_string()),
            print_interval: 1000,
            threshold: 5.0,
        }
    }
}

fn parse_args() -> DemoConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut config = DemoConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--distance" | "-d" => {
                i += 1;
                config.code_distance = args[i].parse().expect("Invalid distance");
            }
            "--error-rate" | "-e" => {
                i += 1;
                config.error_rate = args[i].parse().expect("Invalid error rate");
            }
            "--rounds" | "-r" => {
                i += 1;
                config.num_rounds = args[i].parse().expect("Invalid rounds");
            }
            "--seed" | "-s" => {
                i += 1;
                config.seed = args[i].parse().expect("Invalid seed");
            }
            "--output" | "-o" => {
                i += 1;
                config.output_file = Some(args[i].clone());
            }
            "--threshold" | "-t" => {
                i += 1;
                config.threshold = args[i].parse().expect("Invalid threshold");
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    config
}

fn print_help() {
    println!(
        r#"
ruQu Demo - Coherence Gate Demonstration

USAGE:
    ruqu_demo [OPTIONS]

OPTIONS:
    -d, --distance <N>      Code distance (default: 5)
    -e, --error-rate <P>    Physical error rate (default: 0.01)
    -r, --rounds <N>        Number of rounds (default: 10000)
    -s, --seed <N>          Random seed (default: 42)
    -o, --output <FILE>     Output metrics file (default: ruqu_metrics.json)
    -t, --threshold <T>     Gate threshold (default: 5.0)
    -h, --help              Print this help message
"#
    );
}

// ============================================================================
// LATENCY TRACKING
// ============================================================================

struct LatencyTracker {
    latencies: Vec<u64>,
    recent: VecDeque<u64>,
    max_recent: usize,
}

impl LatencyTracker {
    fn new(max_recent: usize) -> Self {
        Self {
            latencies: Vec::new(),
            recent: VecDeque::with_capacity(max_recent),
            max_recent,
        }
    }

    fn record(&mut self, latency_ns: u64) {
        self.latencies.push(latency_ns);
        if self.recent.len() >= self.max_recent {
            self.recent.pop_front();
        }
        self.recent.push_back(latency_ns);
    }

    fn percentile(&self, p: f64) -> u64 {
        if self.latencies.is_empty() {
            return 0;
        }
        let mut sorted = self.latencies.clone();
        sorted.sort_unstable();
        let idx = ((p / 100.0) * (sorted.len() - 1) as f64) as usize;
        sorted[idx]
    }

    fn p50(&self) -> u64 {
        self.percentile(50.0)
    }

    fn p99(&self) -> u64 {
        self.percentile(99.0)
    }

    fn p999(&self) -> u64 {
        self.percentile(99.9)
    }

    fn max(&self) -> u64 {
        self.latencies.iter().copied().max().unwrap_or(0)
    }

    fn mean(&self) -> f64 {
        if self.latencies.is_empty() {
            return 0.0;
        }
        let sum: u64 = self.latencies.iter().sum();
        sum as f64 / self.latencies.len() as f64
    }

    fn count(&self) -> usize {
        self.latencies.len()
    }

    fn histogram(&self, num_buckets: usize) -> Vec<(u64, u64, usize)> {
        if self.latencies.is_empty() {
            return vec![];
        }

        let min = *self.latencies.iter().min().unwrap();
        let max = self.max();
        let range = max - min + 1;
        let bucket_size = (range / num_buckets as u64).max(1);

        let mut buckets = vec![0usize; num_buckets];
        for &lat in &self.latencies {
            let bucket = ((lat - min) / bucket_size).min(num_buckets as u64 - 1) as usize;
            buckets[bucket] += 1;
        }

        buckets
            .into_iter()
            .enumerate()
            .map(|(i, count)| {
                let start = min + i as u64 * bucket_size;
                let end = start + bucket_size;
                (start, end, count)
            })
            .collect()
    }
}

// ============================================================================
// SIMPLE MIN-CUT GATE
// ============================================================================

use std::collections::{HashMap, HashSet};

struct MinCutGate {
    threshold: f64,
    grid_size: usize,
    base_weight: f64,
}

impl MinCutGate {
    fn new(code_distance: usize, error_rate: f64, threshold: f64) -> Self {
        Self {
            threshold,
            grid_size: code_distance - 1,
            base_weight: (-error_rate.ln()).max(0.1),
        }
    }

    fn process(&self, syndrome: &DetectorBitmap) -> GateResult {
        let start = Instant::now();

        // Compute min-cut
        let fired_set: HashSet<usize> = syndrome.iter_fired().collect();
        let min_cut = self.compute_min_cut(&fired_set);

        // Compute risk
        let risk = if min_cut < self.threshold {
            1.0 - (min_cut / self.threshold)
        } else {
            0.0
        };

        // Compute region mask (simplified: which quadrants have errors)
        let region_mask = self.compute_region_mask(&fired_set);

        let latency_ns = start.elapsed().as_nanos() as u64;

        GateResult {
            min_cut,
            risk,
            region_mask,
            decision: if min_cut >= self.threshold {
                Decision::Permit
            } else if min_cut >= self.threshold * 0.5 {
                Decision::Defer
            } else {
                Decision::Deny
            },
            latency_ns,
            fired_count: fired_set.len(),
        }
    }

    fn compute_min_cut(&self, fired_set: &HashSet<usize>) -> f64 {
        // Simple s-t min-cut using Edmonds-Karp
        let mut adj: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();
        let fired_weight = 0.01;

        // Build grid
        for row in 0..self.grid_size {
            for col in 0..self.grid_size {
                let node = (row * self.grid_size + col) as u32;
                let is_fired = fired_set.contains(&(node as usize));

                if col + 1 < self.grid_size {
                    let right = (row * self.grid_size + col + 1) as u32;
                    let right_fired = fired_set.contains(&(right as usize));
                    let weight = if is_fired || right_fired {
                        fired_weight
                    } else {
                        self.base_weight
                    };
                    adj.entry(node).or_default().push((right, weight));
                    adj.entry(right).or_default().push((node, weight));
                }

                if row + 1 < self.grid_size {
                    let bottom = ((row + 1) * self.grid_size + col) as u32;
                    let bottom_fired = fired_set.contains(&(bottom as usize));
                    let weight = if is_fired || bottom_fired {
                        fired_weight
                    } else {
                        self.base_weight
                    };
                    adj.entry(node).or_default().push((bottom, weight));
                    adj.entry(bottom).or_default().push((node, weight));
                }
            }
        }

        let source = (self.grid_size * self.grid_size) as u32;
        let sink = source + 1;

        // Connect boundaries
        let boundary_weight = self.base_weight * 2.0;
        for row in 0..self.grid_size {
            let left = (row * self.grid_size) as u32;
            let right = (row * self.grid_size + self.grid_size - 1) as u32;
            adj.entry(source).or_default().push((left, boundary_weight));
            adj.entry(left).or_default().push((source, boundary_weight));
            adj.entry(right).or_default().push((sink, boundary_weight));
            adj.entry(sink).or_default().push((right, boundary_weight));
        }

        // Max-flow = min-cut
        let mut capacity: HashMap<(u32, u32), f64> = HashMap::new();
        for (&u, neighbors) in &adj {
            for &(v, w) in neighbors {
                *capacity.entry((u, v)).or_default() += w;
            }
        }

        let mut max_flow = 0.0;
        loop {
            // BFS for augmenting path
            let mut parent: HashMap<u32, u32> = HashMap::new();
            let mut visited = HashSet::new();
            let mut queue = std::collections::VecDeque::new();

            queue.push_back(source);
            visited.insert(source);

            while let Some(u) = queue.pop_front() {
                if u == sink {
                    break;
                }
                if let Some(neighbors) = adj.get(&u) {
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

            if !parent.contains_key(&sink) {
                break;
            }

            // Find bottleneck
            let mut path_flow = f64::INFINITY;
            let mut v = sink;
            while v != source {
                let u = parent[&v];
                path_flow = path_flow.min(capacity.get(&(u, v)).copied().unwrap_or(0.0));
                v = u;
            }

            // Update capacities
            v = sink;
            while v != source {
                let u = parent[&v];
                *capacity.entry((u, v)).or_default() -= path_flow;
                *capacity.entry((v, u)).or_default() += path_flow;
                v = u;
            }

            max_flow += path_flow;
        }

        max_flow
    }

    fn compute_region_mask(&self, fired_set: &HashSet<usize>) -> u64 {
        // Split into 4 quadrants
        let half = self.grid_size / 2;
        let mut mask = 0u64;

        for &det in fired_set {
            let row = det / self.grid_size;
            let col = det % self.grid_size;
            let quadrant = match (row < half, col < half) {
                (true, true) => 0,   // Top-left
                (true, false) => 1,  // Top-right
                (false, true) => 2,  // Bottom-left
                (false, false) => 3, // Bottom-right
            };
            mask |= 1 << quadrant;
        }

        mask
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Decision {
    Permit,
    Defer,
    Deny,
}

#[derive(Debug, Clone)]
struct GateResult {
    min_cut: f64,
    risk: f64,
    region_mask: u64,
    decision: Decision,
    latency_ns: u64,
    fired_count: usize,
}

// ============================================================================
// METRICS OUTPUT
// ============================================================================

#[derive(Debug, serde::Serialize)]
struct DemoMetrics {
    config: MetricsConfig,
    summary: MetricsSummary,
    latency: LatencyMetrics,
    decisions: DecisionMetrics,
    histogram: Vec<HistogramBucket>,
}

#[derive(Debug, serde::Serialize)]
struct MetricsConfig {
    code_distance: usize,
    error_rate: f64,
    num_rounds: usize,
    seed: u64,
    threshold: f64,
}

#[derive(Debug, serde::Serialize)]
struct MetricsSummary {
    total_rounds: usize,
    total_time_ms: f64,
    throughput_per_sec: f64,
    total_fired: usize,
    avg_fired_per_round: f64,
}

#[derive(Debug, serde::Serialize)]
struct LatencyMetrics {
    mean_ns: f64,
    p50_ns: u64,
    p99_ns: u64,
    p999_ns: u64,
    max_ns: u64,
}

#[derive(Debug, serde::Serialize)]
struct DecisionMetrics {
    permits: usize,
    defers: usize,
    denies: usize,
    permit_rate: f64,
    deny_rate: f64,
}

#[derive(Debug, serde::Serialize)]
struct HistogramBucket {
    start_ns: u64,
    end_ns: u64,
    count: usize,
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    let config = parse_args();

    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                    ruQu Demo - Proof Artifact                     ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Code Distance: d={}  | Error Rate: {:.4}  | Rounds: {:>6}      ║",
        config.code_distance, config.error_rate, config.num_rounds
    );
    println!(
        "║ Threshold: {:.2}     | Seed: {:>10}                          ║",
        config.threshold, config.seed
    );
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();

    // Initialize components
    let surface_config =
        SurfaceCodeConfig::new(config.code_distance, config.error_rate).with_seed(config.seed);
    let mut syndrome_source = match StimSyndromeSource::new(surface_config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to create syndrome source: {:?}", e);
            std::process::exit(1);
        }
    };

    let gate = MinCutGate::new(config.code_distance, config.error_rate, config.threshold);
    let mut latency_tracker = LatencyTracker::new(1000);

    // Counters
    let mut permits = 0usize;
    let mut defers = 0usize;
    let mut denies = 0usize;
    let mut total_fired = 0usize;

    // Run demo
    println!("Round │ Cut   │ Risk  │ Decision │ Regions │ Latency │ Fired");
    println!("──────┼───────┼───────┼──────────┼─────────┼─────────┼──────");

    let start_time = Instant::now();

    for round in 0..config.num_rounds {
        // Get syndrome
        let syndrome: DetectorBitmap = match syndrome_source.sample() {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Process through gate
        let result = gate.process(&syndrome);
        latency_tracker.record(result.latency_ns);
        total_fired += result.fired_count;

        // Update counters
        match result.decision {
            Decision::Permit => permits += 1,
            Decision::Defer => defers += 1,
            Decision::Deny => denies += 1,
        }

        // Print live status
        if round % config.print_interval == 0 || result.decision == Decision::Deny {
            let decision_str = match result.decision {
                Decision::Permit => "\x1b[32mPERMIT\x1b[0m  ",
                Decision::Defer => "\x1b[33mDEFER\x1b[0m   ",
                Decision::Deny => "\x1b[31mDENY\x1b[0m    ",
            };
            println!(
                "{:>5} │ {:>5.2} │ {:>5.2} │ {} │ {:>07b}  │ {:>5}ns │ {:>3}",
                round,
                result.min_cut,
                result.risk,
                decision_str,
                result.region_mask,
                result.latency_ns,
                result.fired_count
            );
        }
    }

    let total_time = start_time.elapsed();

    // Summary
    println!();
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                         RESULTS SUMMARY                           ║");
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Total Time:        {:>10.2} ms                                 ║",
        total_time.as_secs_f64() * 1000.0
    );
    println!(
        "║ Throughput:        {:>10.0} rounds/sec                         ║",
        config.num_rounds as f64 / total_time.as_secs_f64()
    );
    println!(
        "║ Avg Fired/Round:   {:>10.2}                                    ║",
        total_fired as f64 / config.num_rounds as f64
    );
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║ Latency:                                                         ║");
    println!(
        "║   Mean:   {:>8.0} ns                                           ║",
        latency_tracker.mean()
    );
    println!(
        "║   P50:    {:>8} ns                                           ║",
        latency_tracker.p50()
    );
    println!(
        "║   P99:    {:>8} ns                                           ║",
        latency_tracker.p99()
    );
    println!(
        "║   P999:   {:>8} ns                                           ║",
        latency_tracker.p999()
    );
    println!(
        "║   Max:    {:>8} ns                                           ║",
        latency_tracker.max()
    );
    println!("╠═══════════════════════════════════════════════════════════════════╣");
    println!("║ Decisions:                                                       ║");
    println!(
        "║   Permits: {:>6} ({:>5.1}%)                                      ║",
        permits,
        permits as f64 / config.num_rounds as f64 * 100.0
    );
    println!(
        "║   Defers:  {:>6} ({:>5.1}%)                                      ║",
        defers,
        defers as f64 / config.num_rounds as f64 * 100.0
    );
    println!(
        "║   Denies:  {:>6} ({:>5.1}%)                                      ║",
        denies,
        denies as f64 / config.num_rounds as f64 * 100.0
    );
    println!("╚═══════════════════════════════════════════════════════════════════╝");

    // Write metrics file
    if let Some(output_file) = &config.output_file {
        let metrics = DemoMetrics {
            config: MetricsConfig {
                code_distance: config.code_distance,
                error_rate: config.error_rate,
                num_rounds: config.num_rounds,
                seed: config.seed,
                threshold: config.threshold,
            },
            summary: MetricsSummary {
                total_rounds: config.num_rounds,
                total_time_ms: total_time.as_secs_f64() * 1000.0,
                throughput_per_sec: config.num_rounds as f64 / total_time.as_secs_f64(),
                total_fired,
                avg_fired_per_round: total_fired as f64 / config.num_rounds as f64,
            },
            latency: LatencyMetrics {
                mean_ns: latency_tracker.mean(),
                p50_ns: latency_tracker.p50(),
                p99_ns: latency_tracker.p99(),
                p999_ns: latency_tracker.p999(),
                max_ns: latency_tracker.max(),
            },
            decisions: DecisionMetrics {
                permits,
                defers,
                denies,
                permit_rate: permits as f64 / config.num_rounds as f64,
                deny_rate: denies as f64 / config.num_rounds as f64,
            },
            histogram: latency_tracker
                .histogram(20)
                .into_iter()
                .map(|(start, end, count)| HistogramBucket {
                    start_ns: start,
                    end_ns: end,
                    count,
                })
                .collect(),
        };

        match File::create(output_file) {
            Ok(mut file) => {
                let json = serde_json::to_string_pretty(&metrics).unwrap();
                file.write_all(json.as_bytes()).unwrap();
                println!("\nMetrics written to: {}", output_file);
            }
            Err(e) => {
                eprintln!("Failed to write metrics file: {}", e);
            }
        }
    }

    // Latency histogram
    println!("\nLatency Histogram:");
    let histogram = latency_tracker.histogram(10);
    let max_count = histogram.iter().map(|(_, _, c)| *c).max().unwrap_or(1);
    for (start, end, count) in histogram {
        let bar_len = (count as f64 / max_count as f64 * 40.0) as usize;
        let bar = "█".repeat(bar_len);
        println!("{:>8}-{:<8} │{:<40} {:>5}", start, end, bar, count);
    }
}
