//! System B: Temporal solver neural network implementation
//!
//! This module implements the novel temporal solver approach that combines
//! neural networks with Kalman filter priors and sublinear solver gating
//! for improved latency and mathematical verification.

use crate::{
    config::{ModelConfig, TemporalSolverConfig},
    error::{Result, TemporalNeuralError},
    models::{
        layers::{GruLayer, TcnLayer, DenseLayer, ActivationFunction},
        ModelTrait, ModelParams, ParameterStats,
        system_a::{SystemA, SystemAParams}, // Reuse SystemA architecture
    },
    solvers::{KalmanFilter, SolverGate, PageRankSelector},
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

/// System B: Temporal solver neural network
///
/// This system combines a neural network (same architecture as System A)
/// with Kalman filter priors and sublinear solver verification.
/// The key innovation is residual learning: the network predicts the
/// residual between the Kalman prior and the true target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemB {
    /// Base neural network (same as System A)
    base_network: SystemA,
    /// Kalman filter for prior predictions
    kalman_filter: KalmanFilter,
    /// Sublinear solver gate for verification
    solver_gate: SolverGate,
    /// PageRank-based active selector for training
    active_selector: Option<PageRankSelector>,
    /// Temporal solver configuration
    solver_config: TemporalSolverConfig,
    /// Whether the system is in training or inference mode
    inference_mode: bool,
}

/// Prediction result from System B
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBPrediction {
    /// Kalman filter prior prediction
    pub prior: DVector<f64>,
    /// Neural network residual prediction
    pub residual: DVector<f64>,
    /// Final combined prediction (prior + residual)
    pub prediction: DVector<f64>,
    /// Solver gate verification result
    pub gate_result: GateResult,
    /// Computation timing breakdown
    pub timing: PredictionTiming,
}

/// Result from the solver gate verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateResult {
    /// Whether the gate passed (prediction is verified)
    pub passed: bool,
    /// Certificate error from sublinear solver
    pub certificate_error: f64,
    /// Computational work performed
    pub work_performed: u64,
    /// Fallback strategy used if gate failed
    pub fallback_used: Option<String>,
}

/// Timing breakdown for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictionTiming {
    /// Time spent in Kalman filter update (microseconds)
    pub kalman_us: f64,
    /// Time spent in neural network forward pass (microseconds)
    pub network_us: f64,
    /// Time spent in solver gate verification (microseconds)
    pub gate_us: f64,
    /// Total prediction time (microseconds)
    pub total_us: f64,
}

impl SystemB {
    /// Create a new System B model
    pub fn new(config: &ModelConfig, solver_config: &TemporalSolverConfig) -> Result<Self> {
        // Create base network (same architecture as System A)
        let base_network = SystemA::new(config)?;

        // Initialize Kalman filter
        let kalman_filter = KalmanFilter::new(&solver_config.prior)?;

        // Initialize solver gate
        let solver_gate = SolverGate::new(&solver_config.solver_gate)?;

        // Initialize active selector for training
        let active_selector = Some(PageRankSelector::new(&solver_config.active_selection)?);

        Ok(Self {
            base_network,
            kalman_filter,
            solver_gate,
            active_selector,
            solver_config: solver_config.clone(),
            inference_mode: false,
        })
    }

    /// Set inference mode (disables active selector)
    pub fn set_inference_mode(&mut self, inference_mode: bool) {
        self.inference_mode = inference_mode;
        if inference_mode {
            self.active_selector = None; // Save memory in inference
        }
    }

    /// Predict with full temporal solver pipeline
    pub fn predict_with_solver(&mut self, input: &DMatrix<f64>) -> Result<SystemBPrediction> {
        let start_time = std::time::Instant::now();

        // Step 1: Update Kalman filter and get prior
        let kalman_start = std::time::Instant::now();
        let prior = self.kalman_filter.predict(input)?;
        let kalman_time = kalman_start.elapsed().as_micros() as f64;

        // Step 2: Neural network predicts residual
        let network_start = std::time::Instant::now();
        let residual = self.base_network.forward(input)?;
        let network_time = network_start.elapsed().as_micros() as f64;

        // Step 3: Combine prior and residual
        let prediction = &prior + &residual;

        // Step 4: Verify with solver gate
        let gate_start = std::time::Instant::now();
        let gate_result = self.solver_gate.verify(&prior, &residual, &prediction)?;
        let gate_time = gate_start.elapsed().as_micros() as f64;

        let total_time = start_time.elapsed().as_micros() as f64;

        // Apply fallback if gate failed
        let final_prediction = if gate_result.passed {
            prediction
        } else {
            self.apply_fallback(&prior, &residual, &gate_result)?
        };

        Ok(SystemBPrediction {
            prior,
            residual,
            prediction: final_prediction,
            gate_result,
            timing: PredictionTiming {
                kalman_us: kalman_time,
                network_us: network_time,
                gate_us: gate_time,
                total_us: total_time,
            },
        })
    }

    /// Apply fallback strategy when solver gate fails
    fn apply_fallback(
        &self,
        prior: &DVector<f64>,
        residual: &DVector<f64>,
        gate_result: &GateResult,
    ) -> Result<DVector<f64>> {
        match self.solver_config.solver_gate.fallback_strategy.as_str() {
            "kalman_only" => Ok(prior.clone()),
            "hold_last" => {
                // In a real implementation, this would hold the last verified prediction
                // For now, use the prior
                Ok(prior.clone())
            }
            "disable_gate" => {
                // Use the combined prediction anyway
                Ok(prior + residual)
            }
            "weighted_blend" => {
                // Blend based on certificate error
                let cert_error = gate_result.certificate_error;
                let weight = 1.0 / (1.0 + cert_error * 10.0); // Sigmoid-like weighting
                Ok(prior * (1.0 - weight) + (prior + residual) * weight)
            }
            _ => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: format!(
                        "Unknown fallback strategy: {}",
                        self.solver_config.solver_gate.fallback_strategy
                    ),
                    field: Some("fallback_strategy".to_string()),
                });
            }
        }
    }

    /// Update Kalman filter state with ground truth (for training)
    pub fn update_kalman_state(&mut self, measurement: &DVector<f64>) -> Result<()> {
        self.kalman_filter.update(measurement)
    }

    /// Get active sample selector for training
    pub fn active_selector(&mut self) -> Option<&mut PageRankSelector> {
        self.active_selector.as_mut()
    }

    /// Reset all internal states
    pub fn reset_states(&mut self) -> Result<()> {
        self.kalman_filter.reset()?;
        self.solver_gate.reset()?;
        if let Some(ref mut selector) = self.active_selector {
            selector.reset()?;
        }
        Ok(())
    }

    /// Get solver statistics for monitoring
    pub fn get_solver_stats(&self) -> SolverStats {
        SolverStats {
            gate_pass_rate: self.solver_gate.get_pass_rate(),
            avg_certificate_error: self.solver_gate.get_avg_certificate_error(),
            avg_computational_work: self.solver_gate.get_avg_work(),
            kalman_prediction_error: self.kalman_filter.get_prediction_error(),
            total_predictions: self.solver_gate.get_prediction_count(),
        }
    }

    /// Configure solver parameters dynamically
    pub fn configure_solver(&mut self, epsilon: Option<f64>, budget: Option<u64>) -> Result<()> {
        if let Some(eps) = epsilon {
            self.solver_gate.set_epsilon(eps)?;
        }
        if let Some(b) = budget {
            self.solver_gate.set_budget(b)?;
        }
        Ok(())
    }
}

/// Statistics from the solver components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverStats {
    /// Percentage of predictions that passed the gate
    pub gate_pass_rate: f64,
    /// Average certificate error across all predictions
    pub avg_certificate_error: f64,
    /// Average computational work performed
    pub avg_computational_work: f64,
    /// Kalman filter prediction error
    pub kalman_prediction_error: f64,
    /// Total number of predictions made
    pub total_predictions: u64,
}

impl ModelTrait for SystemB {
    type Params = SystemAParams; // Reuse SystemA parameters

    fn new(config: &ModelConfig) -> Result<Self> {
        // This is a simplified constructor - in practice would need full solver config
        let default_solver_config = TemporalSolverConfig::default();
        Self::new(config, &default_solver_config)
    }

    fn forward(&self, input: &DMatrix<f64>) -> Result<DVector<f64>> {
        // For the ModelTrait interface, we provide a simplified forward pass
        // without solver verification (for compatibility)
        let prior = self.kalman_filter.predict_const(input)?;
        let residual = self.base_network.forward(input)?;
        Ok(&prior + &residual)
    }

    fn parameters(&self) -> &Self::Params {
        self.base_network.parameters()
    }

    fn parameters_mut(&mut self) -> &mut Self::Params {
        self.base_network.parameters_mut()
    }

    fn load_parameters(&mut self, params: Self::Params) -> Result<()> {
        self.base_network.load_parameters(params)
    }

    fn parameter_count(&self) -> usize {
        self.base_network.parameter_count()
    }

    fn memory_usage(&self) -> usize {
        self.base_network.memory_usage() +
        self.kalman_filter.memory_usage() +
        self.solver_gate.memory_usage() +
        self.active_selector.as_ref().map_or(0, |s| s.memory_usage())
    }

    fn input_shape(&self) -> (usize, usize) {
        self.base_network.input_shape()
    }

    fn output_dim(&self) -> usize {
        self.base_network.output_dim()
    }

    fn model_name(&self) -> &'static str {
        "SystemB"
    }

    fn config(&self) -> &ModelConfig {
        self.base_network.config()
    }

    fn prepare_for_inference(&mut self) -> Result<()> {
        self.set_inference_mode(true);
        self.base_network.prepare_for_inference()?;
        self.kalman_filter.prepare_for_inference()?;
        self.solver_gate.prepare_for_inference()?;
        Ok(())
    }

    fn is_inference_ready(&self) -> bool {
        self.inference_mode &&
        self.base_network.is_inference_ready() &&
        self.kalman_filter.is_inference_ready() &&
        self.solver_gate.is_inference_ready()
    }
}

/// Training utilities specific to System B
impl SystemB {
    /// Compute residual learning loss
    pub fn compute_residual_loss(
        &self,
        predictions: &[SystemBPrediction],
        targets: &[DVector<f64>],
    ) -> Result<f64> {
        if predictions.len() != targets.len() {
            return Err(TemporalNeuralError::TrainingError {
                epoch: 0,
                message: "Predictions and targets length mismatch".to_string(),
                metrics: None,
            });
        }

        let mut total_loss = 0.0;
        let mut valid_samples = 0;

        for (pred, target) in predictions.iter().zip(targets.iter()) {
            // Residual learning: target - prior should equal residual
            let expected_residual = target - &pred.prior;
            let residual_error = &pred.residual - &expected_residual;

            // MSE loss
            let sample_loss: f64 = residual_error.iter().map(|x| x * x).sum();
            total_loss += sample_loss;
            valid_samples += 1;
        }

        if valid_samples == 0 {
            return Err(TemporalNeuralError::TrainingError {
                epoch: 0,
                message: "No valid samples for loss computation".to_string(),
                metrics: None,
            });
        }

        Ok(total_loss / valid_samples as f64)
    }

    /// Compute additional regularization terms
    pub fn compute_regularization_loss(&self, predictions: &[SystemBPrediction]) -> f64 {
        let mut reg_loss = 0.0;
        let smoothness_weight = 0.1; // From config

        // Smoothness penalty on velocity predictions
        for pred in predictions {
            if pred.prediction.len() >= 2 {
                // Assume prediction is [x, y] - penalize large velocities
                let velocity_mag = pred.prediction[0].powi(2) + pred.prediction[1].powi(2);
                reg_loss += smoothness_weight * velocity_mag;
            }
        }

        // Gate verification penalty
        let gate_penalty = 1.0;
        for pred in predictions {
            if !pred.gate_result.passed {
                reg_loss += gate_penalty * pred.gate_result.certificate_error;
            }
        }

        reg_loss / predictions.len() as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_b_creation() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 8,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let solver_config = TemporalSolverConfig::default();
        let system = SystemB::new(&config, &solver_config).unwrap();

        assert_eq!(system.model_name(), "SystemB");
        assert_eq!(system.output_dim(), 2);
    }

    #[test]
    fn test_inference_mode() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 4,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let solver_config = TemporalSolverConfig::default();
        let mut system = SystemB::new(&config, &solver_config).unwrap();

        assert!(!system.inference_mode);
        assert!(system.active_selector.is_some());

        system.set_inference_mode(true);
        assert!(system.inference_mode);
        assert!(system.active_selector.is_none());
    }

    #[test]
    fn test_forward_pass() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 4,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let solver_config = TemporalSolverConfig::default();
        let system = SystemB::new(&config, &solver_config).unwrap();

        let input = DMatrix::from_element(4, 10, 1.0);
        let output = system.forward(&input).unwrap();

        assert_eq!(output.len(), 2);
    }

    #[test]
    fn test_solver_stats() {
        let config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 4,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let solver_config = TemporalSolverConfig::default();
        let system = SystemB::new(&config, &solver_config).unwrap();

        let stats = system.get_solver_stats();
        assert!(stats.gate_pass_rate >= 0.0 && stats.gate_pass_rate <= 1.0);
        assert!(stats.avg_certificate_error >= 0.0);
    }
}