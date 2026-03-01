//! Attention Coherence Configuration
//!
//! Configuration for attention-weighted residual computation.

use serde::{Deserialize, Serialize};

/// Configuration for attention-weighted coherence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionCoherenceConfig {
    /// State vector dimension
    pub dimension: usize,

    /// Number of neighbors for coherence graph construction
    pub k_neighbors: usize,

    /// Temperature for attention softmax
    pub temperature: f32,

    /// Base attention width
    pub base_width: usize,

    // Topology gating configuration
    /// Threshold for stable mode
    pub stable_threshold: f32,
    /// Threshold for freeze mode
    pub freeze_threshold: f32,
    /// Coherence update period (ticks)
    pub coherence_update_period: usize,

    // MoE configuration
    /// Number of MoE experts
    pub num_experts: usize,
    /// Top-k experts to use
    pub moe_top_k: usize,
    /// Expert capacity factor
    pub expert_capacity: f32,

    // Diffusion configuration
    /// Enable diffusion smoothing
    pub enable_diffusion: bool,
    /// Diffusion time parameter
    pub diffusion_time: f32,
    /// Number of diffusion steps
    pub diffusion_steps: usize,
    /// Sigma for diffusion kernel
    pub diffusion_sigma: f32,
}

impl Default for AttentionCoherenceConfig {
    fn default() -> Self {
        Self {
            dimension: 64,
            k_neighbors: 8,
            temperature: 1.0,
            base_width: 64,
            stable_threshold: 0.7,
            freeze_threshold: 0.3,
            coherence_update_period: 16,
            num_experts: 4,
            moe_top_k: 2,
            expert_capacity: 1.25,
            enable_diffusion: false,
            diffusion_time: 1.0,
            diffusion_steps: 5,
            diffusion_sigma: 1.0,
        }
    }
}

impl AttentionCoherenceConfig {
    /// Create configuration for small collections
    pub fn small() -> Self {
        Self {
            dimension: 32,
            k_neighbors: 4,
            base_width: 32,
            num_experts: 2,
            diffusion_steps: 3,
            ..Default::default()
        }
    }

    /// Create configuration for large collections
    pub fn large() -> Self {
        Self {
            dimension: 128,
            k_neighbors: 16,
            base_width: 128,
            num_experts: 8,
            moe_top_k: 3,
            diffusion_steps: 10,
            ..Default::default()
        }
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.dimension == 0 {
            return Err("dimension must be positive".to_string());
        }
        if self.temperature <= 0.0 {
            return Err("temperature must be positive".to_string());
        }
        if self.stable_threshold <= self.freeze_threshold {
            return Err("stable_threshold must be greater than freeze_threshold".to_string());
        }
        if self.num_experts == 0 {
            return Err("num_experts must be positive".to_string());
        }
        if self.moe_top_k > self.num_experts {
            return Err("moe_top_k cannot exceed num_experts".to_string());
        }
        Ok(())
    }

    /// Get width reduction factor for cautious mode
    pub fn cautious_width_factor(&self) -> f32 {
        0.5
    }

    /// Get width for given coherence score
    pub fn width_for_coherence(&self, coherence: f32) -> usize {
        if coherence >= self.stable_threshold {
            self.base_width
        } else if coherence >= self.freeze_threshold {
            ((self.base_width as f32) * self.cautious_width_factor()) as usize
        } else {
            1 // Freeze mode: single element
        }
    }
}

/// Attention mode based on coherence state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionMode {
    /// Full attention, normal updates
    Stable,
    /// Reduced width, increased sparsity
    Cautious,
    /// Retrieval only, no updates
    Freeze,
}

impl AttentionMode {
    /// Determine mode from coherence score
    pub fn from_coherence(coherence: f32, config: &AttentionCoherenceConfig) -> Self {
        if coherence >= config.stable_threshold {
            Self::Stable
        } else if coherence >= config.freeze_threshold {
            Self::Cautious
        } else {
            Self::Freeze
        }
    }

    /// Check if updates are allowed
    pub fn allows_updates(&self) -> bool {
        matches!(self, Self::Stable | Self::Cautious)
    }

    /// Get name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Cautious => "cautious",
            Self::Freeze => "freeze",
        }
    }
}

impl std::fmt::Display for AttentionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AttentionCoherenceConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_mode_from_coherence() {
        let config = AttentionCoherenceConfig::default();

        assert_eq!(
            AttentionMode::from_coherence(0.8, &config),
            AttentionMode::Stable
        );
        assert_eq!(
            AttentionMode::from_coherence(0.5, &config),
            AttentionMode::Cautious
        );
        assert_eq!(
            AttentionMode::from_coherence(0.2, &config),
            AttentionMode::Freeze
        );
    }

    #[test]
    fn test_width_for_coherence() {
        let config = AttentionCoherenceConfig {
            base_width: 64,
            stable_threshold: 0.7,
            freeze_threshold: 0.3,
            ..Default::default()
        };

        assert_eq!(config.width_for_coherence(0.8), 64);
        assert_eq!(config.width_for_coherence(0.5), 32);
        assert_eq!(config.width_for_coherence(0.2), 1);
    }

    #[test]
    fn test_invalid_config() {
        let config = AttentionCoherenceConfig {
            stable_threshold: 0.3,
            freeze_threshold: 0.7, // Invalid: freeze > stable
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }
}
