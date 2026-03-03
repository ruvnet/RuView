//! WASM bindings for temporal neural network
//!
//! This module provides WebAssembly bindings for the temporal neural network,
//! enabling sub-millisecond neural inference in web browsers and Node.js.

use wasm_bindgen::prelude::*;
use js_sys::{Array, Object, Reflect};
use web_sys::console;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    config::{Config, ModelConfig, TrainingConfig, InferenceConfig},
    error::{Result, TemporalNeuralError},
};

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console::log_1(&"Temporal Neural Solver initialized".into());
}

/// WASM wrapper for temporal neural network configuration
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmConfig {
    inner: Config,
}

#[wasm_bindgen]
impl WasmConfig {
    /// Create a new configuration from JSON
    #[wasm_bindgen(constructor)]
    pub fn new(json_config: &str) -> Result<WasmConfig, JsValue> {
        let config: Config = serde_json::from_str(json_config)
            .map_err(|e| JsValue::from_str(&format!("Invalid config: {}", e)))?;
        Ok(WasmConfig { inner: config })
    }

    /// Create default configuration for System A
    #[wasm_bindgen(js_name = systemA)]
    pub fn system_a() -> WasmConfig {
        // Create default System A config - simplified for WASM demo
        let config = Config {
            common: crate::config::CommonConfig {
                horizon_ms: 100,
                window_ms: 256,
                sample_rate_hz: 1000,
                features: vec!["x".to_string(), "y".to_string(), "vx".to_string(), "vy".to_string()],
                quantize: true,
                random_seed: Some(42),
                verbose: false,
            },
            model: ModelConfig {
                model_type: "micro_gru".to_string(),
                hidden_size: 32,
                num_layers: 2,
                dropout: 0.1,
                residual: true,
                activation: "tanh".to_string(),
                layer_norm: false,
            },
            training: TrainingConfig {
                optimizer: "adam".to_string(),
                learning_rate: 0.001,
                batch_size: 32,
                epochs: 100,
                patience: 10,
                val_frequency: 5,
                grad_clip: Some(1.0),
                weight_decay: 0.0001,
                smoothness_weight: 0.1,
                checkpoint_frequency: 10,
            },
            inference: InferenceConfig {
                target_latency_ms: 0.9,
                enable_simd: false, // Disabled for WASM compatibility
                num_threads: 1,
                pin_memory: false,
                cpu_affinity: None,
                batch_size: 1,
            },
            system: crate::config::SystemConfig::Traditional(crate::config::TraditionalConfig {
                enabled: true,
            }),
        };
        WasmConfig { inner: config }
    }

    /// Create default configuration for System B (temporal solver)
    #[wasm_bindgen(js_name = systemB)]
    pub fn system_b() -> WasmConfig {
        // Create default System B config - simplified for WASM demo
        let config = Config {
            common: crate::config::CommonConfig {
                horizon_ms: 100,
                window_ms: 256,
                sample_rate_hz: 1000,
                features: vec!["x".to_string(), "y".to_string(), "vx".to_string(), "vy".to_string()],
                quantize: true,
                random_seed: Some(42),
                verbose: false,
            },
            model: ModelConfig {
                model_type: "micro_gru".to_string(),
                hidden_size: 32,
                num_layers: 2,
                dropout: 0.1,
                residual: true,
                activation: "tanh".to_string(),
                layer_norm: false,
            },
            training: TrainingConfig {
                optimizer: "adam".to_string(),
                learning_rate: 0.001,
                batch_size: 32,
                epochs: 100,
                patience: 10,
                val_frequency: 5,
                grad_clip: Some(1.0),
                weight_decay: 0.0001,
                smoothness_weight: 0.1,
                checkpoint_frequency: 10,
            },
            inference: InferenceConfig {
                target_latency_ms: 0.9,
                enable_simd: false, // Disabled for WASM compatibility
                num_threads: 1,
                pin_memory: false,
                cpu_affinity: None,
                batch_size: 1,
            },
            system: crate::config::SystemConfig::TemporalSolver(crate::config::TemporalSolverConfig {
                prior: crate::config::KalmanConfig {
                    process_noise: 0.01,
                    measurement_noise: 0.1,
                    initial_uncertainty: 1.0,
                    transition_model: "constant_velocity".to_string(),
                    update_frequency: 100.0,
                },
                solver_gate: crate::config::SolverGateConfig {
                    algorithm: "neumann".to_string(),
                    error_threshold: 0.01,
                    max_iterations: 100,
                    confidence_threshold: 0.9,
                    fallback_enabled: true,
                },
                active_selection: crate::config::ActiveSelectionConfig {
                    enabled: true,
                    selection_ratio: 0.1,
                    embedding_dim: 16,
                    pagerank_damping: 0.85,
                    update_frequency: 10,
                },
            }),
        };
        WasmConfig { inner: config }
    }

    /// Export configuration as JSON
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(&self.inner).unwrap_or_default()
    }
}

/// WASM wrapper for temporal neural network models
#[wasm_bindgen]
pub struct WasmTemporalSolver {
    predictor: Option<Box<dyn PredictorTrait>>,
    model_type: String,
    config: Config,
    is_trained: bool,
}

/// Trait for unified predictor interface in WASM
trait PredictorTrait {
    fn predict(&mut self, input: &[f64]) -> Result<Prediction>;
    fn predict_batch(&mut self, inputs: &[Vec<f64>]) -> Result<Vec<Prediction>>;
    fn get_latency_stats(&self) -> LatencyStats;
    fn warmup(&mut self, iterations: u32) -> Result<()>;
}

/// Wrapper for System A predictor
struct SystemAPredictor {
    predictor: Predictor,
}

impl PredictorTrait for SystemAPredictor {
    fn predict(&mut self, input: &[f64]) -> Result<Prediction> {
        let matrix = nalgebra::DMatrix::from_row_slice(4, input.len() / 4, input);
        self.predictor.predict(&matrix)
    }

    fn predict_batch(&mut self, inputs: &[Vec<f64>]) -> Result<Vec<Prediction>> {
        let matrices: Result<Vec<_>> = inputs.iter()
            .map(|input| {
                Ok(nalgebra::DMatrix::from_row_slice(4, input.len() / 4, input))
            })
            .collect();
        self.predictor.predict_batch(&matrices?)
    }

    fn get_latency_stats(&self) -> LatencyStats {
        let stats = self.predictor.get_statistics();
        LatencyStats {
            avg_latency_us: stats.avg_latency_us,
            p50_latency_us: stats.p50_latency_us,
            p99_latency_us: stats.p99_latency_us,
            p99_9_latency_us: stats.p99_9_latency_us,
            violation_rate: stats.violation_rate,
            throughput_pred_per_sec: stats.throughput_pred_per_sec,
        }
    }

    fn warmup(&mut self, iterations: u32) -> Result<()> {
        self.predictor.warmup(iterations as usize)
    }
}

/// Wrapper for System B predictor
struct SystemBPredictor {
    predictor: Predictor,
}

impl PredictorTrait for SystemBPredictor {
    fn predict(&mut self, input: &[f64]) -> Result<Prediction> {
        let matrix = nalgebra::DMatrix::from_row_slice(4, input.len() / 4, input);
        self.predictor.predict(&matrix)
    }

    fn predict_batch(&mut self, inputs: &[Vec<f64>]) -> Result<Vec<Prediction>> {
        let matrices: Result<Vec<_>> = inputs.iter()
            .map(|input| {
                Ok(nalgebra::DMatrix::from_row_slice(4, input.len() / 4, input))
            })
            .collect();
        self.predictor.predict_batch(&matrices?)
    }

    fn get_latency_stats(&self) -> LatencyStats {
        let stats = self.predictor.get_statistics();
        LatencyStats {
            avg_latency_us: stats.avg_latency_us,
            p50_latency_us: stats.p50_latency_us,
            p99_latency_us: stats.p99_latency_us,
            p99_9_latency_us: stats.p99_9_latency_us,
            violation_rate: stats.violation_rate,
            throughput_pred_per_sec: stats.throughput_pred_per_sec,
        }
    }

    fn warmup(&mut self, iterations: u32) -> Result<()> {
        self.predictor.warmup(iterations as usize)
    }
}

/// Latency statistics for WASM export
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyStats {
    pub avg_latency_us: f64,
    pub p50_latency_us: f64,
    pub p99_latency_us: f64,
    pub p99_9_latency_us: f64,
    pub violation_rate: f64,
    pub throughput_pred_per_sec: f64,
}

/// Prediction result for WASM export
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct WasmPrediction {
    prediction: Prediction,
}

#[wasm_bindgen]
impl WasmPrediction {
    /// Get predicted values as array
    #[wasm_bindgen(getter = values)]
    pub fn values(&self) -> Vec<f64> {
        self.prediction.values.as_slice().to_vec()
    }

    /// Get confidence score (0.0 to 1.0)
    #[wasm_bindgen(getter = confidence)]
    pub fn confidence(&self) -> f64 {
        self.prediction.confidence
    }

    /// Get prediction latency in microseconds
    #[wasm_bindgen(getter = latency_us)]
    pub fn latency_us(&self) -> f64 {
        self.prediction.latency_us
    }

    /// Get certificate error bound (System B only)
    #[wasm_bindgen(getter = certificate_error)]
    pub fn certificate_error(&self) -> Option<f64> {
        self.prediction.certificate.as_ref().map(|c| c.error_bound)
    }

    /// Check if solver gate passed (System B only)
    #[wasm_bindgen(getter = gate_passed)]
    pub fn gate_passed(&self) -> Option<bool> {
        self.prediction.certificate.as_ref().map(|c| c.is_valid)
    }

    /// Get prediction metadata as JSON
    #[wasm_bindgen(js_name = getMetadataJSON)]
    pub fn get_metadata_json(&self) -> String {
        serde_json::to_string(&self.prediction.metadata).unwrap_or_default()
    }
}

#[wasm_bindgen]
impl WasmTemporalSolver {
    /// Create a new temporal solver instance
    #[wasm_bindgen(constructor)]
    pub fn new(config: &WasmConfig) -> WasmTemporalSolver {
        WasmTemporalSolver {
            predictor: None,
            model_type: "uninitialized".to_string(),
            config: config.inner.clone(),
            is_trained: false,
        }
    }

    /// Initialize System A (traditional neural network)
    #[wasm_bindgen(js_name = initSystemA)]
    pub fn init_system_a(&mut self) -> Result<(), JsValue> {
        let model = SystemA::new(&self.config.model)
            .map_err(|e| JsValue::from_str(&format!("Failed to create System A: {}", e)))?;

        let predictor = Predictor::new_system_a(model, self.config.inference.clone())
            .map_err(|e| JsValue::from_str(&format!("Failed to create predictor: {}", e)))?;

        self.predictor = Some(Box::new(SystemAPredictor { predictor }));
        self.model_type = "SystemA".to_string();
        Ok(())
    }

    /// Initialize System B (temporal solver neural network)
    #[wasm_bindgen(js_name = initSystemB)]
    pub fn init_system_b(&mut self) -> Result<(), JsValue> {
        let temporal_config = match &self.config.system {
            crate::config::SystemConfig::TemporalSolver(config) => config.clone(),
            _ => return Err(JsValue::from_str("Config must be for temporal solver system")),
        };

        let model = SystemB::new(&self.config.model, &temporal_config)
            .map_err(|e| JsValue::from_str(&format!("Failed to create System B: {}", e)))?;

        let predictor = Predictor::new_system_b(model, self.config.inference.clone())
            .map_err(|e| JsValue::from_str(&format!("Failed to create predictor: {}", e)))?;

        self.predictor = Some(Box::new(SystemBPredictor { predictor }));
        self.model_type = "SystemB".to_string();
        Ok(())
    }

    /// Perform single prediction
    #[wasm_bindgen]
    pub fn predict(&mut self, input: &[f64]) -> Result<WasmPrediction, JsValue> {
        let predictor = self.predictor.as_mut()
            .ok_or_else(|| JsValue::from_str("Model not initialized"))?;

        let prediction = predictor.predict(input)
            .map_err(|e| JsValue::from_str(&format!("Prediction failed: {}", e)))?;

        Ok(WasmPrediction { prediction })
    }

    /// Perform batch predictions
    #[wasm_bindgen(js_name = predictBatch)]
    pub fn predict_batch(&mut self, inputs: &JsValue) -> Result<Array, JsValue> {
        let predictor = self.predictor.as_mut()
            .ok_or_else(|| JsValue::from_str("Model not initialized"))?;

        // Convert JS array to Vec<Vec<f64>>
        let js_array = Array::from(inputs);
        let mut input_vectors = Vec::new();

        for i in 0..js_array.length() {
            let js_input = js_array.get(i);
            let input_array = Array::from(&js_input);
            let mut input_vec = Vec::new();

            for j in 0..input_array.length() {
                let val = input_array.get(j).as_f64()
                    .ok_or_else(|| JsValue::from_str("Input must be numeric"))?;
                input_vec.push(val);
            }
            input_vectors.push(input_vec);
        }

        let predictions = predictor.predict_batch(&input_vectors)
            .map_err(|e| JsValue::from_str(&format!("Batch prediction failed: {}", e)))?;

        let result_array = Array::new();
        for prediction in predictions {
            let wasm_pred = WasmPrediction { prediction };
            result_array.push(&JsValue::from(wasm_pred));
        }

        Ok(result_array)
    }

    /// Get current latency statistics
    #[wasm_bindgen(js_name = getStats)]
    pub fn get_stats(&self) -> Result<LatencyStats, JsValue> {
        let predictor = self.predictor.as_ref()
            .ok_or_else(|| JsValue::from_str("Model not initialized"))?;

        Ok(predictor.get_latency_stats())
    }

    /// Warm up the model for consistent latency
    #[wasm_bindgen]
    pub fn warmup(&mut self, iterations: u32) -> Result<(), JsValue> {
        let predictor = self.predictor.as_mut()
            .ok_or_else(|| JsValue::from_str("Model not initialized"))?;

        predictor.warmup(iterations)
            .map_err(|e| JsValue::from_str(&format!("Warmup failed: {}", e)))?;

        Ok(())
    }

    /// Check if model meets performance targets
    #[wasm_bindgen(js_name = meetsTargets)]
    pub fn meets_targets(&self) -> bool {
        if let Some(predictor) = &self.predictor {
            let stats = predictor.get_latency_stats();
            stats.p99_9_latency_us <= 900.0 && stats.violation_rate <= 0.001
        } else {
            false
        }
    }

    /// Get model type
    #[wasm_bindgen(getter = model_type)]
    pub fn model_type(&self) -> String {
        self.model_type.clone()
    }

    /// Check if model is trained
    #[wasm_bindgen(getter = is_trained)]
    pub fn is_trained(&self) -> bool {
        self.is_trained
    }

    /// Get build information
    #[wasm_bindgen(js_name = getBuildInfo)]
    pub fn get_build_info() -> String {
        let build_info = crate::build_info();
        serde_json::to_string(&build_info).unwrap_or_default()
    }

    /// Benchmark the solver
    #[wasm_bindgen]
    pub fn benchmark(&mut self, num_predictions: u32) -> Result<String, JsValue> {
        let predictor = self.predictor.as_mut()
            .ok_or_else(|| JsValue::from_str("Model not initialized"))?;

        let start_time = js_sys::Date::now();
        let test_input = vec![1.0; 1024]; // 4x256 test input

        for _ in 0..num_predictions {
            predictor.predict(&test_input)
                .map_err(|e| JsValue::from_str(&format!("Benchmark prediction failed: {}", e)))?;
        }

        let end_time = js_sys::Date::now();
        let total_time_ms = end_time - start_time;
        let avg_latency_ms = total_time_ms / num_predictions as f64;
        let throughput = num_predictions as f64 / (total_time_ms / 1000.0);

        let stats = predictor.get_latency_stats();

        let benchmark_result = serde_json::json!({
            "num_predictions": num_predictions,
            "total_time_ms": total_time_ms,
            "avg_latency_ms": avg_latency_ms,
            "avg_latency_us": avg_latency_ms * 1000.0,
            "throughput_pred_per_sec": throughput,
            "p99_9_latency_us": stats.p99_9_latency_us,
            "meets_target": avg_latency_ms * 1000.0 <= 900.0,
            "model_type": self.model_type,
        });

        Ok(benchmark_result.to_string())
    }
}

/// Utilities for WASM
#[wasm_bindgen]
pub struct WasmUtils;

#[wasm_bindgen]
impl WasmUtils {
    /// Get version information
    #[wasm_bindgen(js_name = getVersion)]
    pub fn get_version() -> String {
        crate::VERSION.to_string()
    }

    /// Check if SIMD is supported
    #[wasm_bindgen(js_name = hasSIMD)]
    pub fn has_simd() -> bool {
        // In WASM, SIMD support would need to be detected at runtime
        false // Conservative default
    }

    /// Log message to console
    #[wasm_bindgen(js_name = log)]
    pub fn log(message: &str) {
        console::log_1(&message.into());
    }

    /// Generate sample trajectory data for testing
    #[wasm_bindgen(js_name = generateSampleData)]
    pub fn generate_sample_data(length: u32) -> Array {
        let mut data = Array::new();

        for i in 0..length {
            let t = i as f64 * 0.01;
            let trajectory = Array::new();

            // Simple circular trajectory
            trajectory.push(&JsValue::from(t.cos()));      // x
            trajectory.push(&JsValue::from(t.sin()));      // y
            trajectory.push(&JsValue::from(-t.sin()));     // vx
            trajectory.push(&JsValue::from(t.cos()));      // vy

            data.push(&trajectory);
        }

        data
    }

    /// Calculate temporal lead for given distance
    #[wasm_bindgen(js_name = calculateTemporalLead)]
    pub fn calculate_temporal_lead(distance_km: f64, computation_us: f64) -> f64 {
        let light_speed_km_per_s = 299_792.458; // km/ms in vacuum
        let light_travel_us = (distance_km / light_speed_km_per_s) * 1000.0;
        light_travel_us - computation_us
    }
}

/// Training interface for WASM (simplified)
#[wasm_bindgen]
pub struct WasmTrainer {
    trainer: Option<Trainer>,
    config: TrainingConfig,
}

#[wasm_bindgen]
impl WasmTrainer {
    /// Create new trainer
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmTrainer, JsValue> {
        let config: TrainingConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Invalid training config: {}", e)))?;

        let trainer = Trainer::new(config.clone())
            .map_err(|e| JsValue::from_str(&format!("Failed to create trainer: {}", e)))?;

        Ok(WasmTrainer {
            trainer: Some(trainer),
            config,
        })
    }

    /// Train model with provided data
    #[wasm_bindgen]
    pub fn train(&mut self, data_json: &str) -> Result<String, JsValue> {
        let _trainer = self.trainer.as_mut()
            .ok_or_else(|| JsValue::from_str("Trainer not initialized"))?;

        // For WASM, we'll provide a simplified training interface
        // Full training would require streaming data support
        let result = serde_json::json!({
            "message": "Training interface available - use full Rust API for production training",
            "epochs_completed": 0,
            "final_loss": 0.0,
            "training_time_seconds": 0.0
        });

        Ok(result.to_string())
    }
}

// Error handling utilities
impl From<TemporalNeuralError> for JsValue {
    fn from(error: TemporalNeuralError) -> Self {
        JsValue::from_str(&format!("{}", error))
    }
}

// Export main initialization function
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Macro for logging from WASM
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;