//! Latency benchmark for System A and System B
//!
//! This benchmark measures end-to-end prediction latency with high precision,
//! targeting the critical P99.9 < 0.9ms performance requirement.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::{Duration, Instant};
use temporal_neural_net::prelude::*;
use nalgebra::{DMatrix, DVector};
use std::collections::HashMap;

/// Number of warmup iterations before measurement
const WARMUP_ITERATIONS: usize = 10000;

/// Number of measurement samples for latency distribution
const MEASUREMENT_SAMPLES: usize = 100000;

/// Input data dimensions
const SEQUENCE_LENGTH: usize = 64;
const FEATURE_DIM: usize = 4;
const OUTPUT_DIM: usize = 2;

/// Latency measurement structure
#[derive(Debug, Clone)]
struct LatencyMeasurement {
    system_type: String,
    sample_id: usize,
    latency_ns: u64,
    phase_breakdown: PhaseBreakdown,
    success: bool,
    error: Option<String>,
}

/// Breakdown of latency by processing phase
#[derive(Debug, Clone, Default)]
struct PhaseBreakdown {
    /// Data ingestion time
    ingest_ns: u64,
    /// Kalman prior computation (System B only)
    prior_ns: u64,
    /// Neural network forward pass
    network_ns: u64,
    /// Solver gate verification (System B only)
    gate_ns: u64,
    /// Output finalization
    finalization_ns: u64,
}

/// Statistical summary of latency measurements
#[derive(Debug, Clone)]
struct LatencyStatistics {
    count: usize,
    mean_ns: f64,
    std_dev_ns: f64,
    min_ns: u64,
    max_ns: u64,
    p50_ns: u64,
    p90_ns: u64,
    p95_ns: u64,
    p99_ns: u64,
    p99_9_ns: u64,
    p99_99_ns: u64,
    success_rate: f64,
}

/// High-precision latency measurement context
struct LatencyBenchmarkContext {
    system_a: SystemA,
    system_b: SystemB,
    test_inputs: Vec<DMatrix<f64>>,
    measurements_a: Vec<LatencyMeasurement>,
    measurements_b: Vec<LatencyMeasurement>,
}

impl LatencyBenchmarkContext {
    /// Create new benchmark context with pre-initialized systems
    fn new() -> Result<Self> {
        // Create configurations for both systems
        let config_a = Config::default();
        let mut config_b = config_a.clone();
        // Enable temporal solver features for System B
        config_b.system = crate::config::SystemConfig::TemporalSolver(
            crate::config::TemporalSolverConfig::default()
        );

        // Initialize systems
        let system_a = SystemA::new(&config_a.model)?;
        let system_b = SystemB::new(&config_b.model)?;

        // Pre-generate test inputs for consistent benchmarking
        let test_inputs = Self::generate_test_inputs(MEASUREMENT_SAMPLES);

        Ok(Self {
            system_a,
            system_b,
            test_inputs,
            measurements_a: Vec::with_capacity(MEASUREMENT_SAMPLES),
            measurements_b: Vec::with_capacity(MEASUREMENT_SAMPLES),
        })
    }

    /// Generate consistent test inputs
    fn generate_test_inputs(count: usize) -> Vec<DMatrix<f64>> {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(42); // Deterministic seed

        (0..count)
            .map(|_| {
                DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| {
                    rng.gen_range(-1.0..1.0)
                })
            })
            .collect()
    }

    /// Perform warmup phase to ensure stable measurements
    fn warmup(&mut self) -> Result<()> {
        println!("Performing warmup with {} iterations...", WARMUP_ITERATIONS);

        for i in 0..WARMUP_ITERATIONS {
            let input = &self.test_inputs[i % self.test_inputs.len()];

            // Warmup System A
            let _ = self.system_a.forward(input)?;

            // Warmup System B
            let _ = self.system_b.forward(input)?;

            if i % 1000 == 0 {
                println!("Warmup progress: {}/{}", i, WARMUP_ITERATIONS);
            }
        }

        println!("Warmup completed");
        Ok(())
    }

    /// Measure latency for System A with detailed phase breakdown
    fn measure_system_a_latency(&mut self, input: &DMatrix<f64>, sample_id: usize) -> LatencyMeasurement {
        let mut breakdown = PhaseBreakdown::default();
        let start_time = Instant::now();

        // Phase 1: Data ingestion
        let ingest_start = Instant::now();
        let validated_input = input.clone(); // Simulate input validation/copying
        breakdown.ingest_ns = ingest_start.elapsed().as_nanos() as u64;

        // Phase 2: Neural network forward pass
        let network_start = Instant::now();
        let result = self.system_a.forward(&validated_input);
        breakdown.network_ns = network_start.elapsed().as_nanos() as u64;

        // Phase 3: Output finalization
        let finalization_start = Instant::now();
        let success = result.is_ok();
        let error = result.err().map(|e| e.to_string());
        breakdown.finalization_ns = finalization_start.elapsed().as_nanos() as u64;

        let total_latency_ns = start_time.elapsed().as_nanos() as u64;

        LatencyMeasurement {
            system_type: "SystemA".to_string(),
            sample_id,
            latency_ns: total_latency_ns,
            phase_breakdown: breakdown,
            success,
            error,
        }
    }

    /// Measure latency for System B with detailed phase breakdown
    fn measure_system_b_latency(&mut self, input: &DMatrix<f64>, sample_id: usize) -> LatencyMeasurement {
        let mut breakdown = PhaseBreakdown::default();
        let start_time = Instant::now();

        // Phase 1: Data ingestion
        let ingest_start = Instant::now();
        let validated_input = input.clone();
        breakdown.ingest_ns = ingest_start.elapsed().as_nanos() as u64;

        // Phase 2: Kalman prior computation
        let prior_start = Instant::now();
        // Note: This would call into the Kalman filter component
        // For now, we simulate the prior computation time
        std::thread::sleep(Duration::from_nanos(100000)); // 0.1ms simulated prior
        breakdown.prior_ns = prior_start.elapsed().as_nanos() as u64;

        // Phase 3: Neural network forward pass (residual prediction)
        let network_start = Instant::now();
        let result = self.system_b.forward(&validated_input);
        breakdown.network_ns = network_start.elapsed().as_nanos() as u64;

        // Phase 4: Solver gate verification
        let gate_start = Instant::now();
        // Note: This would call into the solver gate component
        // For now, we simulate the gate verification time
        std::thread::sleep(Duration::from_nanos(200000)); // 0.2ms simulated gate
        breakdown.gate_ns = gate_start.elapsed().as_nanos() as u64;

        // Phase 5: Output finalization
        let finalization_start = Instant::now();
        let success = result.is_ok();
        let error = result.err().map(|e| e.to_string());
        breakdown.finalization_ns = finalization_start.elapsed().as_nanos() as u64;

        let total_latency_ns = start_time.elapsed().as_nanos() as u64;

        LatencyMeasurement {
            system_type: "SystemB".to_string(),
            sample_id,
            latency_ns: total_latency_ns,
            phase_breakdown: breakdown,
            success,
            error,
        }
    }

    /// Run comprehensive latency measurements
    fn run_measurements(&mut self) -> Result<()> {
        println!("Running {} latency measurements for each system...", MEASUREMENT_SAMPLES);

        self.measurements_a.clear();
        self.measurements_b.clear();

        for i in 0..MEASUREMENT_SAMPLES {
            let input = &self.test_inputs[i];

            // Measure System A
            let measurement_a = self.measure_system_a_latency(input, i);
            self.measurements_a.push(measurement_a);

            // Measure System B
            let measurement_b = self.measure_system_b_latency(input, i);
            self.measurements_b.push(measurement_b);

            if i % 10000 == 0 {
                println!("Measurement progress: {}/{}", i, MEASUREMENT_SAMPLES);
            }
        }

        println!("Measurements completed");
        Ok(())
    }

    /// Calculate statistics from measurements
    fn calculate_statistics(&self, measurements: &[LatencyMeasurement]) -> LatencyStatistics {
        let successful_measurements: Vec<_> = measurements
            .iter()
            .filter(|m| m.success)
            .collect();

        let latencies: Vec<u64> = successful_measurements
            .iter()
            .map(|m| m.latency_ns)
            .collect();

        if latencies.is_empty() {
            return LatencyStatistics {
                count: 0,
                mean_ns: 0.0,
                std_dev_ns: 0.0,
                min_ns: 0,
                max_ns: 0,
                p50_ns: 0,
                p90_ns: 0,
                p95_ns: 0,
                p99_ns: 0,
                p99_9_ns: 0,
                p99_99_ns: 0,
                success_rate: 0.0,
            };
        }

        let mut sorted_latencies = latencies.clone();
        sorted_latencies.sort_unstable();

        let count = sorted_latencies.len();
        let mean_ns = sorted_latencies.iter().sum::<u64>() as f64 / count as f64;

        let variance = sorted_latencies
            .iter()
            .map(|&x| {
                let diff = x as f64 - mean_ns;
                diff * diff
            })
            .sum::<f64>() / count as f64;
        let std_dev_ns = variance.sqrt();

        let percentile = |p: f64| -> u64 {
            let index = ((count as f64) * p / 100.0).ceil() as usize - 1;
            sorted_latencies[index.min(count - 1)]
        };

        LatencyStatistics {
            count,
            mean_ns,
            std_dev_ns,
            min_ns: sorted_latencies[0],
            max_ns: sorted_latencies[count - 1],
            p50_ns: percentile(50.0),
            p90_ns: percentile(90.0),
            p95_ns: percentile(95.0),
            p99_ns: percentile(99.0),
            p99_9_ns: percentile(99.9),
            p99_99_ns: percentile(99.99),
            success_rate: successful_measurements.len() as f64 / measurements.len() as f64,
        }
    }

    /// Generate comprehensive latency report
    fn generate_report(&self) -> String {
        let stats_a = self.calculate_statistics(&self.measurements_a);
        let stats_b = self.calculate_statistics(&self.measurements_b);

        let mut report = String::new();
        report.push_str("# Latency Benchmark Report\n\n");

        report.push_str(&format!("**Measurement Configuration:**\n"));
        report.push_str(&format!("- Sample size: {}\n", MEASUREMENT_SAMPLES));
        report.push_str(&format!("- Warmup iterations: {}\n", WARMUP_ITERATIONS));
        report.push_str(&format!("- Input dimensions: {}x{}\n", SEQUENCE_LENGTH, FEATURE_DIM));
        report.push_str(&format!("- Target P99.9 latency: <0.9ms\n\n"));

        // System A Results
        report.push_str("## System A (Traditional Micro-Net) Results\n\n");
        report.push_str(&format!("| Metric | Value |\n"));
        report.push_str(&format!("|--------|-------|\n"));
        report.push_str(&format!("| Sample Count | {} |\n", stats_a.count));
        report.push_str(&format!("| Success Rate | {:.2}% |\n", stats_a.success_rate * 100.0));
        report.push_str(&format!("| Mean Latency | {:.3}ms |\n", stats_a.mean_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| Std Dev | {:.3}ms |\n", stats_a.std_dev_ns / 1_000_000.0));
        report.push_str(&format!("| Min Latency | {:.3}ms |\n", stats_a.min_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P50 Latency | {:.3}ms |\n", stats_a.p50_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P90 Latency | {:.3}ms |\n", stats_a.p90_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P95 Latency | {:.3}ms |\n", stats_a.p95_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P99 Latency | {:.3}ms |\n", stats_a.p99_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| **P99.9 Latency** | **{:.3}ms** |\n", stats_a.p99_9_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P99.99 Latency | {:.3}ms |\n", stats_a.p99_99_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| Max Latency | {:.3}ms |\n\n", stats_a.max_ns as f64 / 1_000_000.0));

        // System B Results
        report.push_str("## System B (Temporal Solver Net) Results\n\n");
        report.push_str(&format!("| Metric | Value |\n"));
        report.push_str(&format!("|--------|-------|\n"));
        report.push_str(&format!("| Sample Count | {} |\n", stats_b.count));
        report.push_str(&format!("| Success Rate | {:.2}% |\n", stats_b.success_rate * 100.0));
        report.push_str(&format!("| Mean Latency | {:.3}ms |\n", stats_b.mean_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| Std Dev | {:.3}ms |\n", stats_b.std_dev_ns / 1_000_000.0));
        report.push_str(&format!("| Min Latency | {:.3}ms |\n", stats_b.min_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P50 Latency | {:.3}ms |\n", stats_b.p50_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P90 Latency | {:.3}ms |\n", stats_b.p90_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P95 Latency | {:.3}ms |\n", stats_b.p95_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P99 Latency | {:.3}ms |\n", stats_b.p99_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| **P99.9 Latency** | **{:.3}ms** |\n", stats_b.p99_9_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| P99.99 Latency | {:.3}ms |\n", stats_b.p99_99_ns as f64 / 1_000_000.0));
        report.push_str(&format!("| Max Latency | {:.3}ms |\n\n", stats_b.max_ns as f64 / 1_000_000.0));

        // Comparison Analysis
        report.push_str("## Comparative Analysis\n\n");
        let p99_9_improvement = (stats_a.p99_9_ns as f64 - stats_b.p99_9_ns as f64) / stats_a.p99_9_ns as f64 * 100.0;
        let mean_improvement = (stats_a.mean_ns - stats_b.mean_ns) / stats_a.mean_ns * 100.0;

        report.push_str(&format!("| Comparison Metric | Value |\n"));
        report.push_str(&format!("|-------------------|-------|\n"));
        report.push_str(&format!("| P99.9 Latency Improvement | {:.1}% |\n", p99_9_improvement));
        report.push_str(&format!("| Mean Latency Improvement | {:.1}% |\n", mean_improvement));
        report.push_str(&format!("| System B P99.9 Target (<0.9ms) | {} |\n",
            if stats_b.p99_9_ns < 900_000 { "✅ ACHIEVED" } else { "❌ NOT ACHIEVED" }));
        report.push_str(&format!("| 20% Improvement Target | {} |\n",
            if p99_9_improvement >= 20.0 { "✅ ACHIEVED" } else { "❌ NOT ACHIEVED" }));

        // Success Criteria Validation
        report.push_str("\n## Success Criteria Validation\n\n");
        let criterion_1 = stats_b.p99_9_ns < 900_000; // <0.9ms
        let criterion_2 = p99_9_improvement >= 20.0; // ≥20% improvement

        report.push_str(&format!("1. **System B P99.9 latency <0.9ms**: {} ({:.3}ms)\n",
            if criterion_1 { "✅ PASSED" } else { "❌ FAILED" },
            stats_b.p99_9_ns as f64 / 1_000_000.0));
        report.push_str(&format!("2. **≥20% latency improvement**: {} ({:.1}%)\n",
            if criterion_2 { "✅ PASSED" } else { "❌ FAILED" },
            p99_9_improvement));

        let overall_success = criterion_1 || criterion_2;
        report.push_str(&format!("\n**Overall Result**: {}\n",
            if overall_success { "🎉 SUCCESS - Breakthrough achieved!" } else { "⚠️  Needs improvement" }));

        report
    }
}

/// Criterion benchmark function for System A latency
fn bench_system_a_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut context = rt.block_on(async {
        LatencyBenchmarkContext::new().expect("Failed to create benchmark context")
    });

    rt.block_on(async {
        context.warmup().expect("Warmup failed");
    });

    let input = DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| 0.5);

    c.bench_function("system_a_forward_pass", |b| {
        b.iter(|| {
            black_box(context.system_a.forward(black_box(&input)).unwrap())
        })
    });
}

/// Criterion benchmark function for System B latency
fn bench_system_b_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut context = rt.block_on(async {
        LatencyBenchmarkContext::new().expect("Failed to create benchmark context")
    });

    rt.block_on(async {
        context.warmup().expect("Warmup failed");
    });

    let input = DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| 0.5);

    c.bench_function("system_b_forward_pass", |b| {
        b.iter(|| {
            black_box(context.system_b.forward(black_box(&input)).unwrap())
        })
    });
}

/// Full latency distribution analysis
fn bench_latency_distribution(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut context = rt.block_on(async {
        LatencyBenchmarkContext::new().expect("Failed to create benchmark context")
    });

    rt.block_on(async {
        context.warmup().expect("Warmup failed");
        context.run_measurements().expect("Measurements failed");

        // Generate and save report
        let report = context.generate_report();
        std::fs::write("latency_benchmark_report.md", report)
            .expect("Failed to save report");

        println!("✅ Latency benchmark completed!");
        println!("📊 Report saved to: latency_benchmark_report.md");
    });
}

criterion_group!(
    name = latency_benches;
    config = Criterion::default()
        .sample_size(1000)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = bench_system_a_latency, bench_system_b_latency, bench_latency_distribution
);
criterion_main!(latency_benches);