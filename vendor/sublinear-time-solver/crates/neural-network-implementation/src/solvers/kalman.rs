//! Kalman filter implementation for temporal prior predictions
//!
//! This module provides a Kalman filter implementation optimized for
//! providing high-quality prior predictions for the temporal neural network.

use crate::{
    config::KalmanConfig,
    error::{Result, TemporalNeuralError},
    solvers::InferenceReadyTrait,
};
use nalgebra::{DMatrix, DVector, Matrix2, Vector2};
use serde::{Deserialize, Serialize};

/// Kalman filter for providing temporal priors
///
/// This filter tracks position and velocity for 2D trajectory prediction,
/// providing physics-based priors that the neural network can refine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalmanFilter {
    /// Configuration
    config: KalmanConfig,
    /// Current state estimate [x, y, vx, vy]
    state: DVector<f64>,
    /// State covariance matrix
    covariance: DMatrix<f64>,
    /// State transition matrix
    transition_matrix: DMatrix<f64>,
    /// Process noise covariance
    process_noise: DMatrix<f64>,
    /// Measurement noise covariance
    measurement_noise: DMatrix<f64>,
    /// Measurement matrix (maps state to observations)
    measurement_matrix: DMatrix<f64>,
    /// Whether filter is initialized
    initialized: bool,
    /// Last prediction for error tracking
    last_prediction: Option<DVector<f64>>,
    /// Prediction error history
    prediction_errors: Vec<f64>,
    /// Time of last update
    last_update_time: Option<std::time::Instant>,
    /// Ready for inference flag
    inference_ready: bool,
}

impl KalmanFilter {
    /// Create a new Kalman filter
    pub fn new(config: &KalmanConfig) -> Result<Self> {
        let state_dim = 4; // [x, y, vx, vy]
        let obs_dim = 2; // [x, y]

        let state = DVector::zeros(state_dim);
        let covariance = DMatrix::identity(state_dim, state_dim) * config.initial_uncertainty;

        // Create state transition matrix based on model type
        let transition_matrix = match config.transition_model.as_str() {
            "constant_velocity" => Self::create_constant_velocity_matrix(1.0 / config.update_frequency),
            "constant_acceleration" => Self::create_constant_acceleration_matrix(1.0 / config.update_frequency),
            _ => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: format!("Unknown transition model: {}", config.transition_model),
                    field: Some("transition_model".to_string()),
                });
            }
        };

        // Process noise (uncertainty in dynamics)
        let dt = 1.0 / config.update_frequency;
        let process_noise = Self::create_process_noise_matrix(config.process_noise, dt);

        // Measurement noise
        let measurement_noise = DMatrix::identity(obs_dim, obs_dim) * config.measurement_noise;

        // Measurement matrix (observe position only)
        let measurement_matrix = DMatrix::from_row_slice(obs_dim, state_dim, &[
            1.0, 0.0, 0.0, 0.0, // x
            0.0, 1.0, 0.0, 0.0, // y
        ]);

        Ok(Self {
            config: config.clone(),
            state,
            covariance,
            transition_matrix,
            process_noise,
            measurement_noise,
            measurement_matrix,
            initialized: false,
            last_prediction: None,
            prediction_errors: Vec::new(),
            last_update_time: None,
            inference_ready: false,
        })
    }

    /// Create constant velocity transition matrix
    fn create_constant_velocity_matrix(dt: f64) -> DMatrix<f64> {
        DMatrix::from_row_slice(4, 4, &[
            1.0, 0.0, dt,  0.0, // x = x + vx*dt
            0.0, 1.0, 0.0, dt,  // y = y + vy*dt
            0.0, 0.0, 1.0, 0.0, // vx = vx
            0.0, 0.0, 0.0, 1.0, // vy = vy
        ])
    }

    /// Create constant acceleration transition matrix
    fn create_constant_acceleration_matrix(dt: f64) -> DMatrix<f64> {
        let dt2 = dt * dt / 2.0;
        DMatrix::from_row_slice(4, 4, &[
            1.0, 0.0, dt,  0.0,  // x = x + vx*dt
            0.0, 1.0, 0.0, dt,   // y = y + vy*dt
            0.0, 0.0, 0.9, 0.0,  // vx = 0.9*vx (decay)
            0.0, 0.0, 0.0, 0.9,  // vy = 0.9*vy (decay)
        ])
    }

    /// Create process noise covariance matrix
    fn create_process_noise_matrix(noise_level: f64, dt: f64) -> DMatrix<f64> {
        let dt2 = dt * dt;
        let dt3 = dt * dt2 / 2.0;
        let dt4 = dt2 * dt2 / 4.0;

        // Q matrix for constant velocity model
        DMatrix::from_row_slice(4, 4, &[
            dt4,  0.0,  dt3,  0.0,  // x variance and x-vx covariance
            0.0,  dt4,  0.0,  dt3,  // y variance and y-vy covariance
            dt3,  0.0,  dt2,  0.0,  // vx-x covariance and vx variance
            0.0,  dt3,  0.0,  dt2,  // vy-y covariance and vy variance
        ]) * noise_level
    }

    /// Predict next state (time update)
    pub fn predict(&self, _input: &DMatrix<f64>) -> Result<DVector<f64>> {
        if !self.initialized {
            // Return zero prediction if not initialized
            return Ok(DVector::zeros(2));
        }

        // Predict state: x_k|k-1 = F * x_k-1|k-1
        let predicted_state = &self.transition_matrix * &self.state;

        // Extract position prediction [x, y]
        Ok(DVector::from_vec(vec![predicted_state[0], predicted_state[1]]))
    }

    /// Const version of predict for immutable contexts
    pub fn predict_const(&self, _input: &DMatrix<f64>) -> Result<DVector<f64>> {
        self.predict(_input)
    }

    /// Update filter with measurement (measurement update)
    pub fn update(&mut self, measurement: &DVector<f64>) -> Result<()> {
        if measurement.len() != 2 {
            return Err(TemporalNeuralError::KalmanError {
                message: format!("Expected 2D measurement, got {}", measurement.len()),
                state_dimension: Some(self.state.len()),
            });
        }

        if !self.initialized {
            // Initialize state with first measurement
            self.state[0] = measurement[0]; // x
            self.state[1] = measurement[1]; // y
            self.state[2] = 0.0; // vx = 0
            self.state[3] = 0.0; // vy = 0
            self.initialized = true;
            self.last_update_time = Some(std::time::Instant::now());
            return Ok(());
        }

        // Time update (predict)
        let predicted_state = &self.transition_matrix * &self.state;
        let predicted_covariance = &self.transition_matrix * &self.covariance * self.transition_matrix.transpose() + &self.process_noise;

        // Measurement update (correct)
        let innovation = measurement - &self.measurement_matrix * &predicted_state;
        let innovation_covariance = &self.measurement_matrix * &predicted_covariance * self.measurement_matrix.transpose() + &self.measurement_noise;

        // Kalman gain
        let kalman_gain = &predicted_covariance * self.measurement_matrix.transpose() * innovation_covariance.try_inverse().ok_or_else(|| {
            TemporalNeuralError::KalmanError {
                message: "Innovation covariance matrix is not invertible".to_string(),
                state_dimension: Some(self.state.len()),
            }
        })?;

        // Update state and covariance
        self.state = predicted_state + &kalman_gain * innovation;
        let identity = DMatrix::identity(self.state.len(), self.state.len());
        self.covariance = (identity - &kalman_gain * &self.measurement_matrix) * predicted_covariance;

        // Track prediction error if we had a previous prediction
        if let Some(ref last_pred) = self.last_prediction {
            let error = (measurement - last_pred).norm();
            self.prediction_errors.push(error);

            // Keep only recent errors (for memory efficiency)
            if self.prediction_errors.len() > 1000 {
                self.prediction_errors.remove(0);
            }
        }

        self.last_prediction = Some(measurement.clone());
        self.last_update_time = Some(std::time::Instant::now());

        Ok(())
    }

    /// Get current state estimate
    pub fn get_state(&self) -> &DVector<f64> {
        &self.state
    }

    /// Get current covariance estimate
    pub fn get_covariance(&self) -> &DMatrix<f64> {
        &self.covariance
    }

    /// Get prediction uncertainty (position covariance)
    pub fn get_prediction_uncertainty(&self) -> Matrix2<f64> {
        if !self.initialized {
            return Matrix2::identity() * 1000.0; // High uncertainty
        }

        // Extract position covariance [x, y]
        Matrix2::new(
            self.covariance[(0, 0)], self.covariance[(0, 1)],
            self.covariance[(1, 0)], self.covariance[(1, 1)],
        )
    }

    /// Get average prediction error
    pub fn get_prediction_error(&self) -> f64 {
        if self.prediction_errors.is_empty() {
            0.0
        } else {
            self.prediction_errors.iter().sum::<f64>() / self.prediction_errors.len() as f64
        }
    }

    /// Check if filter is well-conditioned
    pub fn is_well_conditioned(&self) -> bool {
        if !self.initialized {
            return false;
        }

        // Check covariance matrix condition
        let max_eigenvalue = self.covariance.diagonal().max();
        let min_eigenvalue = self.covariance.diagonal().min();

        if min_eigenvalue <= 0.0 {
            return false;
        }

        let condition_number = max_eigenvalue / min_eigenvalue;
        condition_number < 1e6 // Reasonable condition number
    }

    /// Predict at specific time horizon
    pub fn predict_at_horizon(&self, horizon_seconds: f64) -> Result<Vector2<f64>> {
        if !self.initialized {
            return Ok(Vector2::zeros());
        }

        // Create transition matrix for the specific horizon
        let transition = match self.config.transition_model.as_str() {
            "constant_velocity" => Self::create_constant_velocity_matrix(horizon_seconds),
            "constant_acceleration" => Self::create_constant_acceleration_matrix(horizon_seconds),
            _ => self.transition_matrix.clone(),
        };

        // Predict state at horizon
        let predicted_state = &transition * &self.state;

        Ok(Vector2::new(predicted_state[0], predicted_state[1]))
    }

    /// Adaptive tuning based on recent performance
    pub fn adapt_parameters(&mut self) -> Result<()> {
        if self.prediction_errors.len() < 10 {
            return Ok(()); // Need enough data
        }

        let recent_errors: Vec<f64> = self.prediction_errors
            .iter()
            .rev()
            .take(10)
            .cloned()
            .collect();

        let avg_error = recent_errors.iter().sum::<f64>() / recent_errors.len() as f64;

        // Adapt process noise based on error
        if avg_error > 0.1 {
            // High error - increase process noise
            self.process_noise *= 1.1;
        } else if avg_error < 0.01 {
            // Low error - decrease process noise
            self.process_noise *= 0.95;
        }

        // Clamp process noise to reasonable bounds
        let min_noise = 1e-6;
        let max_noise = 1.0;
        for element in self.process_noise.iter_mut() {
            *element = element.clamp(min_noise, max_noise);
        }

        Ok(())
    }
}

impl InferenceReadyTrait for KalmanFilter {
    fn prepare_for_inference(&mut self) -> Result<()> {
        // Ensure filter is in a good state for inference
        if !self.initialized {
            return Err(TemporalNeuralError::KalmanError {
                message: "Kalman filter not initialized".to_string(),
                state_dimension: Some(self.state.len()),
            });
        }

        if !self.is_well_conditioned() {
            return Err(TemporalNeuralError::KalmanError {
                message: "Kalman filter is poorly conditioned".to_string(),
                state_dimension: Some(self.state.len()),
            });
        }

        // Clear prediction error history to save memory
        self.prediction_errors.clear();
        self.inference_ready = true;

        Ok(())
    }

    fn is_inference_ready(&self) -> bool {
        self.inference_ready && self.initialized && self.is_well_conditioned()
    }

    fn memory_usage(&self) -> usize {
        std::mem::size_of::<Self>() +
        self.state.len() * std::mem::size_of::<f64>() +
        self.covariance.len() * std::mem::size_of::<f64>() +
        self.transition_matrix.len() * std::mem::size_of::<f64>() +
        self.process_noise.len() * std::mem::size_of::<f64>() +
        self.measurement_noise.len() * std::mem::size_of::<f64>() +
        self.measurement_matrix.len() * std::mem::size_of::<f64>() +
        self.prediction_errors.len() * std::mem::size_of::<f64>()
    }

    fn reset(&mut self) -> Result<()> {
        self.state.fill(0.0);
        self.covariance = DMatrix::identity(self.state.len(), self.state.len()) * self.config.initial_uncertainty;
        self.initialized = false;
        self.last_prediction = None;
        self.prediction_errors.clear();
        self.last_update_time = None;
        self.inference_ready = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> KalmanConfig {
        KalmanConfig {
            process_noise: 0.01,
            measurement_noise: 0.1,
            initial_uncertainty: 1.0,
            transition_model: "constant_velocity".to_string(),
            update_frequency: 100.0,
        }
    }

    #[test]
    fn test_kalman_creation() {
        let config = create_test_config();
        let filter = KalmanFilter::new(&config).unwrap();

        assert!(!filter.initialized);
        assert_eq!(filter.state.len(), 4);
        assert_eq!(filter.covariance.shape(), (4, 4));
    }

    #[test]
    fn test_kalman_initialization() {
        let config = create_test_config();
        let mut filter = KalmanFilter::new(&config).unwrap();

        let measurement = DVector::from_vec(vec![1.0, 2.0]);
        filter.update(&measurement).unwrap();

        assert!(filter.initialized);
        assert_eq!(filter.state[0], 1.0);
        assert_eq!(filter.state[1], 2.0);
    }

    #[test]
    fn test_prediction_tracking() {
        let config = create_test_config();
        let mut filter = KalmanFilter::new(&config).unwrap();

        // Initialize
        let measurement1 = DVector::from_vec(vec![0.0, 0.0]);
        filter.update(&measurement1).unwrap();

        // Update with moving trajectory
        let measurement2 = DVector::from_vec(vec![1.0, 1.0]);
        filter.update(&measurement2).unwrap();

        // Predict should show movement
        let input = DMatrix::zeros(4, 10); // Dummy input
        let prediction = filter.predict(&input).unwrap();

        assert!(prediction[0] > 0.5); // Should predict continued movement
        assert!(prediction[1] > 0.5);
    }

    #[test]
    fn test_horizon_prediction() {
        let config = create_test_config();
        let mut filter = KalmanFilter::new(&config).unwrap();

        // Initialize with trajectory
        filter.update(&DVector::from_vec(vec![0.0, 0.0])).unwrap();
        filter.update(&DVector::from_vec(vec![1.0, 0.0])).unwrap(); // Moving right

        let horizon_pred = filter.predict_at_horizon(0.5).unwrap(); // 0.5 seconds

        assert!(horizon_pred[0] > 1.0); // Should be further right
        assert!(horizon_pred[1].abs() < 0.1); // Should stay near y=0
    }

    #[test]
    fn test_condition_checking() {
        let config = create_test_config();
        let mut filter = KalmanFilter::new(&config).unwrap();

        assert!(!filter.is_well_conditioned()); // Not initialized

        filter.update(&DVector::from_vec(vec![0.0, 0.0])).unwrap();
        assert!(filter.is_well_conditioned()); // Should be well-conditioned after init
    }

    #[test]
    fn test_inference_preparation() {
        let config = create_test_config();
        let mut filter = KalmanFilter::new(&config).unwrap();

        // Should fail before initialization
        assert!(filter.prepare_for_inference().is_err());

        // Initialize
        filter.update(&DVector::from_vec(vec![0.0, 0.0])).unwrap();

        // Should succeed after initialization
        assert!(filter.prepare_for_inference().is_ok());
        assert!(filter.is_inference_ready());
    }
}