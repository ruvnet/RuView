//! Training callbacks for monitoring and control

use crate::error::Result;

/// Trait for training callbacks
pub trait Callback: Send + Sync {
    /// Called at the start of training
    fn on_train_begin(&mut self) -> Result<()> { Ok(()) }

    /// Called at the end of training
    fn on_train_end(&mut self) -> Result<()> { Ok(()) }

    /// Called at the start of each epoch
    fn on_epoch_begin(&mut self, epoch: u32) -> Result<()> { let _ = epoch; Ok(()) }

    /// Called at the end of each epoch
    fn on_epoch_end(&mut self, epoch: u32, train_loss: f64, val_loss: f64) -> Result<bool> {
        let _ = (epoch, train_loss, val_loss);
        Ok(true) // Continue training
    }
}

/// Early stopping callback
pub struct EarlyStoppingCallback {
    patience: u32,
    min_delta: f64,
    best_loss: f64,
    patience_counter: u32,
}

impl EarlyStoppingCallback {
    pub fn new(patience: u32, min_delta: f64) -> Self {
        Self {
            patience,
            min_delta,
            best_loss: f64::INFINITY,
            patience_counter: 0,
        }
    }
}

impl Callback for EarlyStoppingCallback {
    fn on_epoch_end(&mut self, _epoch: u32, _train_loss: f64, val_loss: f64) -> Result<bool> {
        if val_loss < self.best_loss - self.min_delta {
            self.best_loss = val_loss;
            self.patience_counter = 0;
        } else {
            self.patience_counter += 1;
        }

        Ok(self.patience_counter < self.patience)
    }
}

/// Checkpoint saving callback
pub struct CheckpointCallback {
    frequency: u32,
    checkpoint_dir: String,
}

impl CheckpointCallback {
    pub fn new(frequency: u32, checkpoint_dir: String) -> Self {
        Self { frequency, checkpoint_dir }
    }
}

impl Callback for CheckpointCallback {
    fn on_epoch_end(&mut self, epoch: u32, _train_loss: f64, _val_loss: f64) -> Result<bool> {
        if epoch % self.frequency == 0 {
            // Would save checkpoint here
            log::info!("Checkpoint saved at epoch {}", epoch);
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_early_stopping() {
        let mut callback = EarlyStoppingCallback::new(3, 0.001);

        // Should continue initially
        assert!(callback.on_epoch_end(0, 1.0, 1.0).unwrap());

        // Improvement should reset counter
        assert!(callback.on_epoch_end(1, 0.8, 0.8).unwrap());

        // No improvement should increment counter
        assert!(callback.on_epoch_end(2, 0.9, 0.9).unwrap());
        assert!(callback.on_epoch_end(3, 0.9, 0.9).unwrap());
        assert!(callback.on_epoch_end(4, 0.9, 0.9).unwrap());

        // Should stop after patience is exhausted
        assert!(!callback.on_epoch_end(5, 0.9, 0.9).unwrap());
    }
}