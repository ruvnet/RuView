//! Training pipeline for temporal neural networks
//!
//! This module implements the training logic for both System A and System B,
//! including active sample selection, residual learning, and performance monitoring.

use crate::{
    config::{Config, TrainingConfig},
    data::{DataSplits, WindowedSample},
    error::{Result, TemporalNeuralError, TrainingMetrics},
    models::{ModelTrait, ModelParams, SystemA, SystemB},
    solvers::PageRankSelector,
};
use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub mod optimizer;
pub mod losses;
pub mod callbacks;

pub use optimizer::{Optimizer, AdamOptimizer, SgdOptimizer};
pub use losses::{LossFunction, MseLoss, SmoothnessPenalty};
pub use callbacks::{Callback, EarlyStoppingCallback, CheckpointCallback};

/// Training result containing model and metrics
#[derive(Debug, Clone)]
pub struct TrainingResult {
    /// Training history
    pub history: TrainingHistory,
    /// Final model state
    pub final_loss: f64,
    /// Whether training converged
    pub converged: bool,
    /// Total training time in seconds
    pub total_time_seconds: f64,
    /// Best validation loss achieved
    pub best_val_loss: f64,
    /// Epoch at which best validation loss was achieved
    pub best_epoch: u32,
}

/// Training history tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingHistory {
    /// Training loss per epoch
    pub train_losses: Vec<f64>,
    /// Validation loss per epoch
    pub val_losses: Vec<f64>,
    /// Learning rate per epoch
    pub learning_rates: Vec<f64>,
    /// Training time per epoch (seconds)
    pub epoch_times: Vec<f64>,
    /// Additional metrics per epoch
    pub metrics: Vec<EpochMetrics>,
}

/// Metrics tracked per epoch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpochMetrics {
    /// Epoch number
    pub epoch: u32,
    /// Training samples processed
    pub samples_processed: usize,
    /// Average gradient norm
    pub avg_gradient_norm: f64,
    /// Parameter update magnitude
    pub param_update_norm: f64,
    /// Memory usage in bytes
    pub memory_usage_bytes: usize,
    /// System B specific metrics
    pub system_b_metrics: Option<SystemBMetrics>,
}

/// System B specific training metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemBMetrics {
    /// Gate pass rate during training
    pub gate_pass_rate: f64,
    /// Average certificate error
    pub avg_certificate_error: f64,
    /// Kalman filter prediction error
    pub kalman_prediction_error: f64,
    /// Active selection efficiency
    pub active_selection_efficiency: f64,
    /// Residual learning loss
    pub residual_loss: f64,
}

/// Main trainer for temporal neural networks
pub struct Trainer {
    /// Training configuration
    config: TrainingConfig,
    /// Optimizer
    optimizer: Box<dyn Optimizer>,
    /// Loss function
    loss_fn: Box<dyn LossFunction>,
    /// Callbacks
    callbacks: Vec<Box<dyn Callback>>,
    /// Training history
    history: TrainingHistory,
    /// Current epoch
    current_epoch: u32,
    /// Best validation loss
    best_val_loss: f64,
    /// Early stopping patience counter
    patience_counter: u32,
}

impl Trainer {
    /// Create a new trainer
    pub fn new(config: TrainingConfig) -> Result<Self> {
        // Create optimizer
        let optimizer: Box<dyn Optimizer> = match config.optimizer.as_str() {
            "adam" => Box::new(AdamOptimizer::new(config.learning_rate)),
            "sgd" => Box::new(SgdOptimizer::new(config.learning_rate)),
            "rmsprop" => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: "RMSprop optimizer not yet implemented".to_string(),
                    field: Some("optimizer".to_string()),
                });
            }
            _ => {
                return Err(TemporalNeuralError::ConfigurationError {
                    message: format!("Unknown optimizer: {}", config.optimizer),
                    field: Some("optimizer".to_string()),
                });
            }
        };

        // Create loss function
        let loss_fn: Box<dyn LossFunction> = Box::new(MseLoss::new(config.smoothness_weight));

        // Create callbacks
        let mut callbacks: Vec<Box<dyn Callback>> = Vec::new();

        // Add early stopping
        callbacks.push(Box::new(EarlyStoppingCallback::new(
            config.patience,
            1e-6, // min_delta
        )));

        // Add checkpointing
        if config.checkpoint_frequency > 0 {
            callbacks.push(Box::new(CheckpointCallback::new(
                config.checkpoint_frequency,
                "checkpoints".to_string(),
            )));
        }

        Ok(Self {
            config,
            optimizer,
            loss_fn,
            callbacks,
            history: TrainingHistory {
                train_losses: Vec::new(),
                val_losses: Vec::new(),
                learning_rates: Vec::new(),
                epoch_times: Vec::new(),
                metrics: Vec::new(),
            },
            current_epoch: 0,
            best_val_loss: f64::INFINITY,
            patience_counter: 0,
        })
    }

    /// Train System A (traditional approach)
    pub fn train_system_a(
        &mut self,
        model: &mut SystemA,
        data: &DataSplits,
    ) -> Result<TrainingResult> {
        log::info!("Starting System A training");
        let start_time = Instant::now();

        data.validate()?;

        for epoch in 0..self.config.epochs {
            self.current_epoch = epoch;
            let epoch_start = Instant::now();

            // Training phase
            let train_loss = self.train_epoch_system_a(model, &data.train)?;

            // Validation phase
            let val_loss = self.evaluate_system_a(model, &data.val)?;

            // Update learning rate
            let current_lr = self.optimizer.get_learning_rate();

            // Create epoch metrics
            let metrics = EpochMetrics {
                epoch,
                samples_processed: data.train.len(),
                avg_gradient_norm: 0.0, // Would be computed during training
                param_update_norm: 0.0, // Would be computed during optimization
                memory_usage_bytes: model.memory_usage(),
                system_b_metrics: None,
            };

            // Update history
            self.history.train_losses.push(train_loss);
            self.history.val_losses.push(val_loss);
            self.history.learning_rates.push(current_lr);
            self.history.epoch_times.push(epoch_start.elapsed().as_secs_f64());
            self.history.metrics.push(metrics);

            // Check for improvement
            if val_loss < self.best_val_loss {
                self.best_val_loss = val_loss;
                self.patience_counter = 0;
            } else {
                self.patience_counter += 1;
            }

            // Early stopping check
            if self.patience_counter >= self.config.patience {
                log::info!("Early stopping triggered at epoch {}", epoch);
                break;
            }

            // Progress logging
            if epoch % self.config.val_frequency == 0 {
                log::info!(
                    "Epoch {}: train_loss={:.6}, val_loss={:.6}, lr={:.6}",
                    epoch, train_loss, val_loss, current_lr
                );
            }
        }

        let total_time = start_time.elapsed().as_secs_f64();
        let converged = self.patience_counter < self.config.patience;

        Ok(TrainingResult {
            history: self.history.clone(),
            final_loss: self.history.train_losses.last().copied().unwrap_or(f64::INFINITY),
            converged,
            total_time_seconds: total_time,
            best_val_loss: self.best_val_loss,
            best_epoch: self.find_best_epoch(),
        })
    }

    /// Train System B (temporal solver approach)
    pub fn train_system_b(
        &mut self,
        model: &mut SystemB,
        data: &DataSplits,
    ) -> Result<TrainingResult> {
        log::info!("Starting System B training with temporal solver");
        let start_time = Instant::now();

        data.validate()?;

        for epoch in 0..self.config.epochs {
            self.current_epoch = epoch;
            let epoch_start = Instant::now();

            // Training phase with active selection
            let (train_loss, system_b_metrics) = if epoch < 2 {
                // First 2 epochs: use all data like System A
                let loss = self.train_epoch_system_b_full(model, &data.train)?;
                (loss, self.compute_system_b_metrics(model)?)
            } else {
                // From epoch 3: use active selection
                let (loss, metrics) = self.train_epoch_system_b_active(model, &data.train)?;
                (loss, metrics)
            };

            // Validation phase
            let val_loss = self.evaluate_system_b(model, &data.val)?;

            // Update learning rate
            let current_lr = self.optimizer.get_learning_rate();

            // Create epoch metrics
            let metrics = EpochMetrics {
                epoch,
                samples_processed: data.train.len(),
                avg_gradient_norm: 0.0, // Would be computed during training
                param_update_norm: 0.0, // Would be computed during optimization
                memory_usage_bytes: model.memory_usage(),
                system_b_metrics: Some(system_b_metrics),
            };

            // Update history
            self.history.train_losses.push(train_loss);
            self.history.val_losses.push(val_loss);
            self.history.learning_rates.push(current_lr);
            self.history.epoch_times.push(epoch_start.elapsed().as_secs_f64());
            self.history.metrics.push(metrics);

            // Check for improvement
            if val_loss < self.best_val_loss {
                self.best_val_loss = val_loss;
                self.patience_counter = 0;
            } else {
                self.patience_counter += 1;
            }

            // Early stopping check
            if self.patience_counter >= self.config.patience {
                log::info!("Early stopping triggered at epoch {}", epoch);
                break;
            }

            // Progress logging
            if epoch % self.config.val_frequency == 0 {
                log::info!(
                    "Epoch {}: train_loss={:.6}, val_loss={:.6}, gate_pass_rate={:.3}, lr={:.6}",
                    epoch, train_loss, val_loss,
                    self.history.metrics.last().unwrap().system_b_metrics.as_ref()
                        .map_or(0.0, |m| m.gate_pass_rate),
                    current_lr
                );
            }
        }

        let total_time = start_time.elapsed().as_secs_f64();
        let converged = self.patience_counter < self.config.patience;

        Ok(TrainingResult {
            history: self.history.clone(),
            final_loss: self.history.train_losses.last().copied().unwrap_or(f64::INFINITY),
            converged,
            total_time_seconds: total_time,
            best_val_loss: self.best_val_loss,
            best_epoch: self.find_best_epoch(),
        })
    }

    /// Train one epoch for System A
    fn train_epoch_system_a(&mut self, model: &mut SystemA, samples: &[WindowedSample]) -> Result<f64> {
        let mut total_loss = 0.0;
        let mut sample_count = 0;

        // Process samples in batches
        for batch in samples.chunks(self.config.batch_size as usize) {
            let batch_loss = self.process_batch_system_a(model, batch)?;
            total_loss += batch_loss;
            sample_count += batch.len();
        }

        Ok(total_loss / sample_count as f64)
    }

    /// Train one epoch for System B (full data)
    fn train_epoch_system_b_full(&mut self, model: &mut SystemB, samples: &[WindowedSample]) -> Result<f64> {
        let mut total_loss = 0.0;
        let mut sample_count = 0;

        // Process samples in batches
        for batch in samples.chunks(self.config.batch_size as usize) {
            let batch_loss = self.process_batch_system_b(model, batch)?;
            total_loss += batch_loss;
            sample_count += batch.len();
        }

        Ok(total_loss / sample_count as f64)
    }

    /// Train one epoch for System B with active selection
    fn train_epoch_system_b_active(
        &mut self,
        model: &mut SystemB,
        samples: &[WindowedSample],
    ) -> Result<(f64, SystemBMetrics)> {
        // Get active selector
        let selector = model.active_selector()
            .ok_or_else(|| TemporalNeuralError::TrainingError {
                epoch: self.current_epoch as usize,
                message: "Active selector not available".to_string(),
                metrics: None,
            })?;

        // Extract embeddings and compute errors for all samples
        let (embeddings, errors) = self.extract_embeddings_and_errors(model, samples)?;

        // Add samples to selector
        selector.add_samples(&embeddings, &errors)?;

        // Select active samples
        let selected_indices = selector.select_samples()?;

        // Train on selected samples
        let selected_samples: Vec<&WindowedSample> = selected_indices
            .iter()
            .map(|&idx| &samples[idx])
            .collect();

        let mut total_loss = 0.0;
        let mut sample_count = 0;

        for batch in selected_samples.chunks(self.config.batch_size as usize) {
            let batch_loss = self.process_batch_system_b(model, batch)?;
            total_loss += batch_loss;
            sample_count += batch.len();
        }

        let avg_loss = total_loss / sample_count as f64;
        let metrics = self.compute_system_b_metrics(model)?;

        Ok((avg_loss, metrics))
    }

    /// Process a batch of samples for System A
    fn process_batch_system_a(&mut self, model: &mut SystemA, batch: &[&WindowedSample]) -> Result<f64> {
        let mut batch_loss = 0.0;

        for &sample in batch {
            // Forward pass
            let prediction = model.forward(&sample.input)?;

            // Compute loss
            let loss = self.loss_fn.compute_loss(&prediction, &sample.target)?;
            batch_loss += loss;

            // Backward pass (simplified - in practice would compute gradients)
            // This would involve computing gradients and updating parameters
        }

        // Apply optimizer (simplified)
        self.optimizer.step(model.parameters_mut())?;

        Ok(batch_loss / batch.len() as f64)
    }

    /// Process a batch of samples for System B
    fn process_batch_system_b(&mut self, model: &mut SystemB, batch: &[&WindowedSample]) -> Result<f64> {
        let mut batch_loss = 0.0;
        let mut predictions = Vec::new();
        let mut targets = Vec::new();

        for &sample in batch {
            // Forward pass with solver verification
            let prediction_result = model.predict_with_solver(&sample.input)?;

            // Update Kalman filter with ground truth
            model.update_kalman_state(&sample.target)?;

            // Store for batch loss computation
            predictions.push(prediction_result);
            targets.push(sample.target.clone());
        }

        // Compute residual learning loss
        let residual_loss = model.compute_residual_loss(&predictions, &targets)?;

        // Compute regularization terms
        let reg_loss = model.compute_regularization_loss(&predictions);

        batch_loss = residual_loss + reg_loss;

        // Apply optimizer (simplified)
        self.optimizer.step(model.parameters_mut())?;

        Ok(batch_loss)
    }

    /// Evaluate System A on validation/test data
    fn evaluate_system_a(&self, model: &SystemA, samples: &[WindowedSample]) -> Result<f64> {
        let mut total_loss = 0.0;
        let mut sample_count = 0;

        for sample in samples {
            let prediction = model.forward(&sample.input)?;
            let loss = self.loss_fn.compute_loss(&prediction, &sample.target)?;
            total_loss += loss;
            sample_count += 1;
        }

        Ok(total_loss / sample_count as f64)
    }

    /// Evaluate System B on validation/test data
    fn evaluate_system_b(&self, model: &SystemB, samples: &[WindowedSample]) -> Result<f64> {
        let mut total_loss = 0.0;
        let mut sample_count = 0;

        for sample in samples {
            // Use simple forward pass for evaluation (without solver verification for speed)
            let prediction = model.forward(&sample.input)?;
            let loss = self.loss_fn.compute_loss(&prediction, &sample.target)?;
            total_loss += loss;
            sample_count += 1;
        }

        Ok(total_loss / sample_count as f64)
    }

    /// Extract embeddings and compute errors for active selection
    fn extract_embeddings_and_errors(
        &self,
        model: &SystemB,
        samples: &[WindowedSample],
    ) -> Result<(Vec<DVector<f64>>, Vec<f64>)> {
        let mut embeddings = Vec::new();
        let mut errors = Vec::new();

        for sample in samples {
            // Get prediction
            let prediction = model.forward(&sample.input)?;

            // Compute error
            let error = (&prediction - &sample.target).norm();
            errors.push(error);

            // For embeddings, we'd extract hidden layer activations
            // For simplicity, use a hash of the input as embedding
            let embedding = self.compute_simple_embedding(&sample.input);
            embeddings.push(embedding);
        }

        Ok((embeddings, errors))
    }

    /// Compute simple embedding (placeholder)
    fn compute_simple_embedding(&self, input: &DMatrix<f64>) -> DVector<f64> {
        // Simplified: use mean and std of each feature as embedding
        let mut embedding = Vec::new();

        for i in 0..input.nrows() {
            let row_data: Vec<f64> = input.row(i).iter().cloned().collect();
            let mean = row_data.iter().sum::<f64>() / row_data.len() as f64;
            let variance = row_data.iter()
                .map(|x| (x - mean).powi(2))
                .sum::<f64>() / row_data.len() as f64;

            embedding.push(mean);
            embedding.push(variance.sqrt());
        }

        DVector::from_vec(embedding)
    }

    /// Compute System B specific metrics
    fn compute_system_b_metrics(&self, model: &SystemB) -> Result<SystemBMetrics> {
        let solver_stats = model.get_solver_stats();

        Ok(SystemBMetrics {
            gate_pass_rate: solver_stats.gate_pass_rate,
            avg_certificate_error: solver_stats.avg_certificate_error,
            kalman_prediction_error: solver_stats.kalman_prediction_error,
            active_selection_efficiency: 1.0, // Would be computed from selector stats
            residual_loss: 0.0, // Would be tracked during training
        })
    }

    /// Find the epoch with the best validation loss
    fn find_best_epoch(&self) -> u32 {
        self.history.val_losses
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(idx, _)| idx as u32)
            .unwrap_or(0)
    }

    /// Get training history
    pub fn get_history(&self) -> &TrainingHistory {
        &self.history
    }

    /// Reset trainer for new training run
    pub fn reset(&mut self) {
        self.history = TrainingHistory {
            train_losses: Vec::new(),
            val_losses: Vec::new(),
            learning_rates: Vec::new(),
            epoch_times: Vec::new(),
            metrics: Vec::new(),
        };
        self.current_epoch = 0;
        self.best_val_loss = f64::INFINITY;
        self.patience_counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::{Config, ModelConfig, TrainingConfig},
        data::TimeSeriesData,
    };

    fn create_test_config() -> TrainingConfig {
        TrainingConfig {
            optimizer: "adam".to_string(),
            learning_rate: 1e-3,
            batch_size: 32,
            epochs: 5,
            patience: 10,
            val_frequency: 1,
            grad_clip: Some(1.0),
            weight_decay: 1e-4,
            smoothness_weight: 0.1,
            checkpoint_frequency: 0,
        }
    }

    fn create_test_data() -> DataSplits {
        // Create minimal test data
        let n_samples = 1000;
        let features = nalgebra::DMatrix::from_fn(4, n_samples, |i, j| {
            (i as f64 + j as f64 * 0.01).sin()
        });

        let data = TimeSeriesData::new(
            features,
            vec!["x".to_string(), "y".to_string(), "vx".to_string(), "vy".to_string()],
            100.0,
            "test".to_string(),
        );

        data.temporal_split(0.8, 0.1, 0.1).unwrap()
    }

    #[test]
    fn test_trainer_creation() {
        let config = create_test_config();
        let trainer = Trainer::new(config).unwrap();

        assert_eq!(trainer.current_epoch, 0);
        assert_eq!(trainer.best_val_loss, f64::INFINITY);
    }

    #[test]
    fn test_system_a_training() {
        let training_config = create_test_config();
        let mut trainer = Trainer::new(training_config).unwrap();

        let model_config = ModelConfig {
            model_type: "micro_gru".to_string(),
            hidden_size: 8,
            num_layers: 1,
            dropout: 0.0,
            residual: false,
            activation: "tanh".to_string(),
            layer_norm: false,
        };

        let mut model = SystemA::new(&model_config).unwrap();
        let data = create_test_data();

        // This is a simplified test - full training would require gradient computation
        // For now, just test that the training loop runs without errors
        let result = trainer.train_system_a(&mut model, &data);
        assert!(result.is_ok());
    }

    #[test]
    fn test_training_history() {
        let config = create_test_config();
        let trainer = Trainer::new(config).unwrap();

        let history = trainer.get_history();
        assert!(history.train_losses.is_empty());
        assert!(history.val_losses.is_empty());
    }
}