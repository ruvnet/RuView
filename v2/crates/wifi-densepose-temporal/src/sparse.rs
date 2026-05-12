use ruvllm_sparse_attention::{
    AttentionBackend, KvCache, SparseAttentionConfig, SubquadraticSparseAttention, Tensor3,
};

use crate::{TemporalError, TemporalHeadConfig};

/// AETHER temporal head implemented with `ruvllm_sparse_attention`.
///
/// The selection rule from ADR-096 §4.4 is enforced at `forward()`
/// time: when `q_heads == kv_heads` we use `forward()` (plain MHA
/// over the sparse pattern); when they differ we use `forward_gqa()`.
/// The streaming `step()` path is staged behind a follow-up — KvCache
/// lifecycle ties to `PoseTrack` per ADR-096 §8.5 and lives on the
/// caller, not here.
pub struct SparseGqaHead {
    cfg: TemporalHeadConfig,
    attn: SubquadraticSparseAttention,
}

impl SparseGqaHead {
    pub fn new(cfg: &TemporalHeadConfig) -> Result<Self, TemporalError> {
        cfg.validate()?;

        let attn_cfg = SparseAttentionConfig {
            window: cfg.window,
            block_size: cfg.block_size,
            global_tokens: alloc_first_token(),
            causal: cfg.causal,
            use_log_stride: true,
            use_landmarks: true,
            sort_candidates: false,
        };

        let attn = SubquadraticSparseAttention::new(attn_cfg)?;
        Ok(Self {
            cfg: cfg.clone(),
            attn,
        })
    }

    pub fn cfg(&self) -> &TemporalHeadConfig {
        &self.cfg
    }

    pub fn forward(
        &self,
        q: &Tensor3,
        k: &Tensor3,
        v: &Tensor3,
    ) -> Result<Tensor3, TemporalError> {
        // ADR-096 §4.4: dispatch by GQA shape.
        if self.cfg.q_heads == self.cfg.kv_heads {
            // Pure MHA — sparse `forward` is the right path.
            Ok(self.attn.forward(q, k, v)?)
        } else {
            // GQA / MQA — kv_heads < q_heads, group share factor = q/kv.
            Ok(self.attn.forward_gqa(q, k, v)?)
        }
    }

    /// Streaming decode for re-ID and online classification (ADR-096 §3.2).
    ///
    /// Given one new token's q/k/v, append (k, v) to `cache` and return
    /// the attention output for that one position against the full
    /// accumulated history. Cost is O(log T) per step against a cache
    /// of capacity T — the structural advantage over dense MHA's O(N²)
    /// recompute that ADR-096 specifically calls out as the
    /// dense-MHA-cannot-follow path.
    ///
    /// Cache lifetime is owned by the caller. Per ADR-096 §8.5 the
    /// natural place is one cache per `PoseTrack` (re-ID) or one cache
    /// per active session (online classification). When the track is
    /// dropped, drop the cache.
    pub fn step(
        &self,
        q_new: &Tensor3,
        k_new: &Tensor3,
        v_new: &Tensor3,
        cache: &mut KvCache,
    ) -> Result<Tensor3, TemporalError> {
        if q_new.seq != 1 || k_new.seq != 1 || v_new.seq != 1 {
            return Err(TemporalError::InvalidConfig(
                "step() requires single-token q/k/v (seq == 1 each)",
            ));
        }
        // Append must succeed before decode_step sees the cache; if
        // the cache fills, the caller is responsible for eviction or
        // resetting per ADR-096 §3.2 (H2O heavy-hitter eviction is
        // available upstream but kept opt-in).
        cache.try_append(k_new, v_new)?;
        Ok(self.attn.decode_step(q_new, cache)?)
    }

    /// Construct a KvCache sized for this head's shape. Convenience
    /// so callers don't need to import the upstream crate directly.
    pub fn make_cache(&self, capacity: usize) -> KvCache {
        KvCache::new(
            capacity,
            self.cfg.kv_heads,
            self.cfg.head_dim,
            self.cfg.block_size,
        )
    }
}

/// Always treat token 0 as a global anchor — AETHER's contrastive
/// recipe (ADR-024) gives the first token a special role as the
/// "session start" reference embedding, and global tokens in the
/// sparse pattern preserve full visibility for that one position.
fn alloc_first_token() -> Vec<usize> {
    vec![0]
}
