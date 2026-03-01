//! Full Coherence Gate Simulation
//!
//! This example simulates a complete quantum error correction cycle with:
//! - 256-tile WASM fabric processing syndromes
//! - Real SubpolynomialMinCut for structural analysis
//! - Three-filter decision pipeline
//! - Ed25519 signed permit tokens
//!
//! Run with: cargo run --example coherence_simulation --features "structural" --release

use std::time::{Duration, Instant};

use ruqu::{
    syndrome::DetectorBitmap,
    tile::{GateDecision, GateThresholds, SyndromeDelta, TileReport, TileZero, WorkerTile},
};

#[cfg(feature = "structural")]
use ruqu::mincut::DynamicMinCutEngine;

/// Simulation configuration
struct SimConfig {
    /// Number of worker tiles (max 255)
    num_tiles: usize,
    /// Number of syndrome rounds to simulate
    num_rounds: usize,
    /// Surface code distance (affects graph size)
    code_distance: usize,
    /// Error rate for syndrome generation
    error_rate: f64,
    /// Whether to use real min-cut
    use_real_mincut: bool,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            num_tiles: 64,
            num_rounds: 1000,
            code_distance: 5,
            error_rate: 0.01,
            use_real_mincut: true,
        }
    }
}

/// Statistics collected during simulation
#[derive(Default)]
struct SimStats {
    total_ticks: u64,
    total_decisions: u64,
    permits: u64,
    defers: u64,
    denies: u64,
    tick_times: Vec<Duration>,
    merge_times: Vec<Duration>,
    mincut_times: Vec<Duration>,
}

impl SimStats {
    fn report(&self) {
        println!("\n=== Simulation Statistics ===");
        println!("Total ticks: {}", self.total_ticks);
        println!("Total decisions: {}", self.total_decisions);
        println!(
            "  Permits: {} ({:.1}%)",
            self.permits,
            100.0 * self.permits as f64 / self.total_decisions as f64
        );
        println!(
            "  Defers:  {} ({:.1}%)",
            self.defers,
            100.0 * self.defers as f64 / self.total_decisions as f64
        );
        println!(
            "  Denies:  {} ({:.1}%)",
            self.denies,
            100.0 * self.denies as f64 / self.total_decisions as f64
        );

        if !self.tick_times.is_empty() {
            let tick_ns: Vec<u64> = self
                .tick_times
                .iter()
                .map(|d| d.as_nanos() as u64)
                .collect();
            let avg_tick = tick_ns.iter().sum::<u64>() / tick_ns.len() as u64;
            let max_tick = *tick_ns.iter().max().unwrap();
            let mut sorted = tick_ns.clone();
            sorted.sort();
            let p99_tick = sorted[sorted.len() * 99 / 100];

            println!("\nTick latency:");
            println!("  Average: {} ns", avg_tick);
            println!("  P99:     {} ns", p99_tick);
            println!("  Max:     {} ns", max_tick);
        }

        if !self.merge_times.is_empty() {
            let merge_ns: Vec<u64> = self
                .merge_times
                .iter()
                .map(|d| d.as_nanos() as u64)
                .collect();
            let avg_merge = merge_ns.iter().sum::<u64>() / merge_ns.len() as u64;
            let max_merge = *merge_ns.iter().max().unwrap();
            let mut sorted = merge_ns.clone();
            sorted.sort();
            let p99_merge = sorted[sorted.len() * 99 / 100];

            println!("\nMerge latency (TileZero):");
            println!("  Average: {} ns", avg_merge);
            println!("  P99:     {} ns", p99_merge);
            println!("  Max:     {} ns", max_merge);
        }

        #[cfg(feature = "structural")]
        if !self.mincut_times.is_empty() {
            let mincut_ns: Vec<u64> = self
                .mincut_times
                .iter()
                .map(|d| d.as_nanos() as u64)
                .collect();
            let avg_mincut = mincut_ns.iter().sum::<u64>() / mincut_ns.len() as u64;
            let max_mincut = *mincut_ns.iter().max().unwrap();
            let mut sorted = mincut_ns.clone();
            sorted.sort();
            let p99_mincut = sorted[sorted.len() * 99 / 100];

            println!("\nMin-cut query latency:");
            println!("  Average: {} ns", avg_mincut);
            println!("  P99:     {} ns", p99_mincut);
            println!("  Max:     {} ns", max_mincut);
        }

        // Throughput calculation
        let total_time: Duration = self.tick_times.iter().sum();
        let throughput = self.total_ticks as f64 / total_time.as_secs_f64();
        println!("\nThroughput: {:.0} syndromes/sec", throughput);
    }
}

/// Generate a random syndrome delta based on error rate
fn generate_syndrome(round: u32, error_rate: f64, code_distance: usize) -> SyndromeDelta {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    // Pseudo-random based on round
    let mut hasher = DefaultHasher::new();
    round.hash(&mut hasher);
    let hash = hasher.finish();

    // Determine if this is an error event
    let is_error = (hash % 1000) < (error_rate * 1000.0) as u64;

    let source = ((hash >> 8) % (code_distance * code_distance) as u64) as u16;
    let target = ((hash >> 16) % (code_distance * code_distance) as u64) as u16;
    let value = if is_error { 200 } else { 50 }; // High value indicates potential error

    SyndromeDelta::new(source, target, value)
}

/// Run the coherence gate simulation
fn run_simulation(config: &SimConfig) -> SimStats {
    let mut stats = SimStats::default();

    println!("=== Coherence Gate Simulation ===");
    println!("Tiles: {}", config.num_tiles);
    println!("Rounds: {}", config.num_rounds);
    println!("Code distance: {}", config.code_distance);
    println!("Error rate: {:.2}%", config.error_rate * 100.0);

    // Initialize worker tiles
    let mut workers: Vec<WorkerTile> = (1..=config.num_tiles)
        .map(|id| WorkerTile::new(id as u8))
        .collect();

    // Initialize TileZero with signing key
    let thresholds = GateThresholds {
        structural_min_cut: 3.0,
        shift_max: 0.6,
        tau_deny: 0.05,
        tau_permit: 50.0,
        permit_ttl_ns: 4_000_000,
    };
    let mut tilezero = TileZero::with_random_key(thresholds);

    // Initialize min-cut engine if feature enabled
    #[cfg(feature = "structural")]
    let mut mincut_engine = if config.use_real_mincut {
        Some(DynamicMinCutEngine::new())
    } else {
        None
    };

    // Build initial graph structure (surface code lattice)
    #[cfg(feature = "structural")]
    if let Some(ref mut engine) = mincut_engine {
        let d = config.code_distance;
        // Create lattice edges
        for i in 0..d {
            for j in 0..d {
                let v = (i * d + j) as u32;
                if j + 1 < d {
                    engine.insert_edge(v, v + 1, 1.0);
                }
                if i + 1 < d {
                    engine.insert_edge(v, v + d as u32, 1.0);
                }
            }
        }
    }

    println!("\nRunning simulation...\n");

    // Main simulation loop
    for round in 0..config.num_rounds {
        // Generate syndrome for this round
        let syndrome = generate_syndrome(round as u32, config.error_rate, config.code_distance);

        // Process syndrome through all worker tiles
        let mut reports: Vec<TileReport> = Vec::with_capacity(config.num_tiles);

        for worker in &mut workers {
            let tick_start = Instant::now();
            let report = worker.tick(&syndrome);
            stats.tick_times.push(tick_start.elapsed());
            stats.total_ticks += 1;
            reports.push(report);
        }

        // Run min-cut query if enabled
        #[cfg(feature = "structural")]
        if let Some(ref mut engine) = mincut_engine {
            // Simulate dynamic edge updates based on syndrome
            if syndrome.is_syndrome() && syndrome.value > 100 {
                let mincut_start = Instant::now();

                // Update graph with syndrome information
                let u = syndrome.source as u32;
                let v = syndrome.target as u32;
                if u != v {
                    engine.insert_edge(u, v, 0.5); // Add weak edge for error correlation
                }

                // Query min-cut
                let _cut_value = engine.min_cut_value();
                stats.mincut_times.push(mincut_start.elapsed());
            }
        }

        // TileZero merges reports and makes decision
        let merge_start = Instant::now();
        let decision = tilezero.merge_reports(reports);
        stats.merge_times.push(merge_start.elapsed());
        stats.total_decisions += 1;

        match decision {
            GateDecision::Permit => stats.permits += 1,
            GateDecision::Defer => stats.defers += 1,
            GateDecision::Deny => stats.denies += 1,
        }

        // Issue and verify permit token periodically
        if round % 100 == 0 && decision == GateDecision::Permit {
            let token = tilezero.issue_permit(&decision);
            let verified = tilezero.verify_token(&token);
            assert_eq!(
                verified,
                Some(true),
                "Token verification failed at round {}",
                round
            );
        }

        // Progress indicator
        if round % (config.num_rounds / 10).max(1) == 0 {
            print!(".");
            use std::io::Write;
            std::io::stdout().flush().ok();
        }
    }

    println!(" Done!\n");

    // Verify receipt log integrity
    assert!(
        tilezero.receipt_log.verify_chain(),
        "Receipt log chain verification failed!"
    );
    println!(
        "Receipt log verified: {} entries, chain intact",
        tilezero.receipt_log.len()
    );

    stats
}

/// Run DetectorBitmap SIMD benchmarks
fn benchmark_detector_bitmap() {
    println!("\n=== DetectorBitmap Performance ===");

    const NUM_DETECTORS: usize = 1024;
    const ITERATIONS: usize = 100_000;

    let mut bitmap1 = DetectorBitmap::new(NUM_DETECTORS);
    let mut bitmap2 = DetectorBitmap::new(NUM_DETECTORS);

    // Set some bits
    for i in (0..NUM_DETECTORS).step_by(3) {
        bitmap1.set(i, true);
    }
    for i in (0..NUM_DETECTORS).step_by(5) {
        bitmap2.set(i, true);
    }

    // Benchmark popcount
    let start = Instant::now();
    let mut total = 0usize;
    for _ in 0..ITERATIONS {
        total += bitmap1.popcount();
    }
    let popcount_time = start.elapsed();
    println!(
        "Popcount ({} iterations): {:?} ({:.1} ns/op)",
        ITERATIONS,
        popcount_time,
        popcount_time.as_nanos() as f64 / ITERATIONS as f64
    );
    println!("  Result: {} bits set", total / ITERATIONS);

    // Benchmark XOR
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = bitmap1.xor(&bitmap2);
    }
    let xor_time = start.elapsed();
    println!(
        "XOR ({} iterations): {:?} ({:.1} ns/op)",
        ITERATIONS,
        xor_time,
        xor_time.as_nanos() as f64 / ITERATIONS as f64
    );

    // Benchmark AND
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = bitmap1.and(&bitmap2);
    }
    let and_time = start.elapsed();
    println!(
        "AND ({} iterations): {:?} ({:.1} ns/op)",
        ITERATIONS,
        and_time,
        and_time.as_nanos() as f64 / ITERATIONS as f64
    );

    // Benchmark OR
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = bitmap1.or(&bitmap2);
    }
    let or_time = start.elapsed();
    println!(
        "OR ({} iterations): {:?} ({:.1} ns/op)",
        ITERATIONS,
        or_time,
        or_time.as_nanos() as f64 / ITERATIONS as f64
    );
}

fn main() {
    // Run main simulation
    let config = SimConfig {
        num_tiles: 64,
        num_rounds: 10_000,
        code_distance: 7,
        error_rate: 0.01,
        use_real_mincut: cfg!(feature = "structural"),
    };

    let stats = run_simulation(&config);
    stats.report();

    // Run bitmap benchmarks
    benchmark_detector_bitmap();

    // Summary
    println!("\n=== Optimization Targets ===");

    if !stats.tick_times.is_empty() {
        let tick_ns: Vec<u64> = stats
            .tick_times
            .iter()
            .map(|d| d.as_nanos() as u64)
            .collect();
        let mut sorted = tick_ns.clone();
        sorted.sort();
        let p99 = sorted[sorted.len() * 99 / 100];

        if p99 > 4000 {
            println!("WARNING: Tick P99 ({} ns) exceeds 4μs target", p99);
        } else {
            println!("OK: Tick P99 ({} ns) within 4μs target", p99);
        }
    }

    println!("\nSimulation complete!");
}
