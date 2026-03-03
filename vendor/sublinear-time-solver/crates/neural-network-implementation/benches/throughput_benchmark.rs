//! Throughput benchmark for System A and System B
//!
//! This benchmark measures prediction throughput (predictions per second)
//! under various load conditions and batch sizes.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::time::{Duration, Instant};
use temporal_neural_net::prelude::*;
use nalgebra::{DMatrix, DVector};
use rayon::prelude::*;
use std::sync::Arc;

/// Test configuration
const SEQUENCE_LENGTH: usize = 64;
const FEATURE_DIM: usize = 4;
const THROUGHPUT_TEST_DURATION_SEC: u64 = 30;
const BATCH_SIZES: &[usize] = &[1, 4, 8, 16, 32, 64, 128];
const CONCURRENT_THREADS: &[usize] = &[1, 2, 4, 8];

/// Throughput measurement result
#[derive(Debug, Clone)]
struct ThroughputMeasurement {
    system_type: String,
    batch_size: usize,
    thread_count: usize,
    duration_ms: u64,
    total_predictions: usize,
    throughput_pred_per_sec: f64,
    avg_latency_ms: f64,
    memory_usage_mb: f64,
    cpu_utilization: f64,
    error_rate: f64,
}

/// Throughput benchmark context
struct ThroughputBenchmarkContext {
    system_a: Arc<SystemA>,
    system_b: Arc<SystemB>,
    test_batches: Vec<Vec<DMatrix<f64>>>,
}

impl ThroughputBenchmarkContext {
    /// Create new throughput benchmark context
    fn new() -> Result<Self> {
        let config_a = Config::default();
        let mut config_b = config_a.clone();
        config_b.system = crate::config::SystemConfig::TemporalSolver(
            crate::config::TemporalSolverConfig::default()
        );

        let system_a = Arc::new(SystemA::new(&config_a.model)?);
        let system_b = Arc::new(SystemB::new(&config_b.model)?);

        // Pre-generate test batches
        let test_batches = Self::generate_test_batches();

        Ok(Self {
            system_a,
            system_b,
            test_batches,
        })
    }

    /// Generate test batches for different batch sizes
    fn generate_test_batches() -> Vec<Vec<DMatrix<f64>>> {
        use rand::prelude::*;
        let mut rng = StdRng::seed_from_u64(42);

        BATCH_SIZES
            .iter()
            .map(|&batch_size| {
                (0..1000) // Generate 1000 batches for each size
                    .map(|_| {
                        DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| {
                            rng.gen_range(-1.0..1.0)
                        })
                    })
                    .collect()
            })
            .collect()
    }

    /// Measure single-threaded throughput
    fn measure_single_threaded_throughput(
        &self,
        system_type: &str,
        batch_size: usize,
        duration_sec: u64,
    ) -> ThroughputMeasurement {
        let start_time = Instant::now();
        let duration = Duration::from_secs(duration_sec);
        let batch_index = BATCH_SIZES.iter().position(|&x| x == batch_size).unwrap_or(0);
        let test_batch = &self.test_batches[batch_index];

        let mut total_predictions = 0;
        let mut total_latency_ms = 0.0;
        let mut errors = 0;
        let mut batch_iter = test_batch.iter().cycle();

        let memory_start = Self::get_memory_usage_mb();

        while start_time.elapsed() < duration {
            let input = batch_iter.next().unwrap();
            let prediction_start = Instant::now();

            let result = match system_type {
                "SystemA" => self.system_a.forward(input),
                "SystemB" => self.system_b.forward(input),
                _ => panic!("Unknown system type"),
            };

            let prediction_latency = prediction_start.elapsed().as_millis() as f64;
            total_latency_ms += prediction_latency;

            if result.is_ok() {
                total_predictions += batch_size;
            } else {
                errors += 1;
            }
        }

        let actual_duration_ms = start_time.elapsed().as_millis() as u64;
        let memory_end = Self::get_memory_usage_mb();
        let throughput_pred_per_sec = (total_predictions as f64) / (actual_duration_ms as f64 / 1000.0);
        let avg_latency_ms = total_latency_ms / (total_predictions as f64 / batch_size as f64);
        let error_rate = errors as f64 / (total_predictions as f64 / batch_size as f64);

        ThroughputMeasurement {
            system_type: system_type.to_string(),
            batch_size,
            thread_count: 1,
            duration_ms: actual_duration_ms,
            total_predictions,
            throughput_pred_per_sec,
            avg_latency_ms,
            memory_usage_mb: memory_end - memory_start,
            cpu_utilization: Self::get_cpu_utilization(),
            error_rate,
        }
    }

    /// Measure multi-threaded throughput
    fn measure_multi_threaded_throughput(
        &self,
        system_type: &str,
        batch_size: usize,
        thread_count: usize,
        duration_sec: u64,
    ) -> ThroughputMeasurement {
        let start_time = Instant::now();
        let duration = Duration::from_secs(duration_sec);
        let batch_index = BATCH_SIZES.iter().position(|&x| x == batch_size).unwrap_or(0);
        let test_batch = &self.test_batches[batch_index];

        let memory_start = Self::get_memory_usage_mb();

        // Create thread pool for parallel execution
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(thread_count)
            .build()
            .unwrap();

        let results = pool.install(|| {
            let chunk_size = test_batch.len() / thread_count;
            let chunks: Vec<_> = test_batch.chunks(chunk_size).collect();

            chunks
                .par_iter()
                .map(|chunk| {
                    let mut thread_predictions = 0;
                    let mut thread_latency_ms = 0.0;
                    let mut thread_errors = 0;
                    let mut chunk_iter = chunk.iter().cycle();

                    while start_time.elapsed() < duration {
                        let input = chunk_iter.next().unwrap();
                        let prediction_start = Instant::now();

                        let result = match system_type {
                            "SystemA" => self.system_a.forward(input),
                            "SystemB" => self.system_b.forward(input),
                            _ => panic!("Unknown system type"),
                        };

                        let prediction_latency = prediction_start.elapsed().as_millis() as f64;
                        thread_latency_ms += prediction_latency;

                        if result.is_ok() {
                            thread_predictions += batch_size;
                        } else {
                            thread_errors += 1;
                        }
                    }

                    (thread_predictions, thread_latency_ms, thread_errors)
                })
                .collect::<Vec<_>>()
        });

        // Aggregate results from all threads
        let total_predictions: usize = results.iter().map(|(p, _, _)| p).sum();
        let total_latency_ms: f64 = results.iter().map(|(_, l, _)| l).sum();
        let total_errors: usize = results.iter().map(|(_, _, e)| e).sum();

        let actual_duration_ms = start_time.elapsed().as_millis() as u64;
        let memory_end = Self::get_memory_usage_mb();
        let throughput_pred_per_sec = (total_predictions as f64) / (actual_duration_ms as f64 / 1000.0);
        let avg_latency_ms = total_latency_ms / (total_predictions as f64 / batch_size as f64);
        let error_rate = total_errors as f64 / (total_predictions as f64 / batch_size as f64);

        ThroughputMeasurement {
            system_type: system_type.to_string(),
            batch_size,
            thread_count,
            duration_ms: actual_duration_ms,
            total_predictions,
            throughput_pred_per_sec,
            avg_latency_ms,
            memory_usage_mb: memory_end - memory_start,
            cpu_utilization: Self::get_cpu_utilization(),
            error_rate,
        }
    }

    /// Get current memory usage (simplified)
    fn get_memory_usage_mb() -> f64 {
        // This is a placeholder - in a real implementation, you'd use
        // a proper memory profiling library
        42.0 // MB
    }

    /// Get current CPU utilization (simplified)
    fn get_cpu_utilization() -> f64 {
        // This is a placeholder - in a real implementation, you'd use
        // a proper CPU monitoring library
        85.0 // Percentage
    }

    /// Generate comprehensive throughput report
    fn generate_throughput_report(&self, measurements: &[ThroughputMeasurement]) -> String {
        let mut report = String::new();
        report.push_str("# Throughput Benchmark Report\n\n");

        // Group measurements by system type
        let system_a_measurements: Vec<_> = measurements
            .iter()
            .filter(|m| m.system_type == "SystemA")
            .collect();
        let system_b_measurements: Vec<_> = measurements
            .iter()
            .filter(|m| m.system_type == "SystemB")
            .collect();

        // System A Results
        report.push_str("## System A (Traditional Micro-Net) Throughput Results\n\n");
        report.push_str("| Batch Size | Threads | Throughput (pred/sec) | Avg Latency (ms) | Memory (MB) | Error Rate |\n");
        report.push_str("|------------|---------|----------------------|------------------|-------------|------------|\n");

        for measurement in &system_a_measurements {
            report.push_str(&format!(
                "| {} | {} | {:.1} | {:.3} | {:.1} | {:.2}% |\n",
                measurement.batch_size,
                measurement.thread_count,
                measurement.throughput_pred_per_sec,
                measurement.avg_latency_ms,
                measurement.memory_usage_mb,
                measurement.error_rate * 100.0
            ));
        }

        // System B Results
        report.push_str("\n## System B (Temporal Solver Net) Throughput Results\n\n");
        report.push_str("| Batch Size | Threads | Throughput (pred/sec) | Avg Latency (ms) | Memory (MB) | Error Rate |\n");
        report.push_str("|------------|---------|----------------------|------------------|-------------|------------|\n");

        for measurement in &system_b_measurements {
            report.push_str(&format!(
                "| {} | {} | {:.1} | {:.3} | {:.1} | {:.2}% |\n",
                measurement.batch_size,
                measurement.thread_count,
                measurement.throughput_pred_per_sec,
                measurement.avg_latency_ms,
                measurement.memory_usage_mb,
                measurement.error_rate * 100.0
            ));
        }

        // Peak Performance Analysis
        report.push_str("\n## Peak Performance Analysis\n\n");

        let peak_a = system_a_measurements
            .iter()
            .max_by(|a, b| a.throughput_pred_per_sec.partial_cmp(&b.throughput_pred_per_sec).unwrap())
            .unwrap();

        let peak_b = system_b_measurements
            .iter()
            .max_by(|a, b| a.throughput_pred_per_sec.partial_cmp(&b.throughput_pred_per_sec).unwrap())
            .unwrap();

        report.push_str(&format!("**System A Peak Performance:**\n"));
        report.push_str(&format!("- Throughput: {:.1} predictions/sec\n", peak_a.throughput_pred_per_sec));
        report.push_str(&format!("- Configuration: Batch size {}, {} threads\n", peak_a.batch_size, peak_a.thread_count));
        report.push_str(&format!("- Latency: {:.3}ms\n\n", peak_a.avg_latency_ms));

        report.push_str(&format!("**System B Peak Performance:**\n"));
        report.push_str(&format!("- Throughput: {:.1} predictions/sec\n", peak_b.throughput_pred_per_sec));
        report.push_str(&format!("- Configuration: Batch size {}, {} threads\n", peak_b.batch_size, peak_b.thread_count));
        report.push_str(&format!("- Latency: {:.3}ms\n\n", peak_b.avg_latency_ms));

        // Throughput Improvement Analysis
        let throughput_improvement = (peak_b.throughput_pred_per_sec - peak_a.throughput_pred_per_sec)
            / peak_a.throughput_pred_per_sec * 100.0;

        report.push_str("## Comparative Analysis\n\n");
        report.push_str(&format!("| Metric | System A | System B | Improvement |\n"));
        report.push_str(&format!("|--------|----------|----------|-------------|\n"));
        report.push_str(&format!("| Peak Throughput | {:.1} pred/sec | {:.1} pred/sec | {:.1}% |\n",
            peak_a.throughput_pred_per_sec, peak_b.throughput_pred_per_sec, throughput_improvement));
        report.push_str(&format!("| Best Latency | {:.3}ms | {:.3}ms | {:.1}% |\n",
            peak_a.avg_latency_ms, peak_b.avg_latency_ms,
            (peak_a.avg_latency_ms - peak_b.avg_latency_ms) / peak_a.avg_latency_ms * 100.0));

        report
    }
}

/// Benchmark throughput for different batch sizes
fn bench_batch_size_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let context = rt.block_on(async {
        ThroughputBenchmarkContext::new().expect("Failed to create context")
    });

    let mut group = c.benchmark_group("batch_throughput");

    for &batch_size in BATCH_SIZES {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("SystemA", batch_size),
            &batch_size,
            |b, &batch_size| {
                let test_input = DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| 0.5);
                b.iter(|| {
                    for _ in 0..batch_size {
                        black_box(context.system_a.forward(black_box(&test_input)).unwrap());
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("SystemB", batch_size),
            &batch_size,
            |b, &batch_size| {
                let test_input = DMatrix::from_fn(SEQUENCE_LENGTH, FEATURE_DIM, |_, _| 0.5);
                b.iter(|| {
                    for _ in 0..batch_size {
                        black_box(context.system_b.forward(black_box(&test_input)).unwrap());
                    }
                });
            },
        );
    }

    group.finish();
}

/// Comprehensive throughput analysis
fn bench_comprehensive_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    rt.block_on(async {
        let context = ThroughputBenchmarkContext::new()
            .expect("Failed to create benchmark context");

        let mut measurements = Vec::new();

        println!("Running comprehensive throughput benchmarks...");

        // Test all combinations of batch sizes and thread counts
        for &batch_size in BATCH_SIZES {
            for &thread_count in CONCURRENT_THREADS {
                println!("Testing batch_size={}, threads={}", batch_size, thread_count);

                // Test System A
                if thread_count == 1 {
                    let measurement_a = context.measure_single_threaded_throughput(
                        "SystemA", batch_size, 5
                    );
                    measurements.push(measurement_a);
                } else {
                    let measurement_a = context.measure_multi_threaded_throughput(
                        "SystemA", batch_size, thread_count, 5
                    );
                    measurements.push(measurement_a);
                }

                // Test System B
                if thread_count == 1 {
                    let measurement_b = context.measure_single_threaded_throughput(
                        "SystemB", batch_size, 5
                    );
                    measurements.push(measurement_b);
                } else {
                    let measurement_b = context.measure_multi_threaded_throughput(
                        "SystemB", batch_size, thread_count, 5
                    );
                    measurements.push(measurement_b);
                }
            }
        }

        // Generate and save report
        let report = context.generate_throughput_report(&measurements);
        std::fs::write("throughput_benchmark_report.md", report)
            .expect("Failed to save throughput report");

        println!("✅ Throughput benchmark completed!");
        println!("📊 Report saved to: throughput_benchmark_report.md");
    });
}

criterion_group!(
    name = throughput_benches;
    config = Criterion::default()
        .sample_size(100)
        .measurement_time(Duration::from_secs(30))
        .warm_up_time(Duration::from_secs(5));
    targets = bench_batch_size_throughput, bench_comprehensive_throughput
);
criterion_main!(throughput_benches);