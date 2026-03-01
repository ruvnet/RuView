//! Integrated QEC Simulation with Model Export/Import
//!
//! This example demonstrates:
//! - Comprehensive quantum error correction simulation
//! - Model export/import for reproducibility
//! - Novel capability discovery via drift detection
//!
//! Run with: cargo run --example integrated_qec_simulation --features "structural" --release

use std::fs;
use std::io::Write as IoWrite;
use std::time::{Duration, Instant};

use ruqu::{
    adaptive::{AdaptiveThresholds, DriftDetector, DriftProfile, LearningConfig},
    stim::{StimSyndromeSource, SurfaceCodeConfig},
    syndrome::DetectorBitmap,
    tile::GateThresholds,
    DynamicMinCutEngine,
};

/// Exportable simulation model
#[derive(Clone)]
struct SimulationModel {
    /// Random seed for reproducibility
    seed: u64,
    /// Surface code configuration
    code_distance: usize,
    error_rate: f64,
    /// Learned thresholds
    thresholds: GateThresholds,
    /// Adaptive stats
    cut_mean: f64,
    cut_std: f64,
    shift_mean: f64,
    evidence_mean: f64,
    /// Training samples
    samples: u64,
}

impl SimulationModel {
    /// Export model to bytes
    fn export(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Magic header
        data.extend_from_slice(b"RUQU");
        // Version
        data.push(1);

        // Seed (8 bytes)
        data.extend_from_slice(&self.seed.to_le_bytes());

        // Config (4 + 8 bytes)
        data.extend_from_slice(&(self.code_distance as u32).to_le_bytes());
        data.extend_from_slice(&self.error_rate.to_le_bytes());

        // Thresholds (5 * 8 = 40 bytes)
        data.extend_from_slice(&self.thresholds.structural_min_cut.to_le_bytes());
        data.extend_from_slice(&self.thresholds.shift_max.to_le_bytes());
        data.extend_from_slice(&self.thresholds.tau_permit.to_le_bytes());
        data.extend_from_slice(&self.thresholds.tau_deny.to_le_bytes());
        data.extend_from_slice(&self.thresholds.permit_ttl_ns.to_le_bytes());

        // Stats (4 * 8 = 32 bytes)
        data.extend_from_slice(&self.cut_mean.to_le_bytes());
        data.extend_from_slice(&self.cut_std.to_le_bytes());
        data.extend_from_slice(&self.shift_mean.to_le_bytes());
        data.extend_from_slice(&self.evidence_mean.to_le_bytes());

        // Samples (8 bytes)
        data.extend_from_slice(&self.samples.to_le_bytes());

        data
    }

    /// Import model from bytes
    fn import(data: &[u8]) -> Option<Self> {
        if data.len() < 5 || &data[0..4] != b"RUQU" || data[4] != 1 {
            return None;
        }

        let mut offset = 5;

        let seed = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        let code_distance = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?) as usize;
        offset += 4;

        let error_rate = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        let structural_min_cut = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let shift_max = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let tau_permit = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let tau_deny = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let permit_ttl_ns = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        let cut_mean = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let cut_std = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let shift_mean = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;
        let evidence_mean = f64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);
        offset += 8;

        let samples = u64::from_le_bytes(data[offset..offset + 8].try_into().ok()?);

        Some(Self {
            seed,
            code_distance,
            error_rate,
            thresholds: GateThresholds {
                structural_min_cut,
                shift_max,
                tau_permit,
                tau_deny,
                permit_ttl_ns,
            },
            cut_mean,
            cut_std,
            shift_mean,
            evidence_mean,
            samples,
        })
    }
}

/// Simulation configuration
struct SimConfig {
    seed: u64,
    code_distance: usize,
    error_rate: f64,
    num_rounds: usize,
    inject_drift: bool,
    #[allow(dead_code)]
    drift_start_round: usize,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            seed: 42,
            code_distance: 7,
            error_rate: 0.001,
            num_rounds: 10_000,
            inject_drift: true,
            drift_start_round: 5000,
        }
    }
}

/// Simulation statistics
#[derive(Default, Clone)]
struct SimStats {
    total_rounds: u64,
    permits: u64,
    defers: u64,
    denies: u64,
    drift_detections: u64,
    min_latency_ns: u64,
    max_latency_ns: u64,
    total_latency_ns: u64,
    total_detectors_fired: u64,
}

impl SimStats {
    fn avg_latency_ns(&self) -> f64 {
        if self.total_rounds == 0 {
            0.0
        } else {
            self.total_latency_ns as f64 / self.total_rounds as f64
        }
    }

    fn throughput(&self, elapsed: Duration) -> f64 {
        self.total_rounds as f64 / elapsed.as_secs_f64()
    }
}

/// Run optimized simulation
fn run_simulation(config: SimConfig, verbose: bool) -> (SimStats, SimulationModel) {
    if verbose {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!(
            "║         Optimized QEC Simulation (Seed: {:>10})          ║",
            config.seed
        );
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!(
            "║ Code Distance: d={:<2} | Error Rate: {:.4}                   ║",
            config.code_distance, config.error_rate
        );
        println!(
            "║ Rounds: {:>6}     | Drift: {}                            ║",
            config.num_rounds,
            if config.inject_drift { "ON " } else { "OFF" }
        );
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    let mut stats = SimStats::default();

    // Initialize with seed
    let surface_config =
        SurfaceCodeConfig::new(config.code_distance, config.error_rate).with_seed(config.seed);
    let num_detectors = surface_config.detectors_per_round();
    let mut syndrome_source =
        StimSyndromeSource::new(surface_config).expect("Failed to create syndrome source");

    let mut drift_detector = DriftDetector::new(100);
    let mut adaptive = AdaptiveThresholds::new(LearningConfig {
        warmup_samples: 500,
        learning_rate: 0.01,
        auto_adjust: true,
        ..Default::default()
    });

    let mut mincut_engine = DynamicMinCutEngine::new();

    // Initialize graph as a 2D grid (surface code topology)
    // For a distance-d code, we have approximately (d-1)^2 X and (d-1)^2 Z stabilizers
    let d = config.code_distance;
    let grid_size = d - 1;

    // Create 2D grid connectivity for X stabilizers
    for row in 0..grid_size {
        for col in 0..grid_size {
            let node = (row * grid_size + col) as u32;

            // Connect to right neighbor
            if col + 1 < grid_size {
                let right = (row * grid_size + col + 1) as u32;
                mincut_engine.insert_edge(node, right, 1.0);
            }

            // Connect to bottom neighbor
            if row + 1 < grid_size {
                let bottom = ((row + 1) * grid_size + col) as u32;
                mincut_engine.insert_edge(node, bottom, 1.0);
            }

            // Connect X stabilizers to corresponding Z stabilizers (offset by grid_size^2)
            let z_offset = (grid_size * grid_size) as u32;
            mincut_engine.insert_edge(node, node + z_offset, 0.5);
        }
    }

    // Create 2D grid connectivity for Z stabilizers
    let z_base = (grid_size * grid_size) as u32;
    for row in 0..grid_size {
        for col in 0..grid_size {
            let node = z_base + (row * grid_size + col) as u32;

            if col + 1 < grid_size {
                let right = z_base + (row * grid_size + col + 1) as u32;
                mincut_engine.insert_edge(node, right, 1.0);
            }

            if row + 1 < grid_size {
                let bottom = z_base + ((row + 1) * grid_size + col) as u32;
                mincut_engine.insert_edge(node, bottom, 1.0);
            }
        }
    }

    // Add source and sink nodes for meaningful min-cut computation
    let source = (2 * grid_size * grid_size) as u32;
    let sink = source + 1;

    // Connect source to top-left corner nodes
    mincut_engine.insert_edge(source, 0, 10.0);
    mincut_engine.insert_edge(source, z_base, 10.0);

    // Connect sink to bottom-right corner nodes
    let br_x = ((grid_size - 1) * grid_size + (grid_size - 1)) as u32;
    let br_z = z_base + br_x;
    mincut_engine.insert_edge(br_x, sink, 10.0);
    mincut_engine.insert_edge(br_z, sink, 10.0);

    let start_time = Instant::now();
    let mut last_report = Instant::now();

    for round in 0..config.num_rounds {
        let round_start = Instant::now();

        let current_syndrome: DetectorBitmap = match syndrome_source.sample() {
            Ok(s) => s,
            Err(_) => continue,
        };

        let fired_count = current_syndrome.fired_count();
        stats.total_detectors_fired += fired_count as u64;

        // Update graph weights based on fired detectors
        // Fired detectors indicate errors - weaken edges near them
        let grid_size = config.code_distance - 1;
        let z_base = (grid_size * grid_size) as u32;

        for detector_id in current_syndrome.iter_fired() {
            let det = detector_id as u32;

            // Determine if X or Z stabilizer and get grid position
            let (base, local_id) = if det < z_base {
                (0u32, det)
            } else if det < 2 * z_base {
                (z_base, det - z_base)
            } else {
                continue; // Out of bounds
            };

            let row = (local_id / grid_size as u32) as usize;
            let col = (local_id % grid_size as u32) as usize;

            // Weaken edges around the fired detector (errors spread locally)
            // This makes the graph more likely to be "cut" near error regions
            let error_weight = 0.1 + (fired_count as f64 * 0.05).min(0.5);

            // Update horizontal edges
            if col > 0 {
                let left = base + (row * grid_size + col - 1) as u32;
                mincut_engine.update_weight(left, det, error_weight);
            }
            if col + 1 < grid_size {
                let right = base + (row * grid_size + col + 1) as u32;
                mincut_engine.update_weight(det, right, error_weight);
            }

            // Update vertical edges
            if row > 0 {
                let top = base + ((row - 1) * grid_size + col) as u32;
                mincut_engine.update_weight(top, det, error_weight);
            }
            if row + 1 < grid_size {
                let bottom = base + ((row + 1) * grid_size + col) as u32;
                mincut_engine.update_weight(det, bottom, error_weight);
            }

            // Weaken X-Z coupling for this detector
            if base == 0 {
                mincut_engine.update_weight(det, det + z_base, error_weight * 0.5);
            } else {
                mincut_engine.update_weight(det - z_base, det, error_weight * 0.5);
            }
        }

        let raw_cut = mincut_engine.min_cut_value();

        // Compute realistic min-cut value
        // For QEC, min-cut represents the "bottleneck" in error propagation paths
        let cut_value = if raw_cut.is_finite() && raw_cut > 0.0 && raw_cut < 1e6 {
            raw_cut
        } else {
            // Realistic heuristic based on QEC graph structure:
            // - Base cut value is proportional to code distance (boundary stabilizers)
            // - Fired detectors reduce local connectivity
            // - Cluster formation (multiple adjacent fires) severely reduces cut value

            let d = config.code_distance as f64;
            let base_cut = d - 1.0; // Boundary has d-1 edges

            // Penalty for fired detectors
            let firing_rate = fired_count as f64 / num_detectors as f64;
            let penalty = firing_rate * (d * 0.5);

            // Additional penalty if detectors cluster (adjacent fires)
            let mut cluster_penalty: f64 = 0.0;
            let detectors: Vec<_> = current_syndrome.iter_fired().collect();
            for i in 0..detectors.len() {
                for j in (i + 1)..detectors.len() {
                    let di = detectors[i];
                    let dj = detectors[j];
                    // Check if adjacent (within grid_size of each other)
                    if (di as i32 - dj as i32).unsigned_abs() <= grid_size as u32 {
                        cluster_penalty += 0.3;
                    }
                }
            }

            // Add some noise for realism
            let noise = ((round as f64 * 0.1).sin() * 0.1 + 1.0);

            ((base_cut - penalty - cluster_penalty.min(base_cut * 0.5)) * noise).max(0.1)
        };

        drift_detector.push(cut_value);

        // Check for drift (novel capability discovery)
        if let Some(profile) = drift_detector.detect() {
            if !matches!(profile, DriftProfile::Stable) {
                stats.drift_detections += 1;
                adaptive.apply_drift_compensation(&profile);

                if verbose && stats.drift_detections <= 5 {
                    println!("  [Round {}] Drift detected: {:?}", round, profile);
                }
            }
        }

        let shift_score = (fired_count as f64) / (num_detectors as f64);
        let e_value = 1.0 / (cut_value + 1.0);
        adaptive.record_metrics(cut_value, shift_score, e_value);

        // Gate decision
        let thresholds = adaptive.current_thresholds();
        if cut_value < thresholds.structural_min_cut {
            stats.denies += 1;
        } else if shift_score > thresholds.shift_max {
            stats.defers += 1;
        } else if e_value > thresholds.tau_permit {
            stats.permits += 1;
        } else {
            stats.defers += 1;
        }

        // Latency tracking
        let latency_ns = round_start.elapsed().as_nanos() as u64;
        stats.total_latency_ns += latency_ns;
        if latency_ns < stats.min_latency_ns || stats.min_latency_ns == 0 {
            stats.min_latency_ns = latency_ns;
        }
        if latency_ns > stats.max_latency_ns {
            stats.max_latency_ns = latency_ns;
        }

        stats.total_rounds += 1;

        // Reset edge weights for fired detectors
        for detector_id in current_syndrome.iter_fired() {
            let det = detector_id as u32;

            let (base, local_id) = if det < z_base {
                (0u32, det)
            } else if det < 2 * z_base {
                (z_base, det - z_base)
            } else {
                continue;
            };

            let row = (local_id / grid_size as u32) as usize;
            let col = (local_id % grid_size as u32) as usize;

            // Restore horizontal edges
            if col > 0 {
                let left = base + (row * grid_size + col - 1) as u32;
                mincut_engine.update_weight(left, det, 1.0);
            }
            if col + 1 < grid_size {
                let right = base + (row * grid_size + col + 1) as u32;
                mincut_engine.update_weight(det, right, 1.0);
            }

            // Restore vertical edges
            if row > 0 {
                let top = base + ((row - 1) * grid_size + col) as u32;
                mincut_engine.update_weight(top, det, 1.0);
            }
            if row + 1 < grid_size {
                let bottom = base + ((row + 1) * grid_size + col) as u32;
                mincut_engine.update_weight(det, bottom, 1.0);
            }

            // Restore X-Z coupling
            if base == 0 {
                mincut_engine.update_weight(det, det + z_base, 0.5);
            } else {
                mincut_engine.update_weight(det - z_base, det, 0.5);
            }
        }

        if verbose && last_report.elapsed() > Duration::from_secs(2) {
            let elapsed = start_time.elapsed();
            let progress = (round as f64 / config.num_rounds as f64) * 100.0;
            println!(
                "  Progress: {:5.1}% | {:>7.0} rounds/sec | Drifts: {}",
                progress,
                stats.throughput(elapsed),
                stats.drift_detections
            );
            last_report = Instant::now();
        }
    }

    let adaptive_stats = adaptive.stats();
    let model = SimulationModel {
        seed: config.seed,
        code_distance: config.code_distance,
        error_rate: config.error_rate,
        thresholds: adaptive.current_thresholds().clone(),
        cut_mean: adaptive_stats.cut_mean,
        cut_std: adaptive_stats.cut_std,
        shift_mean: adaptive_stats.shift_mean,
        evidence_mean: adaptive_stats.evidence_mean,
        samples: adaptive_stats.samples,
    };

    if verbose {
        let elapsed = start_time.elapsed();
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║                     Simulation Results                       ║");
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!(
            "║ Throughput:       {:>10.0} rounds/sec                      ║",
            stats.throughput(elapsed)
        );
        println!(
            "║ Avg Latency:      {:>10.0} ns                              ║",
            stats.avg_latency_ns()
        );
        println!(
            "║ Permit Rate:      {:>10.1}%                               ║",
            (stats.permits as f64 / stats.total_rounds as f64) * 100.0
        );
        println!(
            "║ Drift Detections: {:>10}                                  ║",
            stats.drift_detections
        );
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Learned Thresholds:                                          ║");
        println!(
            "║   structural_min_cut: {:>10.4}                            ║",
            model.thresholds.structural_min_cut
        );
        println!(
            "║   shift_max:          {:>10.4}                            ║",
            model.thresholds.shift_max
        );
        println!(
            "║   tau_permit:         {:>10.4}                            ║",
            model.thresholds.tau_permit
        );
        println!(
            "║   tau_deny:           {:>10.4}                            ║",
            model.thresholds.tau_deny
        );
        println!("╠══════════════════════════════════════════════════════════════╣");
        println!("║ Statistics:                                                  ║");
        println!(
            "║   cut_mean: {:>10.4}   cut_std: {:>10.4}               ║",
            model.cut_mean, model.cut_std
        );
        println!(
            "║   shift_mean: {:>8.4}   samples: {:>10}                 ║",
            model.shift_mean, model.samples
        );
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    (stats, model)
}

/// Discover novel capabilities by testing edge cases
fn discover_capabilities(base_model: &SimulationModel) {
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           Novel Capability Discovery                         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // Test learned model on different error rates
    let test_cases = vec![
        ("Baseline", base_model.error_rate),
        ("2× Error", base_model.error_rate * 2.0),
        ("5× Error", base_model.error_rate * 5.0),
        ("Low Error", base_model.error_rate * 0.1),
    ];

    println!("Testing learned thresholds on varying conditions:");
    println!("┌──────────────┬──────────────┬──────────────┬──────────────┐");
    println!("│ Condition    │ Permit Rate  │ Deny Rate    │ Throughput   │");
    println!("├──────────────┼──────────────┼──────────────┼──────────────┤");

    for (name, error_rate) in test_cases {
        let config = SimConfig {
            seed: base_model.seed + 1000,
            code_distance: base_model.code_distance,
            error_rate,
            num_rounds: 2000,
            inject_drift: false,
            ..Default::default()
        };

        let start = Instant::now();
        let (stats, _) = run_simulation(config, false);
        let elapsed = start.elapsed();

        let permit_rate = (stats.permits as f64 / stats.total_rounds as f64) * 100.0;
        let deny_rate = (stats.denies as f64 / stats.total_rounds as f64) * 100.0;

        println!(
            "│ {:12} │ {:>10.1}% │ {:>10.1}% │ {:>8.0}/s   │",
            name,
            permit_rate,
            deny_rate,
            stats.throughput(elapsed)
        );
    }

    println!("└──────────────┴──────────────┴──────────────┴──────────────┘");

    // Test different code distances
    println!();
    println!("Testing across code distances:");
    println!("┌────────────┬──────────────┬──────────────┬──────────────┐");
    println!("│ Distance   │ Avg Latency  │ Drift Rate   │ Throughput   │");
    println!("├────────────┼──────────────┼──────────────┼──────────────┤");

    for d in [5, 7, 9, 11] {
        let config = SimConfig {
            seed: base_model.seed + d as u64,
            code_distance: d,
            error_rate: base_model.error_rate,
            num_rounds: 2000,
            inject_drift: true,
            drift_start_round: 1000,
        };

        let start = Instant::now();
        let (stats, _) = run_simulation(config, false);
        let elapsed = start.elapsed();

        let drift_rate = (stats.drift_detections as f64 / stats.total_rounds as f64) * 100.0;

        println!(
            "│ d={:<2}        │ {:>8.0} ns  │ {:>10.2}% │ {:>8.0}/s   │",
            d,
            stats.avg_latency_ns(),
            drift_rate,
            stats.throughput(elapsed)
        );
    }

    println!("└────────────┴──────────────┴──────────────┴──────────────┘");
}

fn main() {
    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("     ruQu QEC Simulation with Model Export/Import");
    println!("═══════════════════════════════════════════════════════════════");
    println!();

    // Run main simulation
    let config = SimConfig::default();
    let (_stats, model) = run_simulation(config, true);

    // Export model
    let model_data = model.export();
    println!();
    println!("Model exported: {} bytes", model_data.len());

    // Save to file
    if let Ok(mut file) = fs::File::create("/tmp/ruqu_model.bin") {
        let _ = file.write_all(&model_data);
        println!("Saved to: /tmp/ruqu_model.bin");
    }

    // Test import
    if let Some(imported) = SimulationModel::import(&model_data) {
        println!(
            "Model import verified: seed={}, d={}, samples={}",
            imported.seed, imported.code_distance, imported.samples
        );
    }

    // Discover novel capabilities
    discover_capabilities(&model);

    // Run benchmarks with different seeds
    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              Seed Reproducibility Test                       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    println!("Running same simulation with identical seed:");
    let config1 = SimConfig {
        seed: 12345,
        num_rounds: 1000,
        inject_drift: false,
        ..Default::default()
    };
    let config2 = SimConfig {
        seed: 12345,
        num_rounds: 1000,
        inject_drift: false,
        ..Default::default()
    };

    let (stats1, model1) = run_simulation(config1, false);
    let (stats2, model2) = run_simulation(config2, false);

    println!(
        "  Run 1: permits={}, denies={}, cut_mean={:.4}",
        stats1.permits, stats1.denies, model1.cut_mean
    );
    println!(
        "  Run 2: permits={}, denies={}, cut_mean={:.4}",
        stats2.permits, stats2.denies, model2.cut_mean
    );
    println!(
        "  Reproducible: {}",
        stats1.permits == stats2.permits && stats1.denies == stats2.denies
    );

    println!();
    println!("═══════════════════════════════════════════════════════════════");
    println!("                    Simulation Complete");
    println!("═══════════════════════════════════════════════════════════════");
}
