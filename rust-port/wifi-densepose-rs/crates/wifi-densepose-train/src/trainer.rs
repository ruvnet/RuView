//! Training loop orchestrator.
//!
//! This module will be implemented by the trainer agent. It currently provides
//! the public interface stubs so that the crate compiles as a whole.

use crate::config::TrainingConfig;

/// Orchestrates the full training loop: data loading, forward pass, loss
/// computation, back-propagation, validation, and checkpointing.
pub struct Trainer {
    config: TrainingConfig,
}

impl Trainer {
    /// Create a new `Trainer` from the given configuration.
    pub fn new(config: TrainingConfig) -> Self {
        Trainer { config }
    }

    /// Return a reference to the active training configuration.
    pub fn config(&self) -> &TrainingConfig {
        &self.config
    }
}
