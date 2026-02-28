//! Activation predictor module.
//!
//! This module provides predictors for determining which neurons will be active
//! before performing the full computation.

mod lowrank;

pub use lowrank::LowRankPredictor;

use crate::error::Result;

/// Trait for activation predictors.
pub trait Predictor: Send + Sync {
    /// Predict active neurons for the given input.
    ///
    /// Returns a vector of neuron indices that are predicted to be active.
    fn predict(&self, input: &[f32]) -> Result<Vec<usize>>;

    /// Calibrate the predictor using sample data.
    ///
    /// # Arguments
    /// * `samples` - Input samples
    /// * `activations` - Corresponding activation patterns
    fn calibrate(&mut self, samples: &[Vec<f32>], activations: &[Vec<f32>]) -> Result<()>;

    /// Get predictor statistics.
    fn stats(&self) -> PredictorStats;
}

/// Alias for backward compatibility.
pub trait NeuronPredictor: Predictor {}

impl<T: Predictor> NeuronPredictor for T {}

/// Dense predictor that returns all neurons (for baseline comparison).
pub struct DensePredictor {
    neuron_count: usize,
}

impl DensePredictor {
    /// Create a new dense predictor.
    pub fn new(neuron_count: usize) -> Self {
        Self { neuron_count }
    }
}

impl Predictor for DensePredictor {
    fn predict(&self, _input: &[f32]) -> Result<Vec<usize>> {
        Ok((0..self.neuron_count).collect())
    }

    fn calibrate(&mut self, _samples: &[Vec<f32>], _activations: &[Vec<f32>]) -> Result<()> {
        Ok(())
    }

    fn stats(&self) -> PredictorStats {
        PredictorStats {
            predictions: 0,
            avg_active_neurons: self.neuron_count as f32,
            avg_sparsity: 0.0,
            is_calibrated: true,
        }
    }
}

/// Statistics about predictor performance.
#[derive(Debug, Clone, Default)]
pub struct PredictorStats {
    /// Number of predictions made.
    pub predictions: usize,

    /// Average number of neurons predicted as active.
    pub avg_active_neurons: f32,

    /// Average sparsity ratio (1 - active/total).
    pub avg_sparsity: f32,

    /// Whether the predictor is calibrated.
    pub is_calibrated: bool,
}
