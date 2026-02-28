//! Configuration types for SONA threshold tuning.

use serde::{Deserialize, Serialize};

/// Configuration for the SONA threshold tuner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunerConfig {
    /// Hidden dimension for SONA engine.
    pub hidden_dim: usize,
    /// Embedding dimension.
    pub embedding_dim: usize,
    /// Initial threshold configuration.
    pub initial_thresholds: ThresholdConfig,
    /// Instant learning loop configuration.
    pub instant_loop: LearningLoopConfig,
    /// Background learning loop configuration.
    pub background_loop: LearningLoopConfig,
    /// EWC++ lambda for weight consolidation.
    pub ewc_lambda: f32,
    /// Pattern similarity threshold for reasoning bank queries.
    pub pattern_similarity_threshold: f32,
    /// Maximum patterns to store in reasoning bank.
    pub max_patterns: usize,
    /// Enable auto-consolidation after N trajectories.
    pub auto_consolidate_after: usize,
}

impl Default for TunerConfig {
    fn default() -> Self {
        Self {
            hidden_dim: 256,
            embedding_dim: 256,
            initial_thresholds: ThresholdConfig::default(),
            instant_loop: LearningLoopConfig::instant(),
            background_loop: LearningLoopConfig::background(),
            ewc_lambda: 0.4,
            pattern_similarity_threshold: 0.85,
            max_patterns: 10000,
            auto_consolidate_after: 100,
        }
    }
}

/// Threshold configuration for compute lanes.
///
/// The coherence gate uses these thresholds to determine which compute lane
/// to use based on the current energy level.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThresholdConfig {
    /// Energy threshold for Lane 0 (Reflex) - below this, allow without checks.
    pub reflex: f32,
    /// Energy threshold for Lane 1 (Retrieval) - requires evidence fetching.
    pub retrieval: f32,
    /// Energy threshold for Lane 2 (Heavy) - requires multi-step reasoning.
    pub heavy: f32,
    /// Persistence window in seconds before escalation.
    pub persistence_window_secs: u64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            reflex: 0.1,    // Low energy: proceed without checks
            retrieval: 0.3, // Medium energy: fetch evidence
            heavy: 0.7,     // High energy: deep reasoning
            persistence_window_secs: 5,
        }
    }
}

impl ThresholdConfig {
    /// Create a conservative threshold configuration.
    #[must_use]
    pub fn conservative() -> Self {
        Self {
            reflex: 0.05,
            retrieval: 0.15,
            heavy: 0.5,
            persistence_window_secs: 10,
        }
    }

    /// Create an aggressive threshold configuration.
    #[must_use]
    pub fn aggressive() -> Self {
        Self {
            reflex: 0.2,
            retrieval: 0.5,
            heavy: 0.9,
            persistence_window_secs: 2,
        }
    }

    /// Check if the configuration is valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.reflex >= 0.0
            && self.retrieval > self.reflex
            && self.heavy > self.retrieval
            && self.heavy <= 1.0
    }

    /// Get the compute lane for a given energy level.
    #[must_use]
    pub fn lane_for_energy(&self, energy: f32) -> ComputeLane {
        if energy < self.reflex {
            ComputeLane::Reflex
        } else if energy < self.retrieval {
            ComputeLane::Retrieval
        } else if energy < self.heavy {
            ComputeLane::Heavy
        } else {
            ComputeLane::Human
        }
    }

    /// Interpolate between two configurations.
    #[must_use]
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            reflex: self.reflex + (other.reflex - self.reflex) * t,
            retrieval: self.retrieval + (other.retrieval - self.retrieval) * t,
            heavy: self.heavy + (other.heavy - self.heavy) * t,
            persistence_window_secs: if t < 0.5 {
                self.persistence_window_secs
            } else {
                other.persistence_window_secs
            },
        }
    }
}

/// Compute lanes for escalating complexity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComputeLane {
    /// Lane 0: Local residual updates, simple aggregates (<1ms).
    Reflex = 0,
    /// Lane 1: Evidence fetching, lightweight reasoning (~10ms).
    Retrieval = 1,
    /// Lane 2: Multi-step planning, spectral analysis (~100ms).
    Heavy = 2,
    /// Lane 3: Human escalation for sustained incoherence.
    Human = 3,
}

/// Configuration for a learning loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearningLoopConfig {
    /// Learning rate.
    pub learning_rate: f32,
    /// LoRA rank (1-2 for Micro-LoRA, higher for Base-LoRA).
    pub lora_rank: usize,
    /// Batch size for updates.
    pub batch_size: usize,
    /// Maximum latency target in microseconds.
    pub max_latency_us: u64,
    /// Enable gradient clipping.
    pub gradient_clipping: bool,
    /// Gradient clip value.
    pub gradient_clip_value: f32,
}

impl LearningLoopConfig {
    /// Create configuration for instant (Micro-LoRA) loop.
    #[must_use]
    pub fn instant() -> Self {
        Self {
            learning_rate: 0.01,
            lora_rank: 1, // Ultra-low rank for speed
            batch_size: 1,
            max_latency_us: 50, // <0.05ms target
            gradient_clipping: true,
            gradient_clip_value: 1.0,
        }
    }

    /// Create configuration for background (Base-LoRA) loop.
    #[must_use]
    pub fn background() -> Self {
        Self {
            learning_rate: 0.001,
            lora_rank: 8, // Higher rank for better learning
            batch_size: 32,
            max_latency_us: 10_000, // 10ms is fine for background
            gradient_clipping: true,
            gradient_clip_value: 1.0,
        }
    }
}

impl Default for LearningLoopConfig {
    fn default() -> Self {
        Self::instant()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_threshold_config_validity() {
        assert!(ThresholdConfig::default().is_valid());
        assert!(ThresholdConfig::conservative().is_valid());
        assert!(ThresholdConfig::aggressive().is_valid());

        let invalid = ThresholdConfig {
            reflex: 0.5,
            retrieval: 0.3, // Less than reflex
            heavy: 0.7,
            persistence_window_secs: 5,
        };
        assert!(!invalid.is_valid());
    }

    #[test]
    fn test_lane_for_energy() {
        let config = ThresholdConfig::default();

        assert_eq!(config.lane_for_energy(0.0), ComputeLane::Reflex);
        assert_eq!(config.lane_for_energy(0.05), ComputeLane::Reflex);
        assert_eq!(config.lane_for_energy(0.15), ComputeLane::Retrieval);
        assert_eq!(config.lane_for_energy(0.5), ComputeLane::Heavy);
        assert_eq!(config.lane_for_energy(1.0), ComputeLane::Human);
    }

    #[test]
    fn test_threshold_lerp() {
        let conservative = ThresholdConfig::conservative();
        let aggressive = ThresholdConfig::aggressive();

        let mid = conservative.lerp(&aggressive, 0.5);
        assert!(mid.reflex > conservative.reflex);
        assert!(mid.reflex < aggressive.reflex);
    }
}
