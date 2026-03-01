//! ruQu Predictive Evaluation Binary
//!
//! This binary produces formal evaluation metrics for ruQu's predictive capabilities.
//! It demonstrates that ruQu can detect logical failure risk BEFORE it manifests.
//!
//! ## Usage
//!
//! ```bash
//! cargo run --bin ruqu_predictive_eval --release -- \
//!     --distance 5 \
//!     --error-rate 0.001 \
//!     --runs 100
//! ```
//!
//! ## Output
//!
//! Produces DARPA-style evaluation metrics including:
//! - Lead time distribution (median, p10, p90)
//! - Precision and recall
//! - False alarm rate per 100k cycles
//! - Actionability for different mitigation windows

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use ruqu::stim::{StimSyndromeSource, SurfaceCodeConfig};
use ruqu::syndrome::DetectorBitmap;

// ============================================================================
// CONFIGURATION
// ============================================================================

#[derive(Debug, Clone)]
struct EvalConfig {
    code_distance: usize,
    error_rate: f64,
    num_runs: usize,
    cycles_per_run: usize,
    seed: u64,
    inject_mode: InjectMode,
}

#[derive(Debug, Clone, Copy)]
enum InjectMode {
    /// Independent noise only (baseline)
    Independent,
    /// Correlated burst injection
    CorrelatedBurst,
    /// Both modes for comparison
    Both,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            code_distance: 5,
            error_rate: 0.001,
            num_runs: 100,
            cycles_per_run: 500,
            seed: 42,
            inject_mode: InjectMode::CorrelatedBurst,
        }
    }
}

fn parse_args() -> EvalConfig {
    let args: Vec<String> = std::env::args().collect();
    let mut config = EvalConfig::default();

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
            "--runs" | "-r" => {
                i += 1;
                config.num_runs = args[i].parse().expect("Invalid runs");
            }
            "--cycles" | "-c" => {
                i += 1;
                config.cycles_per_run = args[i].parse().expect("Invalid cycles");
            }
            "--seed" | "-s" => {
                i += 1;
                config.seed = args[i].parse().expect("Invalid seed");
            }
            "--inject" => {
                i += 1;
                config.inject_mode = match args[i].as_str() {
                    "independent" => InjectMode::Independent,
                    "burst" | "correlated" | "correlated_burst" => InjectMode::CorrelatedBurst,
                    "both" => InjectMode::Both,
                    _ => panic!("Invalid inject mode: {}", args[i]),
                };
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                print_help();
                std::process::exit(1);
            }
        }
        i += 1;
    }

    config
}

fn print_help() {
    println!("ruQu Predictive Evaluation");
    println!();
    println!("USAGE:");
    println!("    ruqu_predictive_eval [OPTIONS]");
    println!();
    println!("OPTIONS:");
    println!("    -d, --distance <N>      Code distance (default: 5)");
    println!("    -e, --error-rate <F>    Physical error rate (default: 0.001)");
    println!("    -r, --runs <N>          Number of evaluation runs (default: 100)");
    println!("    -c, --cycles <N>        Cycles per run (default: 500)");
    println!("    -s, --seed <N>          Random seed (default: 42)");
    println!("    --inject <MODE>         Injection mode: independent, burst, both");
    println!("    -h, --help              Print this help");
}

// ============================================================================
// STRUCTURAL SIGNAL WITH DYNAMICS
// ============================================================================

/// Structural signal with cut dynamics (velocity and curvature)
#[derive(Debug, Clone, Default)]
pub struct StructuralSignal {
    /// Current min-cut value
    pub cut: f64,
    /// Rate of change (Δλ)
    pub velocity: f64,
    /// Acceleration of change (Δ²λ)
    pub curvature: f64,
    /// Baseline mean for adaptive thresholding
    pub baseline_mean: f64,
    /// Baseline standard deviation
    pub baseline_std: f64,
}

/// Warning detector with velocity and curvature tracking
struct WarningDetector {
    history: VecDeque<f64>,
    velocity_history: VecDeque<f64>,
    max_history: usize,
    warmup_samples: usize,
    baseline_mean: f64,
    baseline_std: f64,
    theta_sigma: f64,
    theta_absolute: f64,
    delta: f64,
    lookback: usize,
    min_event_count: usize,
}

impl WarningDetector {
    fn new() -> Self {
        Self {
            history: VecDeque::new(),
            velocity_history: VecDeque::new(),
            max_history: 100,
            warmup_samples: 20,
            baseline_mean: 0.0,
            baseline_std: 0.0,
            theta_sigma: 2.5,
            theta_absolute: 2.0,
            delta: 1.2,
            lookback: 5,
            min_event_count: 5,
        }
    }

    fn push(&mut self, cut: f64) {
        // Track velocity
        if let Some(&prev) = self.history.back() {
            let velocity = cut - prev;
            self.velocity_history.push_back(velocity);
            if self.velocity_history.len() > self.max_history {
                self.velocity_history.pop_front();
            }
        }

        self.history.push_back(cut);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        // Update baseline during warmup
        if self.history.len() <= self.warmup_samples {
            let sum: f64 = self.history.iter().sum();
            self.baseline_mean = sum / self.history.len() as f64;

            if self.history.len() > 1 {
                let variance: f64 = self
                    .history
                    .iter()
                    .map(|x| (x - self.baseline_mean).powi(2))
                    .sum::<f64>()
                    / (self.history.len() - 1) as f64;
                self.baseline_std = variance.sqrt();
            }
        }
    }

    fn current(&self) -> f64 {
        self.history.back().copied().unwrap_or(0.0)
    }

    fn velocity(&self) -> f64 {
        self.velocity_history.back().copied().unwrap_or(0.0)
    }

    fn curvature(&self) -> f64 {
        if self.velocity_history.len() < 2 {
            return 0.0;
        }
        let n = self.velocity_history.len();
        self.velocity_history[n - 1] - self.velocity_history[n - 2]
    }

    fn signal(&self) -> StructuralSignal {
        StructuralSignal {
            cut: self.current(),
            velocity: self.velocity(),
            curvature: self.curvature(),
            baseline_mean: self.baseline_mean,
            baseline_std: self.baseline_std,
        }
    }

    fn drop_from_lookback(&self) -> f64 {
        if self.history.len() <= self.lookback {
            return 0.0;
        }
        let n = self.history.len();
        self.history[n - 1] - self.history[n - 1 - self.lookback]
    }

    fn is_warning(&self, event_count: usize) -> bool {
        if self.history.len() < self.warmup_samples {
            return false;
        }
        if self.baseline_mean == 0.0 {
            return false;
        }

        let adaptive_threshold =
            (self.baseline_mean - self.theta_sigma * self.baseline_std).max(0.5);

        let below_adaptive = self.current() <= adaptive_threshold;
        let below_absolute = self.current() <= self.theta_absolute;
        let rapid_drop = self.drop_from_lookback() <= -self.delta;
        let high_events = event_count >= self.min_event_count;

        // AND mode: structural + drop + intensity
        (below_adaptive || below_absolute) && rapid_drop && high_events
    }
}

// ============================================================================
// QEC GRAPH CONSTRUCTION
// ============================================================================

struct STMinCutGraph {
    num_nodes: u32,
    edges: Vec<(u32, u32, f64)>,
    source_edges: Vec<(u32, f64)>,
    sink_edges: Vec<(u32, f64)>,
}

impl STMinCutGraph {
    fn new(num_nodes: u32) -> Self {
        Self {
            num_nodes,
            edges: Vec::new(),
            source_edges: Vec::new(),
            sink_edges: Vec::new(),
        }
    }

    fn add_edge(&mut self, u: u32, v: u32, weight: f64) {
        self.edges.push((u, v, weight));
    }

    fn connect_source(&mut self, node: u32, weight: f64) {
        self.source_edges.push((node, weight));
    }

    fn connect_sink(&mut self, node: u32, weight: f64) {
        self.sink_edges.push((node, weight));
    }

    fn compute_min_cut(&self) -> f64 {
        // BFS-based approximation
        let mut visited = vec![false; self.num_nodes as usize];
        let mut queue = VecDeque::new();
        let mut total_flow = 0.0;

        // Build adjacency with capacities
        let mut adj: HashMap<u32, Vec<(u32, f64)>> = HashMap::new();
        for &(u, v, w) in &self.edges {
            adj.entry(u).or_default().push((v, w));
            adj.entry(v).or_default().push((u, w));
        }

        // Start from source-connected nodes
        for &(node, cap) in &self.source_edges {
            if !visited[node as usize] {
                queue.push_back((node, cap));
                visited[node as usize] = true;
            }
        }

        // BFS to sink
        let sink_set: HashSet<u32> = self.sink_edges.iter().map(|(n, _)| *n).collect();

        while let Some((current, flow)) = queue.pop_front() {
            if sink_set.contains(&current) {
                total_flow += flow;
                continue;
            }

            if let Some(neighbors) = adj.get(&current) {
                for &(next, cap) in neighbors {
                    if !visited[next as usize] {
                        visited[next as usize] = true;
                        let next_flow = flow.min(cap);
                        queue.push_back((next, next_flow));
                    }
                }
            }
        }

        // Return cut value (inverse of flow for this approximation)
        let source_capacity: f64 = self.source_edges.iter().map(|(_, c)| c).sum();
        (source_capacity - total_flow).max(0.1)
    }
}

fn build_qec_graph(
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

    // Build X-stabilizer grid
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
        let left = (row * grid_size) as u32;
        let right = (row * grid_size + grid_size - 1) as u32;
        graph.connect_source(left, boundary_weight);
        graph.connect_sink(right, boundary_weight);
    }

    graph
}

// ============================================================================
// GROUND TRUTH
// ============================================================================

fn is_logical_failure(syndrome: &DetectorBitmap, code_distance: usize) -> bool {
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

// ============================================================================
// EVALUATION RESULTS
// ============================================================================

#[derive(Default)]
struct EvalResults {
    total_cycles: u64,
    failures_observed: u64,
    warnings_issued: u64,
    true_warnings: u64,
    false_warnings: u64,
    lead_times: Vec<u64>,
}

impl EvalResults {
    fn precision(&self) -> f64 {
        if self.warnings_issued == 0 {
            return 0.0;
        }
        self.true_warnings as f64 / self.warnings_issued as f64
    }

    fn recall(&self) -> f64 {
        if self.failures_observed == 0 {
            return 0.0;
        }
        self.true_warnings as f64 / self.failures_observed as f64
    }

    fn false_alarms_per_100k(&self) -> f64 {
        if self.total_cycles == 0 {
            return 0.0;
        }
        self.false_warnings as f64 / self.total_cycles as f64 * 100_000.0
    }

    fn median_lead_time(&self) -> f64 {
        if self.lead_times.is_empty() {
            return 0.0;
        }
        let mut sorted = self.lead_times.clone();
        sorted.sort();
        sorted[sorted.len() / 2] as f64
    }

    fn p10_lead_time(&self) -> f64 {
        if self.lead_times.is_empty() {
            return 0.0;
        }
        let mut sorted = self.lead_times.clone();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.10) as usize;
        sorted[idx.min(sorted.len() - 1)] as f64
    }

    fn p90_lead_time(&self) -> f64 {
        if self.lead_times.is_empty() {
            return 0.0;
        }
        let mut sorted = self.lead_times.clone();
        sorted.sort();
        let idx = (sorted.len() as f64 * 0.90) as usize;
        sorted[idx.min(sorted.len() - 1)] as f64
    }

    fn actionable_rate(&self, min_cycles: u64) -> f64 {
        if self.lead_times.is_empty() {
            return 0.0;
        }
        let actionable = self.lead_times.iter().filter(|&&t| t >= min_cycles).count();
        actionable as f64 / self.lead_times.len() as f64
    }
}

// ============================================================================
// SYNDROME GENERATOR WITH BURST INJECTION
// ============================================================================

struct SyndromeGenerator {
    source: StimSyndromeSource,
    burst_active: bool,
    burst_remaining: usize,
    burst_center: usize,
    burst_radius: usize,
    code_distance: usize,
}

impl SyndromeGenerator {
    fn new(code_distance: usize, error_rate: f64, seed: u64) -> Self {
        let config = SurfaceCodeConfig {
            distance: code_distance,
            error_rate,
            seed: Some(seed),
            rounds: 1,
            rotated: false,
            measure_errors: true,
        };
        Self {
            source: StimSyndromeSource::new(config).expect("Failed to create source"),
            burst_active: false,
            burst_remaining: 0,
            burst_center: 0,
            burst_radius: 2,
            code_distance,
        }
    }

    fn inject_burst(&mut self, duration: usize, center: usize) {
        self.burst_active = true;
        self.burst_remaining = duration;
        self.burst_center = center;
    }

    fn sample(&mut self) -> DetectorBitmap {
        let mut syndrome = self.source.sample().unwrap_or_else(|_| {
            DetectorBitmap::new(2 * (self.code_distance - 1) * (self.code_distance - 1))
        });

        if self.burst_active && self.burst_remaining > 0 {
            let grid_size = self.code_distance - 1;
            let center_row = self.burst_center / grid_size;
            let center_col = self.burst_center % grid_size;

            for dr in 0..=self.burst_radius {
                for dc in 0..=self.burst_radius {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    for &(sr, sc) in &[(1i32, 1i32), (1, -1), (-1, 1), (-1, -1)] {
                        let row = center_row as i32 + dr as i32 * sr;
                        let col = center_col as i32 + dc as i32 * sc;
                        if row >= 0 && row < grid_size as i32 && col >= 0 && col < grid_size as i32
                        {
                            let detector = (row as usize) * grid_size + (col as usize);
                            if detector < syndrome.detector_count() {
                                syndrome.set(detector, true);
                            }
                        }
                    }
                }
            }

            if self.burst_center < syndrome.detector_count() {
                syndrome.set(self.burst_center, true);
            }

            self.burst_remaining -= 1;
            if self.burst_remaining == 0 {
                self.burst_active = false;
            }
        }

        syndrome
    }
}

// ============================================================================
// MAIN EVALUATION
// ============================================================================

fn run_evaluation(config: &EvalConfig, with_bursts: bool) -> EvalResults {
    let mut results = EvalResults::default();
    let grid_size = config.code_distance - 1;
    let num_detectors = 2 * grid_size * grid_size;

    for run in 0..config.num_runs {
        let seed = config.seed + run as u64;
        let mut generator = SyndromeGenerator::new(config.code_distance, config.error_rate, seed);
        let mut detector = WarningDetector::new();

        let mut warning_active = false;
        let mut warning_start = 0u64;
        let mut cycles_since_warning = 0u64;

        // Schedule burst injection at random point
        let burst_cycle = if with_bursts {
            (seed % (config.cycles_per_run as u64 / 2)) as usize + config.cycles_per_run / 4
        } else {
            usize::MAX
        };
        let burst_duration = 8;
        let burst_center = ((seed * 7) % num_detectors as u64) as usize;

        for cycle in 0..config.cycles_per_run {
            // Inject burst at scheduled time
            if cycle == burst_cycle && with_bursts {
                generator.inject_burst(burst_duration, burst_center);
            }

            let syndrome = generator.sample();
            let graph = build_qec_graph(config.code_distance, config.error_rate, &syndrome);
            let cut = graph.compute_min_cut();
            let event_count = syndrome.fired_count();

            detector.push(cut);

            let is_failure = is_logical_failure(&syndrome, config.code_distance);
            let is_warning = detector.is_warning(event_count);

            // Track warning onset
            if is_warning && !warning_active {
                warning_active = true;
                warning_start = cycle as u64;
                cycles_since_warning = 0;
                results.warnings_issued += 1;
            }

            if warning_active {
                cycles_since_warning += 1;
            }

            // Track failures
            if is_failure {
                results.failures_observed += 1;

                if warning_active && cycles_since_warning > 0 {
                    results.true_warnings += 1;
                    results.lead_times.push(cycles_since_warning);
                }

                // Reset warning state after failure
                warning_active = false;
            }

            // Timeout warnings without failure (false alarm)
            if warning_active && cycles_since_warning > 20 {
                results.false_warnings += 1;
                warning_active = false;
            }

            results.total_cycles += 1;
        }
    }

    results
}

fn main() {
    let config = parse_args();
    let start_time = Instant::now();

    println!();
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║                    ruQu PREDICTIVE EVALUATION                         ║");
    println!("║                  Formal Metrics for Early Warning                     ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");

    println!();
    println!("Configuration:");
    println!("  Code Distance:  d={}", config.code_distance);
    println!("  Error Rate:     {:.4}", config.error_rate);
    println!("  Runs:           {}", config.num_runs);
    println!("  Cycles/Run:     {}", config.cycles_per_run);
    println!("  Seed:           {}", config.seed);
    println!("  Inject Mode:    {:?}", config.inject_mode);

    // Run with correlated bursts
    let results = match config.inject_mode {
        InjectMode::Independent => run_evaluation(&config, false),
        InjectMode::CorrelatedBurst => run_evaluation(&config, true),
        InjectMode::Both => {
            println!();
            println!("═══════════════════════════════════════════════════════════════════════");
            println!("                    REGIME A: Independent Noise");
            println!("═══════════════════════════════════════════════════════════════════════");
            let independent = run_evaluation(&config, false);
            print_results(&independent);

            println!();
            println!("═══════════════════════════════════════════════════════════════════════");
            println!("                    REGIME B: Correlated Bursts");
            println!("═══════════════════════════════════════════════════════════════════════");
            let bursts = run_evaluation(&config, true);
            print_results(&bursts);

            bursts
        }
    };

    if !matches!(config.inject_mode, InjectMode::Both) {
        println!();
        println!("═══════════════════════════════════════════════════════════════════════");
        println!("                         EVALUATION RESULTS");
        println!("═══════════════════════════════════════════════════════════════════════");
        print_results(&results);
    }

    // Actionability breakdown
    println!();
    println!("═══════════════════════════════════════════════════════════════════════");
    println!("                           ACTIONABILITY");
    println!("═══════════════════════════════════════════════════════════════════════");
    println!();
    println!(
        "  Decoder switch (1 cycle):         {:>5.1}%",
        results.actionable_rate(1) * 100.0
    );
    println!(
        "  Extra syndrome round (2 cycles):  {:>5.1}%",
        results.actionable_rate(2) * 100.0
    );
    println!(
        "  Region quarantine (5 cycles):     {:>5.1}%",
        results.actionable_rate(5) * 100.0
    );
    println!(
        "  Full recalibration (10 cycles):   {:>5.1}%",
        results.actionable_rate(10) * 100.0
    );

    // Summary
    println!();
    println!("═══════════════════════════════════════════════════════════════════════");
    println!("                             SUMMARY");
    println!("═══════════════════════════════════════════════════════════════════════");

    let predictive = results.recall() >= 0.80
        && results.false_alarms_per_100k() < 50.0
        && results.median_lead_time() >= 2.0;

    if predictive {
        println!();
        println!("  ✓ PREDICTIVE: ruQu satisfies all criteria");
        println!("    - Recall >= 80%: {:.1}%", results.recall() * 100.0);
        println!(
            "    - False alarms < 50/100k: {:.1}/100k",
            results.false_alarms_per_100k()
        );
        println!(
            "    - Median lead >= 2 cycles: {:.1} cycles",
            results.median_lead_time()
        );
    } else {
        println!();
        println!("  ~ PARTIAL: Some criteria not met");
        println!(
            "    - Recall: {:.1}% (target: >=80%)",
            results.recall() * 100.0
        );
        println!(
            "    - False alarms: {:.1}/100k (target: <50)",
            results.false_alarms_per_100k()
        );
        println!(
            "    - Median lead: {:.1} cycles (target: >=2)",
            results.median_lead_time()
        );
    }

    let elapsed = start_time.elapsed();
    println!();
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!(
        "  Throughput: {:.0} cycles/sec",
        results.total_cycles as f64 / elapsed.as_secs_f64()
    );
    println!();
}

fn print_results(results: &EvalResults) {
    println!();
    println!("Failures observed:  {}", results.failures_observed);
    println!("Warnings issued:    {}", results.warnings_issued);
    println!("True warnings:      {}", results.true_warnings);
    println!("False warnings:     {}", results.false_warnings);
    println!();
    println!("Lead time (cycles):");
    println!("  median:  {:.1}", results.median_lead_time());
    println!("  p10:     {:.1}", results.p10_lead_time());
    println!("  p90:     {:.1}", results.p90_lead_time());
    println!();
    println!("Precision:     {:.2}", results.precision());
    println!("Recall:        {:.2}", results.recall());
    println!(
        "False alarms:  {:.1} / 100k cycles",
        results.false_alarms_per_100k()
    );
}
