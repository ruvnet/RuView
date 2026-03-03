//! Utility functions and helpers

use crate::error::{Result, TemporalNeuralError};
use nalgebra::DMatrix;
use std::time::{Duration, Instant};

/// Timer for performance measurement
pub struct Timer {
    start: Instant,
    name: String,
}

impl Timer {
    pub fn new(name: &str) -> Self {
        Self {
            start: Instant::now(),
            name: name.to_string(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_ms(&self) -> f64 {
        self.elapsed().as_secs_f64() * 1000.0
    }

    pub fn elapsed_micros(&self) -> f64 {
        self.elapsed().as_secs_f64() * 1_000_000.0
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        println!("{}: {:.3}ms", self.name, self.elapsed_ms());
    }
}

/// Mathematical utilities
pub mod math {
    use super::*;

    /// Compute softmax activation
    pub fn softmax(input: &[f64]) -> Vec<f64> {
        let max_val = input.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let exp_vals: Vec<f64> = input.iter().map(|&x| (x - max_val).exp()).collect();
        let sum: f64 = exp_vals.iter().sum();
        exp_vals.iter().map(|&x| x / sum).collect()
    }

    /// Compute ReLU activation
    pub fn relu(x: f64) -> f64 {
        x.max(0.0)
    }

    /// Compute tanh activation
    pub fn tanh_activation(x: f64) -> f64 {
        x.tanh()
    }

    /// Compute sigmoid activation
    pub fn sigmoid(x: f64) -> f64 {
        1.0 / (1.0 + (-x).exp())
    }

    /// Compute mean squared error
    pub fn mse(predicted: &[f64], actual: &[f64]) -> Result<f64> {
        if predicted.len() != actual.len() {
            return Err(TemporalNeuralError::DimensionMismatch {
                message: "Predicted and actual arrays have different lengths".to_string(),
                expected: Some(actual.len().to_string()),
                actual: Some(predicted.len().to_string()),
                context: Some("mse computation".to_string()),
            });
        }

        let sum_sq_error: f64 = predicted
            .iter()
            .zip(actual.iter())
            .map(|(&p, &a)| (p - a).powi(2))
            .sum();

        Ok(sum_sq_error / predicted.len() as f64)
    }

    /// Compute mean absolute error
    pub fn mae(predicted: &[f64], actual: &[f64]) -> Result<f64> {
        if predicted.len() != actual.len() {
            return Err(TemporalNeuralError::DimensionMismatch {
                message: "Predicted and actual arrays have different lengths".to_string(),
                expected: Some(actual.len().to_string()),
                actual: Some(predicted.len().to_string()),
                context: Some("mae computation".to_string()),
            });
        }

        let sum_abs_error: f64 = predicted
            .iter()
            .zip(actual.iter())
            .map(|(&p, &a)| (p - a).abs())
            .sum();

        Ok(sum_abs_error / predicted.len() as f64)
    }
}

/// Memory utilities
pub mod memory {
    use super::*;

    /// Get current memory usage in bytes
    pub fn get_memory_usage() -> Result<usize> {
        // Placeholder for memory monitoring
        // In real implementation, would use system calls
        Ok(0)
    }

    /// Calculate matrix memory footprint
    pub fn matrix_memory_size(matrix: &DMatrix<f64>) -> usize {
        matrix.nrows() * matrix.ncols() * std::mem::size_of::<f64>()
    }
}

/// Validation utilities
pub mod validation {
    use super::*;

    /// Validate matrix dimensions for operations
    pub fn validate_matrix_dims(
        a: &DMatrix<f64>,
        b: &DMatrix<f64>,
        operation: &str,
    ) -> Result<()> {
        match operation {
            "multiply" => {
                if a.ncols() != b.nrows() {
                    return Err(TemporalNeuralError::DimensionMismatch {
                        message: "Matrix dimensions incompatible for multiplication".to_string(),
                        expected: Some(a.ncols().to_string()),
                        actual: Some(b.nrows().to_string()),
                        context: Some("matrix multiplication".to_string()),
                    });
                }
            }
            "add" | "subtract" => {
                if a.shape() != b.shape() {
                    return Err(TemporalNeuralError::DimensionMismatch {
                        message: "Matrix shapes must match for addition/subtraction".to_string(),
                        expected: Some(format!("{}x{}", a.nrows(), a.ncols())),
                        actual: Some(format!("{}x{}", b.nrows(), b.ncols())),
                        context: Some(operation.to_string()),
                    });
                }
            }
            _ => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: format!("Unknown operation: {}", operation),
                    field: Some("operation".to_string()),
                });
            }
        }
        Ok(())
    }

    /// Validate prediction bounds
    pub fn validate_prediction_bounds(prediction: &[f64], bounds: (f64, f64)) -> Result<()> {
        for &value in prediction {
            if value < bounds.0 || value > bounds.1 {
                return Err(TemporalNeuralError::ValidationError {
                    message: format!(
                        "Prediction value {} outside bounds [{}, {}]",
                        value, bounds.0, bounds.1
                    ),
                    expected: Some(format!("[{}, {}]", bounds.0, bounds.1)),
                    actual: Some(value.to_string()),
                    rule: Some("bounds_check".to_string()),
                });
            }
        }
        Ok(())
    }
}
