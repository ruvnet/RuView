//! Attention Control Policy
//!
//! 3-mode policy for controlling attention based on coherence:
//! - Stable: full attention width
//! - Cautious: reduced width, increased sparsity
//! - Freeze: retrieval only, no updates

use serde::{Deserialize, Serialize};

/// Attention operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttentionMode {
    /// Full attention, normal updates
    Stable,
    /// Reduced attention width, increased sparsity
    Cautious,
    /// Retrieval only, no updates, no writes
    Freeze,
}

/// Policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    /// Coherence threshold for stable mode (above this = stable)
    pub stable_threshold: f32,
    /// Coherence threshold for freeze mode (below this = freeze)
    pub freeze_threshold: f32,
    /// Attention width multiplier in cautious mode (0.5 = half width)
    pub cautious_width_factor: f32,
    /// Sparsity increase in cautious mode (2.0 = twice as sparse)
    pub cautious_sparsity_factor: f32,
    /// How many tokens between coherence updates
    pub update_period: usize,
    /// Hysteresis factor to prevent mode oscillation
    pub hysteresis: f32,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            stable_threshold: 0.7,
            freeze_threshold: 0.3,
            cautious_width_factor: 0.5,
            cautious_sparsity_factor: 2.0,
            update_period: 4,
            hysteresis: 0.05,
        }
    }
}

/// Attention control policy
#[derive(Debug, Clone)]
pub struct AttentionPolicy {
    config: PolicyConfig,
    current_mode: AttentionMode,
    mode_history: Vec<AttentionMode>,
}

impl AttentionPolicy {
    /// Create new policy
    pub fn new(config: PolicyConfig) -> Self {
        Self {
            config,
            current_mode: AttentionMode::Stable,
            mode_history: Vec::new(),
        }
    }

    /// Determine mode from coherence score
    pub fn determine_mode(&mut self, coherence: f32) -> AttentionMode {
        let new_mode = self.compute_mode(coherence);

        // Apply hysteresis to prevent oscillation
        let mode = self.apply_hysteresis(new_mode, coherence);

        // Record history
        self.mode_history.push(mode);
        if self.mode_history.len() > 16 {
            self.mode_history.remove(0);
        }

        self.current_mode = mode;
        mode
    }

    /// Compute mode without hysteresis
    fn compute_mode(&self, coherence: f32) -> AttentionMode {
        if coherence >= self.config.stable_threshold {
            AttentionMode::Stable
        } else if coherence <= self.config.freeze_threshold {
            AttentionMode::Freeze
        } else {
            AttentionMode::Cautious
        }
    }

    /// Apply hysteresis to mode transitions
    fn apply_hysteresis(&self, new_mode: AttentionMode, coherence: f32) -> AttentionMode {
        let h = self.config.hysteresis;

        match (self.current_mode, new_mode) {
            // Stable -> Cautious: require coherence to drop below threshold - hysteresis
            (AttentionMode::Stable, AttentionMode::Cautious) => {
                if coherence < self.config.stable_threshold - h {
                    AttentionMode::Cautious
                } else {
                    AttentionMode::Stable
                }
            }
            // Cautious -> Stable: require coherence to rise above threshold + hysteresis
            (AttentionMode::Cautious, AttentionMode::Stable) => {
                if coherence > self.config.stable_threshold + h {
                    AttentionMode::Stable
                } else {
                    AttentionMode::Cautious
                }
            }
            // Cautious -> Freeze: require coherence to drop below threshold - hysteresis
            (AttentionMode::Cautious, AttentionMode::Freeze) => {
                if coherence < self.config.freeze_threshold - h {
                    AttentionMode::Freeze
                } else {
                    AttentionMode::Cautious
                }
            }
            // Freeze -> Cautious: require coherence to rise above threshold + hysteresis
            (AttentionMode::Freeze, AttentionMode::Cautious) => {
                if coherence > self.config.freeze_threshold + h {
                    AttentionMode::Cautious
                } else {
                    AttentionMode::Freeze
                }
            }
            // Same mode or big jump (Stable <-> Freeze): accept new mode
            _ => new_mode,
        }
    }

    /// Get current mode
    pub fn current_mode(&self) -> AttentionMode {
        self.current_mode
    }

    /// Get attention width for current mode
    pub fn get_attention_width(&self, base_width: usize) -> usize {
        match self.current_mode {
            AttentionMode::Stable => base_width,
            AttentionMode::Cautious => {
                ((base_width as f32 * self.config.cautious_width_factor) as usize).max(1)
            }
            AttentionMode::Freeze => 0, // No attention updates
        }
    }

    /// Get sparsity factor for current mode
    pub fn get_sparsity_factor(&self) -> f32 {
        match self.current_mode {
            AttentionMode::Stable => 1.0,
            AttentionMode::Cautious => self.config.cautious_sparsity_factor,
            AttentionMode::Freeze => f32::INFINITY, // Maximum sparsity
        }
    }

    /// Check if updates are allowed
    pub fn allows_updates(&self) -> bool {
        self.current_mode != AttentionMode::Freeze
    }

    /// Check if writes are allowed
    pub fn allows_writes(&self) -> bool {
        self.current_mode != AttentionMode::Freeze
    }

    /// Get mode stability (how often mode has been same recently)
    pub fn mode_stability(&self) -> f32 {
        if self.mode_history.is_empty() {
            return 1.0;
        }

        let current = self.current_mode;
        let matches = self.mode_history.iter().filter(|&&m| m == current).count();
        matches as f32 / self.mode_history.len() as f32
    }

    /// Reset to stable mode
    pub fn reset(&mut self) {
        self.current_mode = AttentionMode::Stable;
        self.mode_history.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_modes() {
        let mut policy = AttentionPolicy::new(PolicyConfig::default());

        // High coherence = stable
        assert_eq!(policy.determine_mode(0.9), AttentionMode::Stable);

        // Medium coherence = cautious
        assert_eq!(policy.determine_mode(0.5), AttentionMode::Cautious);

        // Low coherence = freeze
        assert_eq!(policy.determine_mode(0.1), AttentionMode::Freeze);
    }

    #[test]
    fn test_attention_width() {
        let mut policy = AttentionPolicy::new(PolicyConfig::default());

        policy.determine_mode(0.9);
        assert_eq!(policy.get_attention_width(100), 100);

        policy.determine_mode(0.5);
        assert_eq!(policy.get_attention_width(100), 50);

        policy.determine_mode(0.1);
        assert_eq!(policy.get_attention_width(100), 0);
    }

    #[test]
    fn test_hysteresis() {
        let mut policy = AttentionPolicy::new(PolicyConfig {
            stable_threshold: 0.7,
            freeze_threshold: 0.3,
            hysteresis: 0.1,
            ..Default::default()
        });

        // Start stable
        policy.determine_mode(0.8);
        assert_eq!(policy.current_mode(), AttentionMode::Stable);

        // Drop to 0.65 (below 0.7 but above 0.7 - 0.1 = 0.6)
        policy.determine_mode(0.65);
        // Should stay stable due to hysteresis
        assert_eq!(policy.current_mode(), AttentionMode::Stable);

        // Drop to 0.55 (below 0.6)
        policy.determine_mode(0.55);
        assert_eq!(policy.current_mode(), AttentionMode::Cautious);
    }

    #[test]
    fn test_update_permissions() {
        let mut policy = AttentionPolicy::new(PolicyConfig::default());

        policy.determine_mode(0.8);
        assert!(policy.allows_updates());
        assert!(policy.allows_writes());

        policy.determine_mode(0.1);
        assert!(!policy.allows_updates());
        assert!(!policy.allows_writes());
    }
}
