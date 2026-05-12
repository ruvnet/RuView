use crate::TemporalError;

/// Backend choice per ADR-096 §4.4.
///
/// * `Dense` — back-compat path against `ruvector-attention`. Reserved;
///   not yet implemented in this crate (returns a typed error so callers
///   can fail loudly during config validation rather than at forward()).
/// * `SparseGqa` — `ruvllm_sparse_attention` `forward_gqa` for prefill,
///   `decode_step` against `KvCache` for streaming inference.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemporalBackendKind {
    Dense,
    SparseGqa,
}

#[derive(Clone, Debug)]
pub struct TemporalHeadConfig {
    pub backend: TemporalBackendKind,

    /// Number of query heads. For pure MHA, equals `kv_heads`.
    pub q_heads: usize,
    /// Number of key/value heads. Must divide `q_heads`. GQA group size
    /// is `q_heads / kv_heads`.
    pub kv_heads: usize,
    /// Per-head feature dimension.
    pub head_dim: usize,

    /// Local attention window radius (sparse pattern primitive #1, ADR-096 §3).
    pub window: usize,
    /// Landmark block size (sparse pattern primitive #3).
    pub block_size: usize,
    /// Whether the attention is causal. AETHER temporal aggregation is
    /// causal (cannot peek at future CSI frames during streaming re-ID).
    pub causal: bool,
}

impl TemporalHeadConfig {
    /// Default config sized for the AETHER training default
    /// (`window_frames = 100`) but with the sparse machinery wired up
    /// so the long-window roadmap (10 s / 1000 frames) only requires
    /// changing `window` at the call site, not re-architecting.
    pub fn default_aether() -> Self {
        Self {
            backend: TemporalBackendKind::SparseGqa,
            q_heads: 4,
            kv_heads: 1, // MQA — collapses to one shared K/V across query heads
            head_dim: 32,
            window: 32,
            block_size: 16,
            causal: true,
        }
    }

    pub fn validate(&self) -> Result<(), TemporalError> {
        if self.q_heads == 0 || self.kv_heads == 0 || self.head_dim == 0 {
            return Err(TemporalError::InvalidConfig(
                "q_heads, kv_heads, head_dim must all be > 0",
            ));
        }
        if self.q_heads % self.kv_heads != 0 {
            return Err(TemporalError::InvalidConfig(
                "q_heads must be divisible by kv_heads (GQA constraint)",
            ));
        }
        if self.block_size == 0 {
            return Err(TemporalError::InvalidConfig("block_size must be > 0"));
        }
        Ok(())
    }
}
