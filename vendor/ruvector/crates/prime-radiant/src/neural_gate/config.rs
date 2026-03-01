//! Configuration types for the neural coherence gate.

use serde::{Deserialize, Serialize};

/// Configuration for the neural coherence gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeuralGateConfig {
    /// Hysteresis configuration.
    pub hysteresis: HysteresisConfig,
    /// Global workspace configuration.
    pub workspace: WorkspaceConfig,
    /// Oscillator configuration for routing.
    pub oscillator: OscillatorConfig,
    /// HDC hypervector dimension.
    pub hdc_dimension: usize,
    /// Memory capacity for witness storage.
    pub memory_capacity: usize,
    /// Dendritic coincidence window in microseconds.
    pub coincidence_window_us: u64,
    /// Number of dendritic branches.
    pub num_branches: usize,
    /// Enable oscillatory routing.
    pub enable_oscillatory_routing: bool,
}

impl Default for NeuralGateConfig {
    fn default() -> Self {
        Self {
            hysteresis: HysteresisConfig::default(),
            workspace: WorkspaceConfig::default(),
            oscillator: OscillatorConfig::default(),
            hdc_dimension: 10000, // 10K-dimensional hypervectors
            memory_capacity: 10000,
            coincidence_window_us: 5000, // 5ms window
            num_branches: 8,
            enable_oscillatory_routing: true,
        }
    }
}

/// Configuration for hysteresis tracking.
///
/// Hysteresis prevents rapid oscillation between decision states
/// by requiring a threshold to be crossed by a margin before switching.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HysteresisConfig {
    /// Lower threshold for switching to "low" state.
    pub low_threshold: f32,
    /// Upper threshold for switching to "high" state.
    pub high_threshold: f32,
    /// Minimum time to stay in a state before switching (ms).
    pub min_dwell_time_ms: u64,
    /// Smoothing factor for energy (0 = no smoothing, 1 = full smoothing).
    pub smoothing_factor: f32,
}

impl Default for HysteresisConfig {
    fn default() -> Self {
        Self {
            low_threshold: 0.3,
            high_threshold: 0.7,
            min_dwell_time_ms: 100,
            smoothing_factor: 0.2,
        }
    }
}

impl HysteresisConfig {
    /// Create a sensitive hysteresis configuration (smaller band).
    #[must_use]
    pub fn sensitive() -> Self {
        Self {
            low_threshold: 0.4,
            high_threshold: 0.6,
            min_dwell_time_ms: 50,
            smoothing_factor: 0.1,
        }
    }

    /// Create a stable hysteresis configuration (larger band).
    #[must_use]
    pub fn stable() -> Self {
        Self {
            low_threshold: 0.2,
            high_threshold: 0.8,
            min_dwell_time_ms: 200,
            smoothing_factor: 0.3,
        }
    }

    /// Check if the configuration is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.low_threshold >= 0.0
            && self.high_threshold <= 1.0
            && self.low_threshold < self.high_threshold
            && self.smoothing_factor >= 0.0
            && self.smoothing_factor <= 1.0
    }
}

/// Configuration for the global workspace.
///
/// The global workspace implements the "conscious access" mechanism,
/// broadcasting significant decisions to all modules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Capacity of the workspace buffer.
    pub buffer_capacity: usize,
    /// Significance threshold for broadcast.
    pub broadcast_threshold: f32,
    /// Enable attention-based selection.
    pub attention_selection: bool,
    /// Competition decay factor.
    pub competition_decay: f32,
    /// Number of competitor slots.
    pub num_competitors: usize,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            buffer_capacity: 100,
            broadcast_threshold: 0.6,
            attention_selection: true,
            competition_decay: 0.9,
            num_competitors: 8,
        }
    }
}

/// Configuration for oscillatory routing.
///
/// Uses the Kuramoto model for phase-based routing of information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OscillatorConfig {
    /// Number of oscillators.
    pub num_oscillators: usize,
    /// Natural frequency (Hz).
    pub natural_frequency: f32,
    /// Coupling strength.
    pub coupling_strength: f32,
    /// Phase noise standard deviation.
    pub phase_noise: f32,
    /// Synchronization threshold for routing.
    pub sync_threshold: f32,
}

impl Default for OscillatorConfig {
    fn default() -> Self {
        Self {
            num_oscillators: 64,
            natural_frequency: 40.0, // Gamma band (40 Hz)
            coupling_strength: 0.5,
            phase_noise: 0.1,
            sync_threshold: 0.8,
        }
    }
}

impl OscillatorConfig {
    /// Create configuration for fast oscillations (beta band).
    #[must_use]
    pub fn beta_band() -> Self {
        Self {
            num_oscillators: 64,
            natural_frequency: 20.0, // Beta band
            coupling_strength: 0.4,
            phase_noise: 0.15,
            sync_threshold: 0.75,
        }
    }

    /// Create configuration for slow oscillations (theta band).
    #[must_use]
    pub fn theta_band() -> Self {
        Self {
            num_oscillators: 32,
            natural_frequency: 6.0, // Theta band
            coupling_strength: 0.6,
            phase_noise: 0.05,
            sync_threshold: 0.85,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hysteresis_validity() {
        assert!(HysteresisConfig::default().is_valid());
        assert!(HysteresisConfig::sensitive().is_valid());
        assert!(HysteresisConfig::stable().is_valid());

        let invalid = HysteresisConfig {
            low_threshold: 0.8,
            high_threshold: 0.3, // Less than low
            min_dwell_time_ms: 100,
            smoothing_factor: 0.2,
        };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_default_configs() {
        let config = NeuralGateConfig::default();
        assert_eq!(config.hdc_dimension, 10000);
        assert!(config.hysteresis.is_valid());
    }
}
