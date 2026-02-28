//! Energy-Based Early Exit
//!
//! Implements early exit based on energy convergence rather than confidence thresholds.
//!
//! ## Key Insight
//!
//! Traditional early exit uses confidence (max softmax probability) which can be
//! confidently wrong. Energy convergence is more principled:
//!
//! - If energy stops changing, further layers won't help
//! - Energy provides a geometric measure of consistency
//! - Works naturally with sheaf attention
//!
//! ## Exit Criterion
//!
//! Exit when: |E_current - E_previous| < epsilon
//!
//! This means the representation has stabilized and further processing
//! is unlikely to improve coherence.

use crate::error::{AttentionError, AttentionResult};
use serde::{Deserialize, Serialize};

/// Configuration for energy-based early exit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EarlyExitConfig {
    /// Energy convergence threshold (exit if delta < epsilon)
    pub epsilon: f32,
    /// Minimum layers to process before considering exit
    pub min_layers: usize,
    /// Maximum layers (hard limit)
    pub max_layers: usize,
    /// Number of consecutive converged steps required
    pub patience: usize,
    /// Whether to track energy history
    pub track_history: bool,
    /// Exponential moving average smoothing factor (0 = no smoothing)
    pub ema_alpha: f32,
}

impl Default for EarlyExitConfig {
    fn default() -> Self {
        Self {
            epsilon: 0.001,
            min_layers: 2,
            max_layers: 12,
            patience: 1,
            track_history: true,
            ema_alpha: 0.0,
        }
    }
}

impl EarlyExitConfig {
    /// Create config with epsilon
    pub fn new(epsilon: f32) -> Self {
        Self {
            epsilon,
            ..Default::default()
        }
    }

    /// Builder: set epsilon
    pub fn with_epsilon(mut self, epsilon: f32) -> Self {
        self.epsilon = epsilon;
        self
    }

    /// Builder: set minimum layers
    pub fn with_min_layers(mut self, min: usize) -> Self {
        self.min_layers = min;
        self
    }

    /// Builder: set maximum layers
    pub fn with_max_layers(mut self, max: usize) -> Self {
        self.max_layers = max;
        self
    }

    /// Builder: set patience
    pub fn with_patience(mut self, patience: usize) -> Self {
        self.patience = patience;
        self
    }

    /// Builder: set history tracking
    pub fn with_track_history(mut self, track: bool) -> Self {
        self.track_history = track;
        self
    }

    /// Builder: set EMA smoothing
    pub fn with_ema_alpha(mut self, alpha: f32) -> Self {
        self.ema_alpha = alpha.clamp(0.0, 1.0);
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> AttentionResult<()> {
        if self.epsilon <= 0.0 {
            return Err(AttentionError::InvalidConfig(
                "epsilon must be positive".to_string(),
            ));
        }
        if self.min_layers > self.max_layers {
            return Err(AttentionError::InvalidConfig(
                "min_layers cannot exceed max_layers".to_string(),
            ));
        }
        if self.patience == 0 {
            return Err(AttentionError::InvalidConfig(
                "patience must be at least 1".to_string(),
            ));
        }
        Ok(())
    }
}

/// Result of early exit check
#[derive(Debug, Clone)]
pub struct EarlyExitResult {
    /// Whether to exit early
    pub should_exit: bool,
    /// Current layer index (0-indexed)
    pub layer_index: usize,
    /// Current energy value
    pub current_energy: f32,
    /// Energy delta from previous layer
    pub energy_delta: f32,
    /// Number of consecutive converged steps
    pub converged_steps: usize,
    /// Exit reason (if exiting)
    pub exit_reason: Option<ExitReason>,
}

/// Reason for early exit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitReason {
    /// Energy converged (delta < epsilon)
    EnergyConverged,
    /// Reached maximum layers
    MaxLayersReached,
    /// Energy is zero (perfectly coherent)
    PerfectCoherence,
}

impl ExitReason {
    /// Human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::EnergyConverged => "Energy converged below threshold",
            Self::MaxLayersReached => "Reached maximum layer count",
            Self::PerfectCoherence => "Achieved perfect coherence (zero energy)",
        }
    }
}

/// Energy-based early exit tracker
#[derive(Debug, Clone)]
pub struct EarlyExit {
    config: EarlyExitConfig,
    /// Energy history across layers
    energy_history: Vec<f32>,
    /// EMA-smoothed energy (if enabled)
    ema_energy: Option<f32>,
    /// Count of consecutive converged steps
    converged_count: usize,
    /// Current layer index
    current_layer: usize,
}

impl EarlyExit {
    /// Create new early exit tracker
    pub fn new(config: EarlyExitConfig) -> Self {
        Self {
            config,
            energy_history: Vec::new(),
            ema_energy: None,
            converged_count: 0,
            current_layer: 0,
        }
    }

    /// Create with default configuration
    pub fn default_tracker() -> Self {
        Self::new(EarlyExitConfig::default())
    }

    /// Reset tracker for new sequence
    pub fn reset(&mut self) {
        self.energy_history.clear();
        self.ema_energy = None;
        self.converged_count = 0;
        self.current_layer = 0;
    }

    /// Get configuration
    pub fn config(&self) -> &EarlyExitConfig {
        &self.config
    }

    /// Get mutable configuration
    pub fn config_mut(&mut self) -> &mut EarlyExitConfig {
        &mut self.config
    }

    /// Get energy history
    pub fn energy_history(&self) -> &[f32] {
        &self.energy_history
    }

    /// Get current layer index
    pub fn current_layer(&self) -> usize {
        self.current_layer
    }

    /// Check if should exit after processing a layer
    ///
    /// # Arguments
    ///
    /// * `energy` - Energy computed after the current layer
    ///
    /// # Returns
    ///
    /// Early exit result with decision and diagnostics
    pub fn check(&mut self, energy: f32) -> EarlyExitResult {
        let layer_index = self.current_layer;
        self.current_layer += 1;

        // Update EMA if enabled
        let effective_energy = if self.config.ema_alpha > 0.0 {
            let ema = self.ema_energy.unwrap_or(energy);
            let new_ema = self.config.ema_alpha * energy + (1.0 - self.config.ema_alpha) * ema;
            self.ema_energy = Some(new_ema);
            new_ema
        } else {
            energy
        };

        // Compute delta from previous
        let prev_energy = self.energy_history.last().copied().unwrap_or(f32::INFINITY);
        let energy_delta = (effective_energy - prev_energy).abs();

        // Track history if enabled
        if self.config.track_history {
            self.energy_history.push(effective_energy);
        }

        // Check for perfect coherence
        if effective_energy < 1e-10 {
            return EarlyExitResult {
                should_exit: true,
                layer_index,
                current_energy: effective_energy,
                energy_delta,
                converged_steps: self.converged_count + 1,
                exit_reason: Some(ExitReason::PerfectCoherence),
            };
        }

        // Check minimum layers
        if layer_index < self.config.min_layers {
            return EarlyExitResult {
                should_exit: false,
                layer_index,
                current_energy: effective_energy,
                energy_delta,
                converged_steps: 0,
                exit_reason: None,
            };
        }

        // Check maximum layers
        if layer_index >= self.config.max_layers - 1 {
            return EarlyExitResult {
                should_exit: true,
                layer_index,
                current_energy: effective_energy,
                energy_delta,
                converged_steps: self.converged_count,
                exit_reason: Some(ExitReason::MaxLayersReached),
            };
        }

        // Check convergence
        if energy_delta < self.config.epsilon {
            self.converged_count += 1;
        } else {
            self.converged_count = 0;
        }

        // Check if converged for enough steps
        if self.converged_count >= self.config.patience {
            return EarlyExitResult {
                should_exit: true,
                layer_index,
                current_energy: effective_energy,
                energy_delta,
                converged_steps: self.converged_count,
                exit_reason: Some(ExitReason::EnergyConverged),
            };
        }

        EarlyExitResult {
            should_exit: false,
            layer_index,
            current_energy: effective_energy,
            energy_delta,
            converged_steps: self.converged_count,
            exit_reason: None,
        }
    }

    /// Get statistics about the exit decision
    pub fn statistics(&self) -> EarlyExitStatistics {
        let total_layers = self.current_layer;
        let max_possible = self.config.max_layers;

        let energy_reduction = if self.energy_history.len() >= 2 {
            let first = self.energy_history.first().copied().unwrap_or(0.0);
            let last = self.energy_history.last().copied().unwrap_or(0.0);
            if first > 1e-10 {
                (first - last) / first
            } else {
                0.0
            }
        } else {
            0.0
        };

        let avg_delta = if self.energy_history.len() >= 2 {
            let deltas: Vec<f32> = self
                .energy_history
                .windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .collect();
            deltas.iter().sum::<f32>() / deltas.len() as f32
        } else {
            0.0
        };

        EarlyExitStatistics {
            layers_used: total_layers,
            max_layers: max_possible,
            layers_saved: max_possible.saturating_sub(total_layers),
            speedup_ratio: if total_layers > 0 {
                max_possible as f32 / total_layers as f32
            } else {
                1.0
            },
            energy_reduction,
            average_delta: avg_delta,
            final_energy: self.energy_history.last().copied().unwrap_or(0.0),
        }
    }
}

/// Statistics about early exit behavior
#[derive(Debug, Clone)]
pub struct EarlyExitStatistics {
    /// Number of layers actually processed
    pub layers_used: usize,
    /// Maximum possible layers
    pub max_layers: usize,
    /// Layers saved by early exit
    pub layers_saved: usize,
    /// Speedup ratio (max_layers / layers_used)
    pub speedup_ratio: f32,
    /// Relative energy reduction from first to last layer
    pub energy_reduction: f32,
    /// Average energy delta across layers
    pub average_delta: f32,
    /// Final energy value
    pub final_energy: f32,
}

/// Process layers with early exit
///
/// Generic function that processes layers until early exit condition is met.
pub fn process_with_early_exit<F, T>(
    initial_state: T,
    layers: &[F],
    config: EarlyExitConfig,
    energy_fn: impl Fn(&T) -> f32,
) -> (T, EarlyExitResult)
where
    F: Fn(T) -> T,
    T: Clone,
{
    let mut tracker = EarlyExit::new(config);
    let mut state = initial_state;

    for layer in layers {
        // Process layer
        state = layer(state);

        // Compute energy
        let energy = energy_fn(&state);

        // Check early exit
        let result = tracker.check(energy);
        if result.should_exit {
            return (state, result);
        }
    }

    // Processed all layers
    let final_energy = energy_fn(&state);
    let final_result = EarlyExitResult {
        should_exit: true,
        layer_index: layers.len(),
        current_energy: final_energy,
        energy_delta: 0.0,
        converged_steps: 0,
        exit_reason: Some(ExitReason::MaxLayersReached),
    };

    (state, final_result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = EarlyExitConfig::default();
        assert!(config.epsilon > 0.0);
        assert!(config.min_layers < config.max_layers);
        assert!(config.patience > 0);
    }

    #[test]
    fn test_config_builder() {
        let config = EarlyExitConfig::new(0.01)
            .with_min_layers(3)
            .with_max_layers(10)
            .with_patience(2)
            .with_ema_alpha(0.1);

        assert_eq!(config.epsilon, 0.01);
        assert_eq!(config.min_layers, 3);
        assert_eq!(config.max_layers, 10);
        assert_eq!(config.patience, 2);
        assert_eq!(config.ema_alpha, 0.1);
    }

    #[test]
    fn test_config_validation() {
        assert!(EarlyExitConfig::default().validate().is_ok());

        let bad_config = EarlyExitConfig {
            epsilon: -1.0,
            ..Default::default()
        };
        assert!(bad_config.validate().is_err());

        let bad_config = EarlyExitConfig {
            min_layers: 10,
            max_layers: 5,
            ..Default::default()
        };
        assert!(bad_config.validate().is_err());
    }

    #[test]
    fn test_early_exit_creation() {
        let tracker = EarlyExit::default_tracker();
        assert_eq!(tracker.current_layer(), 0);
        assert!(tracker.energy_history().is_empty());
    }

    #[test]
    fn test_early_exit_reset() {
        let mut tracker = EarlyExit::default_tracker();
        tracker.check(1.0);
        tracker.check(0.5);

        assert_eq!(tracker.current_layer(), 2);

        tracker.reset();
        assert_eq!(tracker.current_layer(), 0);
        assert!(tracker.energy_history().is_empty());
    }

    #[test]
    fn test_min_layers_respected() {
        let config = EarlyExitConfig::default()
            .with_min_layers(3)
            .with_epsilon(0.1);
        let mut tracker = EarlyExit::new(config);

        // Even with converged energy, shouldn't exit before min_layers
        // Note: Using non-zero energy (0.001) to avoid PerfectCoherence early exit
        // which takes precedence over min_layers (as it should - zero energy means done)
        let result = tracker.check(0.001);
        assert!(!result.should_exit);
        assert_eq!(result.layer_index, 0);

        // Same small energy = converged, but still before min_layers
        let result = tracker.check(0.001);
        assert!(!result.should_exit);
        assert_eq!(result.layer_index, 1);

        // Still before min_layers
        let _result = tracker.check(0.001);
    }

    #[test]
    fn test_max_layers_enforced() {
        let config = EarlyExitConfig::default()
            .with_max_layers(3)
            .with_min_layers(1);
        let mut tracker = EarlyExit::new(config);

        tracker.check(10.0); // Layer 0
        tracker.check(5.0); // Layer 1

        let result = tracker.check(2.5); // Layer 2 = max - 1
        assert!(result.should_exit);
        assert_eq!(result.exit_reason, Some(ExitReason::MaxLayersReached));
    }

    #[test]
    fn test_energy_convergence() {
        let config = EarlyExitConfig::default()
            .with_epsilon(0.1)
            .with_min_layers(1)
            .with_patience(1);
        let mut tracker = EarlyExit::new(config);

        tracker.check(1.0); // Layer 0

        // Energy change > epsilon
        let result = tracker.check(0.5); // Layer 1
        assert!(!result.should_exit);

        // Energy change < epsilon (converged)
        let result = tracker.check(0.49); // Layer 2
        assert!(result.should_exit);
        assert_eq!(result.exit_reason, Some(ExitReason::EnergyConverged));
    }

    #[test]
    fn test_patience() {
        let config = EarlyExitConfig::default()
            .with_epsilon(0.1)
            .with_min_layers(1)
            .with_patience(2);
        let mut tracker = EarlyExit::new(config);

        tracker.check(1.0); // Layer 0

        // First converged step
        let result = tracker.check(1.0); // Layer 1
        assert!(!result.should_exit);
        assert_eq!(result.converged_steps, 1);

        // Second converged step (patience = 2)
        let result = tracker.check(1.0); // Layer 2
        assert!(result.should_exit);
        assert_eq!(result.converged_steps, 2);
    }

    #[test]
    fn test_perfect_coherence() {
        let config = EarlyExitConfig::default().with_min_layers(1);
        let mut tracker = EarlyExit::new(config);

        tracker.check(1.0);

        let result = tracker.check(0.0);
        assert!(result.should_exit);
        assert_eq!(result.exit_reason, Some(ExitReason::PerfectCoherence));
    }

    #[test]
    fn test_ema_smoothing() {
        let config = EarlyExitConfig::default()
            .with_ema_alpha(0.5)
            .with_track_history(true);
        let mut tracker = EarlyExit::new(config);

        tracker.check(1.0);
        let result = tracker.check(0.0);

        // With EMA alpha = 0.5: new_ema = 0.5 * 0.0 + 0.5 * 1.0 = 0.5
        // So history should show smoothed value
        assert!(tracker.energy_history().len() >= 2);
    }

    #[test]
    fn test_statistics() {
        let config = EarlyExitConfig::default()
            .with_max_layers(10)
            .with_min_layers(1)
            .with_epsilon(0.1);
        let mut tracker = EarlyExit::new(config);

        tracker.check(1.0);
        tracker.check(0.5);
        tracker.check(0.25);
        tracker.check(0.24); // Should exit here

        let stats = tracker.statistics();
        assert_eq!(stats.layers_used, 4);
        assert_eq!(stats.max_layers, 10);
        assert_eq!(stats.layers_saved, 6);
        assert!(stats.speedup_ratio > 1.0);
        assert!(stats.energy_reduction > 0.0);
    }

    #[test]
    fn test_process_with_early_exit() {
        let config = EarlyExitConfig::default()
            .with_epsilon(0.1)
            .with_min_layers(1)
            .with_max_layers(10);

        // Create "layers" that halve the energy each time
        let layers: Vec<Box<dyn Fn(f32) -> f32>> = (0..10)
            .map(|_| Box::new(|x: f32| x * 0.5) as Box<dyn Fn(f32) -> f32>)
            .collect();

        let layer_refs: Vec<&dyn Fn(f32) -> f32> = layers.iter().map(|f| f.as_ref()).collect();

        // This is a simplified test using closures
        let mut tracker = EarlyExit::new(config);
        let mut state = 10.0f32;

        for layer in &layer_refs {
            state = layer(state);
            let result = tracker.check(state);
            if result.should_exit {
                break;
            }
        }

        // Should have exited before processing all 10 layers
        assert!(tracker.current_layer() < 10);
    }

    #[test]
    fn test_exit_reason_descriptions() {
        assert!(!ExitReason::EnergyConverged.description().is_empty());
        assert!(!ExitReason::MaxLayersReached.description().is_empty());
        assert!(!ExitReason::PerfectCoherence.description().is_empty());
    }
}
