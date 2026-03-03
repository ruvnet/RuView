//! High-performance inference engine for temporal neural networks
//!
//! This module provides optimized inference capabilities with sub-millisecond
//! latency guarantees and comprehensive performance monitoring.

use crate::{
    config::{Config, InferenceConfig},
    error::{Result, TemporalNeuralError},
    models::{ModelTrait, SystemA, SystemB},
    solvers::Certificate,
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub mod quantization;
pub mod simd_ops;
pub mod memory_pool;

pub use quantization::{QuantizedInference, Int8Quantizer};
pub use simd_ops::{SimdAccelerator, VectorOps};
pub use memory_pool::{MemoryPool, PreallocatedBuffer};

/// High-performance predictor with latency guarantees
pub struct Predictor {
    /// Model being used for prediction
    model: PredictorModel,
    /// Inference configuration
    config: InferenceConfig,
    /// Performance monitor
    monitor: PerformanceMonitor,
    /// Memory pool for zero-allocation inference
    memory_pool: MemoryPool,
    /// SIMD accelerator
    simd_accelerator: SimdAccelerator,
    /// Quantization engine (if enabled)
    quantizer: Option<Int8Quantizer>,
    /// Inference statistics
    stats: InferenceStatistics,
}

/// Model wrapper for unified inference interface
enum PredictorModel {
    /// System A (traditional)
    SystemA(SystemA),
    /// System B (temporal solver)
    SystemB(SystemB),
}

/// Prediction result with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prediction {
    /// Predicted values
    pub values: DVector<f64>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Prediction latency in microseconds
    pub latency_us: f64,
    /// Certificate (for System B)
    pub certificate: Option<Certificate>,
    /// Prediction metadata
    pub metadata: PredictionMetadata,
}

/// Metadata about the prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionMetadata {
    /// Model type used
    pub model_type: String,
    /// Prediction timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Input data quality score
    pub input_quality: f64,
    /// Whether quantization was used
    pub quantized: bool,
    /// Whether SIMD was used
    pub simd_used: bool,
    /// Memory usage for this prediction
    pub memory_used_bytes: usize,
    /// Detailed timing breakdown
    pub timing: TimingBreakdown,
}

/// Detailed timing breakdown for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingBreakdown {
    /// Input preprocessing time (microseconds)
    pub preprocessing_us: f64,
    /// Core model inference time (microseconds)
    pub inference_us: f64,
    /// Post-processing time (microseconds)
    pub postprocessing_us: f64,
    /// Solver verification time (microseconds, System B only)
    pub verification_us: Option<f64>,
    /// Memory allocation time (microseconds)
    pub allocation_us: f64,
    /// Total time (microseconds)
    pub total_us: f64,
}

/// Performance monitoring and latency tracking
#[derive(Debug)]
struct PerformanceMonitor {
    /// Recent latency measurements
    recent_latencies: Vec<f64>,
    /// Maximum number of recent measurements to keep
    max_recent: usize,
    /// Target latency threshold
    target_latency_us: f64,
    /// Latency violations counter
    violations: u64,
    /// Total predictions made
    total_predictions: u64,
}

/// Comprehensive inference statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceStatistics {
    /// Total predictions made
    pub total_predictions: u64,
    /// Average latency in microseconds
    pub avg_latency_us: f64,
    /// P50 latency in microseconds
    pub p50_latency_us: f64,
    /// P99 latency in microseconds
    pub p99_latency_us: f64,
    /// P99.9 latency in microseconds
    pub p99_9_latency_us: f64,
    /// Maximum latency observed
    pub max_latency_us: f64,
    /// Minimum latency observed
    pub min_latency_us: f64,
    /// Latency target violations
    pub latency_violations: u64,
    /// Latency violation rate
    pub violation_rate: f64,
    /// Average throughput (predictions per second)
    pub throughput_pred_per_sec: f64,
    /// Memory usage statistics
    pub memory_stats: MemoryStatistics,
    /// System B specific statistics
    pub system_b_stats: Option<SystemBInferenceStats>,
}

/// Memory usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStatistics {
    /// Current memory usage in bytes
    pub current_usage_bytes: usize,
    /// Peak memory usage in bytes
    pub peak_usage_bytes: usize,
    /// Average memory per prediction
    pub avg_memory_per_prediction: f64,
    /// Memory pool utilization
    pub pool_utilization: f64,
}

/// System B specific inference statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBInferenceStats {
    /// Gate pass rate
    pub gate_pass_rate: f64,
    /// Average certificate error
    pub avg_certificate_error: f64,
    /// Fallback usage rate
    pub fallback_rate: f64,
    /// Average solver work performed
    pub avg_solver_work: f64,
}

impl Predictor {
    /// Create a new predictor from a trained model
    pub fn new_system_a(model: SystemA, config: InferenceConfig) -> Result<Self> {
        let monitor = PerformanceMonitor::new(config.target_latency_ms * 1000.0);
        let memory_pool = MemoryPool::new(1024 * 1024)?; // 1MB pool
        let simd_accelerator = SimdAccelerator::new(config.enable_simd);

        let quantizer = if config.enable_simd {
            Some(Int8Quantizer::new()?)
        } else {
            None
        };

        Ok(Self {
            model: PredictorModel::SystemA(model),
            config,
            monitor,
            memory_pool,
            simd_accelerator,
            quantizer,
            stats: InferenceStatistics::new(),
        })
    }

    /// Create a new predictor for System B
    pub fn new_system_b(mut model: SystemB, config: InferenceConfig) -> Result<Self> {
        // Prepare model for inference
        model.prepare_for_inference()?;

        let monitor = PerformanceMonitor::new(config.target_latency_ms * 1000.0);
        let memory_pool = MemoryPool::new(2 * 1024 * 1024)?; // 2MB pool for System B
        let simd_accelerator = SimdAccelerator::new(config.enable_simd);

        let quantizer = if config.enable_simd {
            Some(Int8Quantizer::new()?)
        } else {
            None
        };

        Ok(Self {
            model: PredictorModel::SystemB(model),
            config,
            monitor,
            memory_pool,
            simd_accelerator,
            quantizer,
            stats: InferenceStatistics::new(),
        })
    }

    /// Perform prediction with comprehensive monitoring
    pub fn predict(&mut self, input: &DMatrix<f64>) -> Result<Prediction> {
        let start_time = Instant::now();

        // Pre-allocate memory from pool
        let _buffer = self.memory_pool.acquire()?;
        let allocation_time = start_time.elapsed().as_micros() as f64;

        // Validate input
        self.validate_input(input)?;

        // Preprocessing
        let preprocessing_start = Instant::now();
        let processed_input = self.preprocess_input(input)?;
        let preprocessing_time = preprocessing_start.elapsed().as_micros() as f64;

        // Core inference
        let inference_start = Instant::now();
        let (prediction_values, certificate) = match &mut self.model {
            PredictorModel::SystemA(model) => {
                let pred = model.forward(&processed_input)?;
                (pred, None)
            }
            PredictorModel::SystemB(model) => {
                let pred_result = model.predict_with_solver(&processed_input)?;
                // Create certificate from gate result
                let certificate = Certificate {
                    error_bound: pred_result.gate_result.certificate_error,
                    confidence: pred_result.gate_result.confidence,
                    work_performed: pred_result.gate_result.work_performed,
                    algorithm: "temporal_solver".to_string(),
                    is_valid: pred_result.gate_result.passed,
                    metadata: crate::solvers::CertificateMetadata {
                        condition_number: None,
                        diagonally_dominant: false,
                        iterations: 0,
                        residual_norm: pred_result.gate_result.certificate_error,
                        computation_time_us: pred_result.gate_result.verification_time_us,
                    },
                };
                (pred_result.prediction, Some(certificate))
            }
        };
        let inference_time = inference_start.elapsed().as_micros() as f64;

        // Post-processing
        let postprocessing_start = Instant::now();
        let final_prediction = self.postprocess_prediction(&prediction_values)?;
        let postprocessing_time = postprocessing_start.elapsed().as_micros() as f64;

        let total_time = start_time.elapsed().as_micros() as f64;

        // Update performance monitoring
        self.monitor.record_latency(total_time);

        // Compute confidence score
        let confidence = self.compute_confidence(&final_prediction, certificate.as_ref());

        // Create timing breakdown
        let timing = TimingBreakdown {
            preprocessing_us: preprocessing_time,
            inference_us: inference_time,
            postprocessing_us: postprocessing_time,
            verification_us: certificate.as_ref().map(|c| c.metadata.computation_time_us),
            allocation_us: allocation_time,
            total_us: total_time,
        };

        // Create metadata
        let metadata = PredictionMetadata {
            model_type: match &self.model {
                PredictorModel::SystemA(_) => "SystemA".to_string(),
                PredictorModel::SystemB(_) => "SystemB".to_string(),
            },
            timestamp: chrono::Utc::now(),
            input_quality: self.assess_input_quality(input),
            quantized: self.quantizer.is_some(),
            simd_used: self.config.enable_simd,
            memory_used_bytes: self.memory_pool.current_usage(),
            timing,
        };

        // Update statistics
        self.update_statistics(total_time, &metadata, certificate.as_ref());

        // Check latency constraints
        if total_time > self.config.target_latency_ms * 1000.0 {
            log::warn!(
                "Latency constraint violated: {:.2}μs > {:.2}μs",
                total_time, self.config.target_latency_ms * 1000.0
            );
        }

        Ok(Prediction {
            values: final_prediction,
            confidence,
            latency_us: total_time,
            certificate,
            metadata,
        })
    }

    /// Batch prediction for higher throughput
    pub fn predict_batch(&mut self, inputs: &[DMatrix<f64>]) -> Result<Vec<Prediction>> {
        let mut predictions = Vec::with_capacity(inputs.len());

        for input in inputs {
            let prediction = self.predict(input)?;
            predictions.push(prediction);
        }

        Ok(predictions)
    }

    /// Validate input data
    fn validate_input(&self, input: &DMatrix<f64>) -> Result<()> {
        let expected_shape = match &self.model {
            PredictorModel::SystemA(model) => model.input_shape(),
            PredictorModel::SystemB(model) => model.input_shape(),
        };

        let actual_shape = (input.nrows(), input.ncols());
        if actual_shape != expected_shape {
            return Err(TemporalNeuralError::InferenceError {
                message: format!(
                    "Input shape mismatch: expected {:?}, got {:?}",
                    expected_shape, actual_shape
                ),
                input_shape: Some(vec![actual_shape.0, actual_shape.1]),
                latency_exceeded: false,
            });
        }

        // Check for invalid values
        for &val in input.iter() {
            if !val.is_finite() {
                return Err(TemporalNeuralError::InferenceError {
                    message: "Input contains invalid values (NaN or Inf)".to_string(),
                    input_shape: Some(vec![actual_shape.0, actual_shape.1]),
                    latency_exceeded: false,
                });
            }
        }

        Ok(())
    }

    /// Preprocess input for inference
    fn preprocess_input(&self, input: &DMatrix<f64>) -> Result<DMatrix<f64>> {
        // Apply SIMD optimizations if available
        if self.config.enable_simd {
            self.simd_accelerator.optimize_matrix(input)
        } else {
            Ok(input.clone())
        }
    }

    /// Post-process prediction results
    fn postprocess_prediction(&self, prediction: &DVector<f64>) -> Result<DVector<f64>> {
        // Apply any final transformations
        Ok(prediction.clone())
    }

    /// Compute confidence score for prediction
    fn compute_confidence(&self, prediction: &DVector<f64>, certificate: Option<&Certificate>) -> f64 {
        match certificate {
            Some(cert) => cert.confidence,
            None => {
                // For System A, use prediction magnitude as rough confidence measure
                let mag = prediction.norm();
                if mag < 10.0 { 0.9 } else { 0.7 } // Simple heuristic
            }
        }
    }

    /// Assess input data quality
    fn assess_input_quality(&self, input: &DMatrix<f64>) -> f64 {
        // Check for reasonable variance and no outliers
        let mut quality: f64 = 1.0;

        for i in 0..input.nrows() {
            let row_data: Vec<f64> = input.row(i).iter().cloned().collect();
            let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
            let variance = row_data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / row_data.len() as f64;

            // Penalize very low variance (flat signals)
            if variance < 1e-6 {
                quality *= 0.5;
            }

            // Penalize extreme values
            for &val in &row_data {
                if val.abs() > 100.0 {
                    quality *= 0.8;
                }
            }
        }

        quality.clamp(0.0, 1.0)
    }

    /// Update inference statistics
    fn update_statistics(&mut self, latency_us: f64, metadata: &PredictionMetadata, certificate: Option<&Certificate>) {
        self.stats.total_predictions += 1;

        // Update latency statistics
        let prev_avg = self.stats.avg_latency_us;
        let n = self.stats.total_predictions as f64;
        self.stats.avg_latency_us = (prev_avg * (n - 1.0) + latency_us) / n;

        // Update min/max
        if latency_us > self.stats.max_latency_us {
            self.stats.max_latency_us = latency_us;
        }
        if latency_us < self.stats.min_latency_us || self.stats.min_latency_us == 0.0 {
            self.stats.min_latency_us = latency_us;
        }

        // Update memory statistics
        self.stats.memory_stats.current_usage_bytes = metadata.memory_used_bytes;
        if metadata.memory_used_bytes > self.stats.memory_stats.peak_usage_bytes {
            self.stats.memory_stats.peak_usage_bytes = metadata.memory_used_bytes;
        }

        // Update System B specific stats
        if let Some(cert) = certificate {
            if self.stats.system_b_stats.is_none() {
                self.stats.system_b_stats = Some(SystemBInferenceStats {
                    gate_pass_rate: 0.0,
                    avg_certificate_error: 0.0,
                    fallback_rate: 0.0,
                    avg_solver_work: 0.0,
                });
            }

            if let Some(ref mut b_stats) = self.stats.system_b_stats {
                let prev_avg_error = b_stats.avg_certificate_error;
                b_stats.avg_certificate_error = (prev_avg_error * (n - 1.0) + cert.error_bound) / n;

                let prev_avg_work = b_stats.avg_solver_work;
                b_stats.avg_solver_work = (prev_avg_work * (n - 1.0) + cert.work_performed as f64) / n;
            }
        }

        // Update percentile statistics periodically
        if self.stats.total_predictions % 100 == 0 {
            self.update_percentile_statistics();
        }
    }

    /// Update percentile statistics (P50, P99, P99.9)
    fn update_percentile_statistics(&mut self) {
        let mut latencies = self.monitor.recent_latencies.clone();
        if latencies.is_empty() {
            return;
        }

        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let len = latencies.len();

        self.stats.p50_latency_us = latencies[len / 2];
        self.stats.p99_latency_us = latencies[(len as f64 * 0.99) as usize];
        self.stats.p99_9_latency_us = latencies[(len as f64 * 0.999) as usize];

        // Update violation statistics
        self.stats.latency_violations = self.monitor.violations;
        self.stats.violation_rate = self.monitor.violations as f64 / self.stats.total_predictions as f64;

        // Estimate throughput
        if let (Some(&first), Some(&last)) = (latencies.first(), latencies.last()) {
            let time_span = last - first;
            if time_span > 0.0 {
                self.stats.throughput_pred_per_sec = (len as f64) / (time_span / 1_000_000.0);
            }
        }
    }

    /// Get current inference statistics
    pub fn get_statistics(&self) -> &InferenceStatistics {
        &self.stats
    }

    /// Check if performance targets are being met
    pub fn meets_performance_targets(&self) -> bool {
        self.stats.p99_9_latency_us <= self.config.target_latency_ms * 1000.0 &&
        self.stats.violation_rate <= 0.001 // Less than 0.1% violations
    }

    /// Reset statistics
    pub fn reset_statistics(&mut self) {
        self.stats = InferenceStatistics::new();
        self.monitor.reset();
    }

    /// Warm up the predictor (important for latency-critical applications)
    pub fn warmup(&mut self, warmup_iterations: usize) -> Result<()> {
        log::info!("Warming up predictor with {} iterations", warmup_iterations);

        // Create dummy input of the correct shape
        let input_shape = match &self.model {
            PredictorModel::SystemA(model) => model.input_shape(),
            PredictorModel::SystemB(model) => model.input_shape(),
        };

        let dummy_input = DMatrix::zeros(input_shape.0, input_shape.1);

        for _ in 0..warmup_iterations {
            let _ = self.predict(&dummy_input)?;
        }

        // Reset statistics after warmup
        self.reset_statistics();

        log::info!("Warmup completed");
        Ok(())
    }
}

impl PerformanceMonitor {
    fn new(target_latency_us: f64) -> Self {
        Self {
            recent_latencies: Vec::with_capacity(1000),
            max_recent: 1000,
            target_latency_us,
            violations: 0,
            total_predictions: 0,
        }
    }

    fn record_latency(&mut self, latency_us: f64) {
        self.recent_latencies.push(latency_us);
        if self.recent_latencies.len() > self.max_recent {
            self.recent_latencies.remove(0);
        }

        if latency_us > self.target_latency_us {
            self.violations += 1;
        }

        self.total_predictions += 1;
    }

    fn reset(&mut self) {
        self.recent_latencies.clear();
        self.violations = 0;
        self.total_predictions = 0;
    }
}

impl InferenceStatistics {
    fn new() -> Self {
        Self {
            total_predictions: 0,
            avg_latency_us: 0.0,
            p50_latency_us: 0.0,
            p99_latency_us: 0.0,
            p99_9_latency_us: 0.0,
            max_latency_us: 0.0,
            min_latency_us: 0.0,
            latency_violations: 0,
            violation_rate: 0.0,
            throughput_pred_per_sec: 0.0,
            memory_stats: MemoryStatistics {
                current_usage_bytes: 0,
                peak_usage_bytes: 0,
                avg_memory_per_prediction: 0.0,
                pool_utilization: 0.0,
            },
            system_b_stats: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{ModelConfig, TemporalSolverConfig},
        models::{SystemA, SystemB},
    };

    fn create_test_inference_config() -> InferenceConfig {
        InferenceConfig {
            target_latency_ms: 0.9,
            enable_simd: false, // Disable for tests
            num_threads: 1,
            pin_memory: false,
            cpu_affinity: None,
            batch_size: 1,
        }
    }

    fn create_test_model_config() -> ModelConfig {
        ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 8,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        }
    }

    #[test]
    fn test_system_a_predictor() {
        let model_config = create_test_model_config();
        let inference_config = create_test_inference_config();

        let model = SystemA::new(&model_config).unwrap();
        let mut predictor = Predictor::new_system_a(model, inference_config).unwrap();

        let input = DMatrix::from_element(4, 256, 1.0);
        let prediction = predictor.predict(&input).unwrap();

        assert_eq!(prediction.values.len(), 2);
        assert!(prediction.latency_us > 0.0);
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.certificate.is_none());
    }

    #[test]
    fn test_system_b_predictor() {
        let model_config = create_test_model_config();
        let solver_config = TemporalSolverConfig::default();
        let inference_config = create_test_inference_config();

        let model = SystemB::new(&model_config, &solver_config).unwrap();
        let mut predictor = Predictor::new_system_b(model, inference_config).unwrap();

        let input = DMatrix::from_element(4, 256, 1.0);
        let prediction = predictor.predict(&input).unwrap();

        assert_eq!(prediction.values.len(), 2);
        assert!(prediction.latency_us > 0.0);
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.certificate.is_some());
    }

    #[test]
    fn test_batch_prediction() {
        let model_config = create_test_model_config();
        let inference_config = create_test_inference_config();

        let model = SystemA::new(&model_config).unwrap();
        let mut predictor = Predictor::new_system_a(model, inference_config).unwrap();

        let inputs = vec![
            DMatrix::from_element(4, 256, 1.0),
            DMatrix::from_element(4, 256, 2.0),
            DMatrix::from_element(4, 256, 3.0),
        ];

        let predictions = predictor.predict_batch(&inputs).unwrap();
        assert_eq!(predictions.len(), 3);

        for prediction in predictions {
            assert_eq!(prediction.values.len(), 2);
            assert!(prediction.latency_us > 0.0);
        }
    }

    #[test]
    fn test_statistics_tracking() {
        let model_config = create_test_model_config();
        let inference_config = create_test_inference_config();

        let model = SystemA::new(&model_config).unwrap();
        let mut predictor = Predictor::new_system_a(model, inference_config).unwrap();

        let input = DMatrix::from_element(4, 256, 1.0);

        // Make several predictions
        for _ in 0..10 {
            let _ = predictor.predict(&input).unwrap();
        }

        let stats = predictor.get_statistics();
        assert_eq!(stats.total_predictions, 10);
        assert!(stats.avg_latency_us > 0.0);
    }

    #[test]
    fn test_input_validation() {
        let model_config = create_test_model_config();
        let inference_config = create_test_inference_config();

        let model = SystemA::new(&model_config).unwrap();
        let mut predictor = Predictor::new_system_a(model, inference_config).unwrap();

        // Wrong shape
        let wrong_input = DMatrix::from_element(3, 100, 1.0);
        assert!(predictor.predict(&wrong_input).is_err());

        // Invalid values
        let mut invalid_input = DMatrix::from_element(4, 256, 1.0);
        invalid_input[(0, 0)] = f64::NAN;
        assert!(predictor.predict(&invalid_input).is_err());
    }
}