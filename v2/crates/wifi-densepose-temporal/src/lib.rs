// AETHER temporal head over CSI feature windows (ADR-096).
//
// Wraps `ruvllm_sparse_attention::SubquadraticSparseAttention` so AETHER
// callers in `wifi-densepose-train` and `wifi-densepose-signal` can swap
// dense MHA for sparse-GQA without touching the contrastive recipe.
//
// Status: scaffolding for ADR-096 §4.3. Sparse backend is functional;
// the dense back-compat backend is a follow-up (Phase 2 of the roadmap
// in #513). Streaming `step()` lands once the per-track KvCache lifecycle
// (ADR-096 §8.5) is finalized.

pub mod config;
pub mod dense;
pub mod error;
pub mod sparse;
pub mod weights;

pub use config::{TemporalBackendKind, TemporalHeadConfig};
pub use dense::DenseHead;
pub use error::TemporalError;
pub use sparse::SparseGqaHead;
pub use weights::{
    WeightBlob, WeightBlobHeader, WeightDtype, WEIGHT_BLOB_HEADER_LEN, WEIGHT_BLOB_MAGIC,
    WEIGHT_BLOB_VERSION,
};

// Re-export the upstream Tensor3 + KvCache so callers don't need a
// direct `ruvllm_sparse_attention` dep.
pub use ruvllm_sparse_attention::{KvCache, Tensor3};

/// Thin facade so callers can pick a backend by name.
///
/// Both backends implement `forward()` for prefill. Only `SparseGqa`
/// implements `step()` (streaming O(log T) decode against KvCache);
/// dense MHA structurally lacks a streaming counterpart and returns
/// `TemporalError::BackendDoesNotSupportStreaming` on `step()`.
pub enum AetherTemporalHead {
    SparseGqa(SparseGqaHead),
    Dense(DenseHead),
}

impl AetherTemporalHead {
    pub fn new(cfg: &TemporalHeadConfig) -> Result<Self, TemporalError> {
        match cfg.backend {
            TemporalBackendKind::SparseGqa => {
                Ok(AetherTemporalHead::SparseGqa(SparseGqaHead::new(cfg)?))
            }
            TemporalBackendKind::Dense => Ok(AetherTemporalHead::Dense(DenseHead::new(cfg)?)),
        }
    }

    /// Window-level prefill. Returns the per-token attention output as
    /// a Tensor3 of shape (window, q_heads, head_dim). Pooling to a
    /// single embedding is the caller's responsibility — different
    /// AETHER consumers use different pool ops (mean for re-ID,
    /// last-token for streaming).
    pub fn forward(
        &self,
        q: &Tensor3,
        k: &Tensor3,
        v: &Tensor3,
    ) -> Result<Tensor3, TemporalError> {
        match self {
            AetherTemporalHead::SparseGqa(h) => h.forward(q, k, v),
            AetherTemporalHead::Dense(h) => h.forward(q, k, v),
        }
    }

    /// Streaming decode (ADR-096 §3.2). Caller owns the `cache`; the
    /// natural lifetime is per-tracked-person (one cache per
    /// `PoseTrack`, dropped when the track evicts).
    ///
    /// Returns the attention output for the single new token. Caller
    /// is responsible for downstream pooling / classifier head.
    ///
    /// Dense backend returns `BackendDoesNotSupportStreaming` — no
    /// dense-MHA-with-KV-cache equivalent exists, by design.
    pub fn step(
        &self,
        q_new: &Tensor3,
        k_new: &Tensor3,
        v_new: &Tensor3,
        cache: &mut KvCache,
    ) -> Result<Tensor3, TemporalError> {
        match self {
            AetherTemporalHead::SparseGqa(h) => h.step(q_new, k_new, v_new, cache),
            AetherTemporalHead::Dense(_) => {
                Err(TemporalError::BackendDoesNotSupportStreaming)
            }
        }
    }

    /// Allocate a `KvCache` sized correctly for this head. Convenience
    /// wrapper so AETHER's `pose_tracker.rs` doesn't need to import
    /// the upstream crate.
    ///
    /// Dense backend returns `BackendDoesNotSupportStreaming` — there
    /// is no cache to size for a dense kernel.
    pub fn make_cache(&self, capacity: usize) -> Result<KvCache, TemporalError> {
        match self {
            AetherTemporalHead::SparseGqa(h) => Ok(h.make_cache(capacity)),
            AetherTemporalHead::Dense(_) => Err(TemporalError::BackendDoesNotSupportStreaming),
        }
    }
}
