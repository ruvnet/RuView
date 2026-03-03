//! Rust Integration Example for Temporal Neural Solver
//!
//! This example demonstrates how to integrate the Temporal Neural Solver
//! into Rust applications for ultra-low latency inference.

use std::time::Instant;
use std::path::Path;
use serde::{Deserialize, Serialize};
use nalgebra::{DVector, DMatrix};

// Re-export from the main crate
use temporal_neural_net::{
    models::{SystemA, SystemB, ModelTrait},
    config::{Config, ModelConfig, InferenceConfig},
    data::{TimeSeriesData, WindowedSample},
    inference::{Predictor, Prediction},
    export::ONNXExporter,
    error::{Result, TemporalNeuralError},
};

/// Configuration for the Rust integration example
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleConfig {
    /// Model configuration
    pub model: ModelConfig,
    /// Inference configuration
    pub inference: InferenceConfig,
    /// Example-specific settings
    pub example: ExampleSettings,
}

/// Example-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleSettings {
    /// Number of benchmark iterations
    pub benchmark_iterations: usize,
    /// Whether to enable detailed logging
    pub enable_logging: bool,
    /// Output directory for results
    pub output_dir: String,
}

impl Default for ExampleSettings {
    fn default() -> Self {
        Self {
            benchmark_iterations: 10000,
            enable_logging: true,
            output_dir: "output".to_string(),
        }
    }
}

/// Performance metrics for benchmarking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Number of samples processed
    pub num_samples: usize,
    /// Mean latency in milliseconds
    pub mean_latency_ms: f64,
    /// Standard deviation of latency
    pub std_latency_ms: f64,
    /// P99.9 latency in milliseconds
    pub p99_9_latency_ms: f64,
    /// Throughput in predictions per second
    pub throughput_pps: f64,
    /// Success rate percentage
    pub success_rate: f64,
    /// Whether sub-millisecond target was achieved
    pub sub_millisecond_achieved: bool,
}

/// Rust-native Temporal Neural Solver interface
pub struct RustTemporalSolver {
    /// The underlying neural network model
    model: Box<dyn ModelTrait>,
    /// Inference engine
    predictor: Predictor,
    /// Configuration
    config: Config,
}

impl RustTemporalSolver {
    /// Create a new Temporal Neural Solver instance
    pub fn new(config_path: &str) -> Result<Self> {
        // Load configuration
        let config = Config::from_file(config_path)?;

        // Create model based on configuration
        let model: Box<dyn ModelTrait> = match config.model.system_type.as_str() {
            "A" => Box::new(SystemA::new(config.model.clone())?),
            "B" => Box::new(SystemB::new(config.model.clone())?),
            _ => return Err(TemporalNeuralError::ConfigurationError {
                field: "system_type".to_string(),
                message: "Must be 'A' or 'B'".to_string(),
            }),
        };

        // Create predictor
        let predictor = Predictor::new(*model.clone(), config.inference.clone())?;

        println!("✅ Rust Temporal Neural Solver initialized");
        println!("   System type: {}", config.model.system_type);
        println!("   Architecture: {}", config.model.architecture);

        Ok(Self {
            model,
            predictor,
            config,
        })
    }

    /// Run a single prediction with timing
    pub fn predict_timed(&self, input: &DVector<f64>) -> Result<(Prediction, f64)> {
        let start = Instant::now();
        let prediction = self.predictor.predict(input)?;
        let elapsed = start.elapsed();
        let latency_ms = elapsed.as_secs_f64() * 1000.0;

        Ok((prediction, latency_ms))
    }

    /// Generate synthetic test data for demonstration
    pub fn generate_test_data(&self, sequence_length: usize) -> DVector<f64> {
        let mut data = Vec::new();

        for i in 0..sequence_length {
            let t = i as f64 / sequence_length as f64;

            // Generate sinusoidal trajectory with noise
            let x = (2.0 * std::f64::consts::PI * t).sin() + self.random_noise(0.1);
            let y = (2.0 * std::f64::consts::PI * t).cos() + self.random_noise(0.1);
            let vx = 2.0 * std::f64::consts::PI * (2.0 * std::f64::consts::PI * t).cos() + self.random_noise(0.05);
            let vy = -2.0 * std::f64::consts::PI * (2.0 * std::f64::consts::PI * t).sin() + self.random_noise(0.05);

            data.extend_from_slice(&[x, y, vx, vy]);
        }

        DVector::from_vec(data)
    }

    /// Generate random noise (simplified - in practice use proper RNG)
    fn random_noise(&self, std_dev: f64) -> f64 {
        // Simplified Box-Muller transform for demonstration
        use std::f64::consts::PI;
        static mut U1: f64 = 0.0;
        static mut U2: f64 = 0.0;
        static mut CACHED: bool = false;

        unsafe {
            if CACHED {
                CACHED = false;
                std_dev * (-2.0 * U1.ln()).sqrt() * (2.0 * PI * U2).sin()
            } else {
                U1 = fastrand::f64();
                U2 = fastrand::f64();
                CACHED = true;
                std_dev * (-2.0 * U1.ln()).sqrt() * (2.0 * PI * U2).cos()
            }
        }
    }

    /// Run comprehensive benchmark
    pub fn benchmark(&self, num_iterations: usize) -> Result<PerformanceMetrics> {
        println!("🏃‍♂️ Running Rust benchmark ({} iterations)...", num_iterations);

        let mut latencies = Vec::with_capacity(num_iterations);
        let mut successes = 0;

        // Warmup
        println!("🔥 Warming up...");
        for _ in 0..100 {
            let test_data = self.generate_test_data(10);
            let _ = self.predictor.predict(&test_data);
        }

        // Benchmark loop
        println!("⏱️  Measuring performance...");
        let benchmark_start = Instant::now();

        for i in 0..num_iterations {
            if i % 1000 == 0 && i > 0 {
                println!("   Progress: {}/{}", i, num_iterations);
            }

            let test_data = self.generate_test_data(10);

            match self.predict_timed(&test_data) {
                Ok((_, latency_ms)) => {
                    latencies.push(latency_ms);
                    successes += 1;
                },
                Err(_) => {
                    // Count failures but continue
                }
            }
        }

        let total_time = benchmark_start.elapsed();

        // Calculate statistics
        if latencies.is_empty() {
            return Err(TemporalNeuralError::BenchmarkError {
                message: "No successful predictions in benchmark".to_string(),
            });
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean_latency = latencies.iter().sum::<f64>() / latencies.len() as f64;
        let variance = latencies.iter()
            .map(|x| (x - mean_latency).powi(2))
            .sum::<f64>() / latencies.len() as f64;
        let std_latency = variance.sqrt();

        let p99_9_index = ((latencies.len() as f64 * 0.999) as usize).min(latencies.len() - 1);
        let p99_9_latency = latencies[p99_9_index];

        let throughput = latencies.len() as f64 / total_time.as_secs_f64();
        let success_rate = (successes as f64 / num_iterations as f64) * 100.0;

        let metrics = PerformanceMetrics {
            num_samples: latencies.len(),
            mean_latency_ms: mean_latency,
            std_latency_ms: std_latency,
            p99_9_latency_ms: p99_9_latency,
            throughput_pps: throughput,
            success_rate,
            sub_millisecond_achieved: p99_9_latency < 1.0,
        };

        println!("✅ Benchmark complete!");
        println!("   Mean latency: {:.3}ms", metrics.mean_latency_ms);
        println!("   P99.9 latency: {:.3}ms", metrics.p99_9_latency_ms);
        println!("   Throughput: {:.0} pps", metrics.throughput_pps);
        println!("   Success rate: {:.1}%", metrics.success_rate);
        println!("   Sub-millisecond: {}", if metrics.sub_millisecond_achieved { "✅" } else { "❌" });

        Ok(metrics)
    }

    /// Export model to ONNX format
    pub fn export_to_onnx(&self, output_path: &str) -> Result<()> {
        println!("📤 Exporting to ONNX: {}", output_path);

        let exporter = ONNXExporter::new();

        match self.config.model.system_type.as_str() {
            "A" => {
                if let Ok(system_a) = self.model.as_any().downcast_ref::<SystemA>() {
                    exporter.export_system_a(system_a, output_path)?;
                }
            },
            "B" => {
                if let Ok(system_b) = self.model.as_any().downcast_ref::<SystemB>() {
                    exporter.export_system_b(system_b, output_path)?;
                }
            },
            _ => return Err(TemporalNeuralError::ExportError {
                message: "Unknown system type for export".to_string(),
            }),
        }

        println!("✅ ONNX export complete: {}", output_path);
        Ok(())
    }

    /// Save benchmark results to file
    pub fn save_benchmark_results(&self, metrics: &PerformanceMetrics, path: &str) -> Result<()> {
        let json = serde_json::to_string_pretty(metrics)
            .map_err(|e| TemporalNeuralError::SerializationError {
                message: format!("Failed to serialize metrics: {}", e),
            })?;

        std::fs::write(path, json)
            .map_err(|e| TemporalNeuralError::IoError {
                operation: "write_benchmark_results".to_string(),
                path: path.into(),
                source: e,
            })?;

        println!("📄 Benchmark results saved: {}", path);
        Ok(())
    }
}

/// Demonstration functions
pub mod demo {
    use super::*;

    /// Basic usage demonstration
    pub fn basic_usage() -> Result<()> {
        println!("🎯 Basic Usage Demo");
        println!("==================");

        // Load configuration (you would typically load from file)
        let config = Config::default_system_b();

        // Create solver
        let solver = RustTemporalSolver::new("configs/B_temporal_solver.yaml")
            .or_else(|_| {
                // Fallback to in-memory config if file doesn't exist
                println!("⚠️  Config file not found, using default configuration");
                Ok(RustTemporalSolver {
                    model: Box::new(SystemB::new(config.model.clone())?),
                    predictor: Predictor::new(
                        SystemB::new(config.model.clone())?,
                        config.inference.clone()
                    )?,
                    config,
                })
            })?;

        // Generate test data
        let test_input = solver.generate_test_data(10);
        println!("📊 Generated test input with {} elements", test_input.len());

        // Run single prediction
        let (prediction, latency) = solver.predict_timed(&test_input)?;

        println!("✅ Prediction complete:");
        println!("   Result: {:?}", prediction.value.as_slice());
        println!("   Latency: {:.3}ms", latency);
        println!("   Certificate error: {:.6}", prediction.certificate.error);
        println!("   Sub-millisecond: {}", if latency < 1.0 { "✅" } else { "❌" });

        Ok(())
    }

    /// Performance benchmark demonstration
    pub fn benchmark_demo() -> Result<()> {
        println!("\n📊 Benchmark Demo");
        println!("=================");

        let config = Config::default_system_b();
        let solver = RustTemporalSolver::new("configs/B_temporal_solver.yaml")
            .or_else(|_| {
                println!("⚠️  Using default configuration");
                Ok(RustTemporalSolver {
                    model: Box::new(SystemB::new(config.model.clone())?),
                    predictor: Predictor::new(
                        SystemB::new(config.model.clone())?,
                        config.inference.clone()
                    )?,
                    config,
                })
            })?;

        // Run benchmark
        let metrics = solver.benchmark(5000)?; // Reduced for demo

        // Save results
        solver.save_benchmark_results(&metrics, "rust_benchmark_results.json")?;

        println!("\n🏆 Benchmark Summary:");
        println!("   Samples: {}", metrics.num_samples);
        println!("   Mean latency: {:.3}ms ± {:.3}ms", metrics.mean_latency_ms, metrics.std_latency_ms);
        println!("   P99.9 latency: {:.3}ms", metrics.p99_9_latency_ms);
        println!("   Throughput: {:.0} predictions/second", metrics.throughput_pps);
        println!("   Success rate: {:.1}%", metrics.success_rate);

        println!("\n🎯 Success Criteria:");
        println!("   Sub-millisecond P99.9: {}", if metrics.sub_millisecond_achieved { "✅" } else { "❌" });
        println!("   Target 0.9ms P99.9: {}", if metrics.p99_9_latency_ms < 0.9 { "✅" } else { "❌" });
        println!("   High success rate: {}", if metrics.success_rate > 99.0 { "✅" } else { "❌" });

        Ok(())
    }

    /// ONNX export demonstration
    pub fn onnx_export_demo() -> Result<()> {
        println!("\n📤 ONNX Export Demo");
        println!("===================");

        let config = Config::default_system_b();
        let solver = RustTemporalSolver::new("configs/B_temporal_solver.yaml")
            .or_else(|_| {
                println!("⚠️  Using default configuration");
                Ok(RustTemporalSolver {
                    model: Box::new(SystemB::new(config.model.clone())?),
                    predictor: Predictor::new(
                        SystemB::new(config.model.clone())?,
                        config.inference.clone()
                    )?,
                    config,
                })
            })?;

        // Export to ONNX
        solver.export_to_onnx("temporal_solver_rust_export.onnx")?;

        println!("✅ ONNX export demonstration complete");
        println!("   File: temporal_solver_rust_export.onnx");
        println!("   Ready for deployment with ONNX Runtime");

        Ok(())
    }

    /// Real-time simulation demonstration
    pub fn realtime_simulation() -> Result<()> {
        println!("\n⚡ Real-time Simulation Demo");
        println!("===========================");

        let config = Config::default_system_b();
        let solver = RustTemporalSolver::new("configs/B_temporal_solver.yaml")
            .or_else(|_| {
                println!("⚠️  Using default configuration");
                Ok(RustTemporalSolver {
                    model: Box::new(SystemB::new(config.model.clone())?),
                    predictor: Predictor::new(
                        SystemB::new(config.model.clone())?,
                        config.inference.clone()
                    )?,
                    config,
                })
            })?;

        println!("🎮 Simulating real-time inference loop...");

        let mut total_latency = 0.0;
        let mut max_latency = 0.0;
        let simulation_steps = 100;

        for step in 0..simulation_steps {
            // Generate "sensor data"
            let sensor_data = solver.generate_test_data(10);

            // Run prediction (simulating real-time requirement)
            let (prediction, latency) = solver.predict_timed(&sensor_data)?;

            total_latency += latency;
            max_latency = max_latency.max(latency);

            // Simulate real-time constraints
            if latency > 1.0 {
                println!("⚠️  Step {}: Latency exceeded 1ms ({:.3}ms)", step, latency);
            }

            if step % 20 == 0 {
                println!("   Step {}: {:.3}ms latency", step, latency);
            }

            // Simulate 10ms control loop
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        let avg_latency = total_latency / simulation_steps as f64;

        println!("✅ Real-time simulation complete:");
        println!("   Steps: {}", simulation_steps);
        println!("   Average latency: {:.3}ms", avg_latency);
        println!("   Maximum latency: {:.3}ms", max_latency);
        println!("   Real-time capable: {}", if max_latency < 1.0 { "✅" } else { "❌" });

        Ok(())
    }
}

/// Main entry point for Rust integration example
fn main() -> Result<()> {
    println!("🚀 Temporal Neural Solver - Rust Integration Example");
    println!("=====================================================");

    // Initialize logging
    env_logger::init();

    // Run all demonstrations
    demo::basic_usage()?;
    demo::benchmark_demo()?;
    demo::onnx_export_demo()?;
    demo::realtime_simulation()?;

    println!("\n🎉 Rust integration example complete!");
    println!("\n💡 Integration Tips:");
    println!("   • Use RustTemporalSolver for high-performance applications");
    println!("   • Export to ONNX for cross-platform deployment");
    println!("   • Monitor latency in production with predict_timed()");
    println!("   • Implement proper error handling for production use");
    println!("   • Consider async patterns for concurrent inference");

    println!("\n📚 Next Steps:");
    println!("   • Integrate into your Rust application");
    println!("   • Customize for your specific data format");
    println!("   • Implement production monitoring and alerting");
    println!("   • Consider GPU acceleration for batch processing");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solver_creation() {
        // Test with default configuration
        let config = Config::default_system_b();

        let result = RustTemporalSolver {
            model: Box::new(SystemB::new(config.model.clone()).unwrap()),
            predictor: Predictor::new(
                SystemB::new(config.model.clone()).unwrap(),
                config.inference.clone()
            ).unwrap(),
            config,
        };

        // Basic validation
        assert!(true); // If we get here, creation succeeded
    }

    #[test]
    fn test_data_generation() {
        let config = Config::default_system_b();
        let solver = RustTemporalSolver {
            model: Box::new(SystemB::new(config.model.clone()).unwrap()),
            predictor: Predictor::new(
                SystemB::new(config.model.clone()).unwrap(),
                config.inference.clone()
            ).unwrap(),
            config,
        };

        let data = solver.generate_test_data(10);
        assert_eq!(data.len(), 40); // 10 timesteps * 4 features
    }

    #[test]
    fn test_benchmark_structure() {
        // Test with minimal iterations for fast testing
        let config = Config::default_system_b();
        let solver = RustTemporalSolver {
            model: Box::new(SystemB::new(config.model.clone()).unwrap()),
            predictor: Predictor::new(
                SystemB::new(config.model.clone()).unwrap(),
                config.inference.clone()
            ).unwrap(),
            config,
        };

        let metrics = solver.benchmark(10).unwrap();

        assert!(metrics.num_samples > 0);
        assert!(metrics.mean_latency_ms > 0.0);
        assert!(metrics.success_rate > 0.0);
    }
}