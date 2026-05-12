use ruvllm_sparse_attention::{dense_attention, Tensor3};

use crate::{TemporalError, TemporalHeadConfig};

/// Dense MHA backend (ADR-096 §5 A/B baseline).
///
/// Wraps upstream `dense_attention` — the naive O(N²) reference kernel.
/// Same approximation surface as classical scaled-dot-product attention,
/// no log-stride / landmarks / windowing. Exists primarily as the
/// reference path for the §5 validation gate (rank correlation,
/// contrastive-loss parity, latency baseline).
///
/// Has no streaming counterpart: dense MHA structurally cannot do
/// O(log T) decode — every new token requires recomputing the full
/// attention matrix. Callers that want streaming must use SparseGqa.
pub struct DenseHead {
    causal: bool,
    cfg: TemporalHeadConfig,
}

impl DenseHead {
    pub fn new(cfg: &TemporalHeadConfig) -> Result<Self, TemporalError> {
        cfg.validate()?;
        Ok(Self {
            causal: cfg.causal,
            cfg: cfg.clone(),
        })
    }

    pub fn cfg(&self) -> &TemporalHeadConfig {
        &self.cfg
    }

    /// Naive O(N²) prefill. Q/K/V must share the same head count
    /// (no GQA) — `dense_attention` upstream enforces it.
    pub fn forward(
        &self,
        q: &Tensor3,
        k: &Tensor3,
        v: &Tensor3,
    ) -> Result<Tensor3, TemporalError> {
        Ok(dense_attention(q, k, v, self.causal)?)
    }
}
