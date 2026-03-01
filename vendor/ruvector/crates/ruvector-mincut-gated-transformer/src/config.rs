//! Configuration types for the mincut gated transformer.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};

/// Transformer model configuration.
///
/// All shapes are fixed at model load. Degraded tiers only reduce effective
/// sequence length and window size, not physical allocations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformerConfig {
    /// Maximum sequence length (S_max)
    pub seq_len_max: u16,

    /// Hidden dimension (D)
    pub hidden: u16,

    /// Number of attention heads (H)
    pub heads: u16,

    /// Number of transformer layers (L)
    pub layers: u16,

    /// Normal attention window size (W)
    pub window_normal: u16,

    /// Degraded attention window size
    pub window_degraded: u16,

    /// FFN intermediate dimension multiplier
    pub ffn_mult: u16,

    /// Output logits dimension (task-defined)
    pub logits: u16,

    /// Number of layers to run in degraded mode
    pub layers_degraded: u16,

    /// Sequence length in degraded mode
    pub seq_len_degraded: u16,

    /// Sequence length in safe mode
    pub seq_len_safe: u16,

    /// Enable KV cache
    pub enable_kv_cache: bool,

    /// Enable external writes (memory persistence, tool execution)
    pub enable_external_writes: bool,
}

impl TransformerConfig {
    /// Create baseline CPU configuration.
    ///
    /// - Sequence length: 64
    /// - Hidden size: 256
    /// - Heads: 4
    /// - Head dim: 64
    /// - Layers: 4
    /// - FFN multiplier: 4
    /// - Attention window: 16
    pub fn baseline() -> Self {
        Self {
            seq_len_max: 64,
            hidden: 256,
            heads: 4,
            layers: 4,
            window_normal: 16,
            window_degraded: 8,
            ffn_mult: 4,
            logits: 1024, // Default output dimension
            layers_degraded: 2,
            seq_len_degraded: 32,
            seq_len_safe: 8,
            enable_kv_cache: true,
            enable_external_writes: true,
        }
    }

    /// Create micro configuration for WASM and edge gateways.
    ///
    /// - Sequence length: 32
    /// - Hidden size: 128
    /// - Heads: 4
    /// - Head dim: 32
    /// - Layers: 2
    /// - FFN multiplier: 4
    /// - Attention window: 8
    pub fn micro() -> Self {
        Self {
            seq_len_max: 32,
            hidden: 128,
            heads: 4,
            layers: 2,
            window_normal: 8,
            window_degraded: 4,
            ffn_mult: 4,
            logits: 256,
            layers_degraded: 1,
            seq_len_degraded: 16,
            seq_len_safe: 4,
            enable_kv_cache: true,
            enable_external_writes: true,
        }
    }

    /// Head dimension (hidden / heads)
    #[inline]
    pub fn head_dim(&self) -> u16 {
        self.hidden / self.heads
    }

    /// FFN intermediate dimension (hidden * ffn_mult)
    #[inline]
    pub fn ffn_intermediate(&self) -> u32 {
        (self.hidden as u32) * (self.ffn_mult as u32)
    }

    /// Total KV cache size in bytes (for i8 storage)
    #[inline]
    pub fn kv_cache_bytes(&self) -> usize {
        // K cache: L * S_max * H * Dh
        // V cache: L * S_max * H * Dh
        let per_layer = (self.seq_len_max as usize) * (self.hidden as usize);
        2 * (self.layers as usize) * per_layer
    }

    /// Total buffer size needed for runtime state
    pub fn total_buffer_bytes(&self) -> usize {
        let s = self.seq_len_max as usize;
        let d = self.hidden as usize;
        let h = self.heads as usize;
        let w = self.window_normal as usize;
        let ffn_int = self.ffn_intermediate() as usize;

        // QKV per layer: 3 * D
        let qkv = 3 * d;

        // Attention scores: H * W (per position, max over all positions)
        let attn_scores = h * w;

        // FFN intermediate
        let ffn_buf = ffn_int;

        // Residual
        let residual = s * d;

        // Norm temp
        let norm_temp = d;

        // KV cache
        let kv_cache = self.kv_cache_bytes();

        // Logits scratch
        let logits_scratch = self.logits as usize * 4; // i32

        qkv + attn_scores + ffn_buf + residual + norm_temp + kv_cache + logits_scratch
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.hidden == 0 {
            return Err(Error::BadConfig("hidden dimension must be positive"));
        }

        if self.heads == 0 {
            return Err(Error::BadConfig("head count must be positive"));
        }

        if self.hidden % self.heads != 0 {
            return Err(Error::BadConfig("hidden must be divisible by heads"));
        }

        if self.layers == 0 {
            return Err(Error::BadConfig("layer count must be positive"));
        }

        if self.seq_len_max == 0 {
            return Err(Error::BadConfig("sequence length must be positive"));
        }

        if self.window_normal == 0 {
            return Err(Error::BadConfig("window size must be positive"));
        }

        if self.window_normal > self.seq_len_max {
            return Err(Error::BadConfig("window cannot exceed sequence length"));
        }

        if self.window_degraded > self.window_normal {
            return Err(Error::BadConfig(
                "degraded window cannot exceed normal window",
            ));
        }

        if self.layers_degraded > self.layers {
            return Err(Error::BadConfig(
                "degraded layers cannot exceed total layers",
            ));
        }

        if self.seq_len_degraded > self.seq_len_max {
            return Err(Error::BadConfig("degraded seq_len cannot exceed max"));
        }

        if self.seq_len_safe > self.seq_len_degraded {
            return Err(Error::BadConfig("safe seq_len cannot exceed degraded"));
        }

        Ok(())
    }
}

impl Default for TransformerConfig {
    fn default() -> Self {
        Self::baseline()
    }
}

/// Gate policy configuration.
///
/// Controls when the gate intervenes to reduce scope, flush cache,
/// freeze writes, or quarantine updates.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GatePolicy {
    /// Minimum acceptable lambda (coherence metric)
    pub lambda_min: u32,

    /// Maximum lambda drop ratio (Q15 fixed point, 0-32767)
    pub drop_ratio_q15_max: u16,

    /// Maximum boundary edges before intervention
    pub boundary_edges_max: u16,

    /// Maximum boundary concentration (Q15, higher = more concentrated)
    pub boundary_concentration_q15_max: u16,

    /// Maximum partition count before intervention
    pub partitions_max: u16,

    /// Maximum spike rate (Q15) before throttling
    pub spike_rate_q15_max: u16,

    /// Minimum novelty (Q15) to proceed without reduction
    pub spike_novelty_q15_min: u16,

    /// Allow KV cache writes when coherence is unstable
    pub allow_kv_write_when_unstable: bool,

    /// Allow external writes when coherence is unstable
    pub allow_external_write_when_unstable: bool,
}

impl GatePolicy {
    /// Conservative policy - more aggressive intervention
    pub fn conservative() -> Self {
        Self {
            lambda_min: 50,
            drop_ratio_q15_max: 8192, // 25%
            boundary_edges_max: 10,
            boundary_concentration_q15_max: 16384, // 50%
            partitions_max: 5,
            spike_rate_q15_max: 8192,
            spike_novelty_q15_min: 4096,
            allow_kv_write_when_unstable: false,
            allow_external_write_when_unstable: false,
        }
    }

    /// Permissive policy - fewer interventions
    pub fn permissive() -> Self {
        Self {
            lambda_min: 20,
            drop_ratio_q15_max: 16384, // 50%
            boundary_edges_max: 50,
            boundary_concentration_q15_max: 24576, // 75%
            partitions_max: 20,
            spike_rate_q15_max: 24576,
            spike_novelty_q15_min: 1024,
            allow_kv_write_when_unstable: true,
            allow_external_write_when_unstable: false,
        }
    }

    /// Validate policy
    pub fn validate(&self) -> Result<()> {
        if self.drop_ratio_q15_max > 32767 {
            return Err(Error::BadConfig("drop_ratio_q15_max exceeds Q15 range"));
        }

        if self.boundary_concentration_q15_max > 32767 {
            return Err(Error::BadConfig(
                "boundary_concentration_q15_max exceeds Q15 range",
            ));
        }

        if self.spike_rate_q15_max > 32767 {
            return Err(Error::BadConfig("spike_rate_q15_max exceeds Q15 range"));
        }

        if self.spike_novelty_q15_min > 32767 {
            return Err(Error::BadConfig("spike_novelty_q15_min exceeds Q15 range"));
        }

        Ok(())
    }
}

impl Default for GatePolicy {
    fn default() -> Self {
        Self {
            lambda_min: 30,
            drop_ratio_q15_max: 12288, // ~37.5%
            boundary_edges_max: 20,
            boundary_concentration_q15_max: 20480, // ~62.5%
            partitions_max: 10,
            spike_rate_q15_max: 16384,
            spike_novelty_q15_min: 2048,
            allow_kv_write_when_unstable: true,
            allow_external_write_when_unstable: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_config() {
        let cfg = TransformerConfig::baseline();
        assert_eq!(cfg.seq_len_max, 64);
        assert_eq!(cfg.hidden, 256);
        assert_eq!(cfg.heads, 4);
        assert_eq!(cfg.head_dim(), 64);
        assert_eq!(cfg.layers, 4);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_micro_config() {
        let cfg = TransformerConfig::micro();
        assert_eq!(cfg.seq_len_max, 32);
        assert_eq!(cfg.hidden, 128);
        assert_eq!(cfg.head_dim(), 32);
        assert_eq!(cfg.layers, 2);
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_invalid_config() {
        let mut cfg = TransformerConfig::baseline();
        cfg.hidden = 100;
        cfg.heads = 3;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_policy_validation() {
        assert!(GatePolicy::default().validate().is_ok());
        assert!(GatePolicy::conservative().validate().is_ok());
        assert!(GatePolicy::permissive().validate().is_ok());

        let mut policy = GatePolicy::default();
        policy.drop_ratio_q15_max = 40000;
        assert!(policy.validate().is_err());
    }
}
