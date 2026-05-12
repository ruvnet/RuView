//! ADR-096 AETHER temporal head — `tch::nn` bridge.
//!
//! Additive integration: wires `wifi-densepose-temporal` (sparse-GQA
//! attention + streaming KvCache) into the train crate's tch graph.
//! Does NOT modify the existing `WiFiDensePoseModel` forward in
//! `model.rs` — that path stays bit-equivalent for back-compat. Use
//! this aggregator alongside the existing model when you want a
//! temporal-axis pooling on top of per-frame backbone features.
//!
//! Bridge boundary:
//!   tch::Tensor [T, in_dim]  →  Tensor3 (seq=T, heads, dim)  →  attention
//!                            ←  Tensor3                       ←  forward()
//!   tch::Tensor [in_dim] (pooled embedding)
//!
//! Memory pattern: tch.copy_data → Vec<f32> → Tensor3::from_vec on the
//! way in; Tensor3 raw → Tensor::of_slice on the way out. Two host
//! copies per call. For training-rate forwards (~100 calls/sec at
//! batch 16) this is negligible vs the actual attention work; for
//! inference-rate streaming it'd be the bottleneck and a
//! zero-copy path is the natural Phase 2.
//!
//! Only the B=1 prefill path is implemented in this commit. Multi-batch
//! and the streaming `step()` bridge land when the §5 validation gate
//! turns green and we need to take the perf hit seriously.
//!
//! Feature-gated: `aether-sparse-temporal` (also requires `tch-backend`).

use tch::{
    nn::{self, Module},
    Device, Kind, Tensor,
};

use wifi_densepose_temporal::{
    AetherTemporalHead, TemporalBackendKind, TemporalError, TemporalHeadConfig, Tensor3,
};

/// Aggregator: tch-side projections + the pure-Rust sparse attention
/// kernel + a tch-side output projection. The projection layers are
/// `nn::Linear` so they participate in the tch VarStore the same way
/// the rest of the model does — gradients, save/load, etc.
pub struct AetherTemporalAggregator {
    cfg: TemporalHeadConfig,
    in_dim: i64,

    // tch-side learnable projections.
    q_proj: nn::Linear,
    k_proj: nn::Linear,
    v_proj: nn::Linear,
    o_proj: nn::Linear,

    // The kernel itself is configuration-only; no weights live inside
    // because the sparse attention forward is purely a function of
    // q/k/v + the SparseAttentionConfig.
    head: AetherTemporalHead,
}

impl AetherTemporalAggregator {
    /// Build the aggregator. `vs` is the tch namespace under which
    /// the four projection layers register. `in_dim` is the input
    /// feature dimension per frame (e.g. backbone output dim).
    pub fn new(vs: nn::Path, in_dim: i64, cfg: TemporalHeadConfig) -> Result<Self, TemporalError> {
        cfg.validate()?;
        // Backend has to be Sparse — Dense projections would still
        // work, but the whole point of this integration is the new
        // sparse-GQA path. If a caller wants dense, they can keep
        // using `apply_antenna_attention` / `apply_spatial_attention`
        // from model.rs.
        if !matches!(cfg.backend, TemporalBackendKind::SparseGqa) {
            return Err(TemporalError::InvalidConfig(
                "aggregator only wires SparseGqa; use existing model.rs paths for dense",
            ));
        }

        let total_q = (cfg.q_heads * cfg.head_dim) as i64;
        let total_kv = (cfg.kv_heads * cfg.head_dim) as i64;

        let q_proj = nn::linear(&vs / "q_proj", in_dim, total_q, Default::default());
        let k_proj = nn::linear(&vs / "k_proj", in_dim, total_kv, Default::default());
        let v_proj = nn::linear(&vs / "v_proj", in_dim, total_kv, Default::default());
        let o_proj = nn::linear(&vs / "o_proj", total_q, in_dim, Default::default());

        let head = AetherTemporalHead::new(&cfg)?;

        Ok(Self {
            cfg,
            in_dim,
            q_proj,
            k_proj,
            v_proj,
            o_proj,
            head,
        })
    }

    /// Forward over a single sequence of frames. Input shape:
    /// `[T, in_dim]` (NB: B=1 only this version — see file header).
    /// Returns the per-token attention output passed through the
    /// output projection: `[T, in_dim]`.
    ///
    /// Pooling (mean over T, last-token, attention-pool, etc.) is the
    /// caller's job — different downstream consumers want different
    /// pools and we don't want to bake one in.
    pub fn forward(&self, frames: &Tensor) -> Result<Tensor, TemporalError> {
        let dims = frames.size();
        if dims.len() != 2 || dims[1] != self.in_dim {
            return Err(TemporalError::InvalidConfig(
                "aggregator.forward expects [T, in_dim] tch::Tensor",
            ));
        }
        let t = dims[0] as usize;
        let device = frames.device();

        // ── Project to Q/K/V on the tch side ──────────────────────
        let q_th = self.q_proj.forward(frames); // [T, q_heads*head_dim]
        let k_th = self.k_proj.forward(frames); // [T, kv_heads*head_dim]
        let v_th = self.v_proj.forward(frames); // [T, kv_heads*head_dim]

        // ── Bridge to Tensor3 (CPU, f32) ──────────────────────────
        let q_t3 = tch_to_tensor3(&q_th, t, self.cfg.q_heads, self.cfg.head_dim)?;
        let k_t3 = tch_to_tensor3(&k_th, t, self.cfg.kv_heads, self.cfg.head_dim)?;
        let v_t3 = tch_to_tensor3(&v_th, t, self.cfg.kv_heads, self.cfg.head_dim)?;

        // ── Sparse attention forward (pure-Rust path) ────────────
        let attn_out = self.head.forward(&q_t3, &k_t3, &v_t3)?;

        // ── Bridge back to tch ───────────────────────────────────
        let attn_th = tensor3_to_tch(&attn_out, device);
        // attn_th shape is [T, q_heads*head_dim].

        // ── Output projection on tch side ────────────────────────
        let out = self.o_proj.forward(&attn_th); // [T, in_dim]
        Ok(out)
    }
}

/// Reshape a `[T, heads*head_dim]` tch::Tensor on (any device, any
/// kind) into a CPU `Tensor3(seq=T, heads, head_dim)`. Forces f32 +
/// CPU + contiguous memory; copies once.
fn tch_to_tensor3(
    th: &Tensor,
    seq: usize,
    heads: usize,
    head_dim: usize,
) -> Result<Tensor3, TemporalError> {
    let dims = th.size();
    if dims.len() != 2 || dims[0] as usize != seq || dims[1] as usize != heads * head_dim {
        return Err(TemporalError::InvalidConfig(
            "tch_to_tensor3 shape mismatch",
        ));
    }
    let cpu = th.to_kind(Kind::Float).to_device(Device::Cpu).contiguous();
    let total = seq * heads * head_dim;
    let mut buf = vec![0.0f32; total];
    cpu.copy_data(&mut buf, total);
    // tch row-major flatten gives [seq][heads*head_dim]. Tensor3
    // expects [seq][heads][dim] in the same row-major order, so the
    // contiguous bytes are layout-compatible — no per-element
    // transpose required.
    Tensor3::from_vec(buf, seq, heads, head_dim)
        .map_err(|e| TemporalError::InvalidConfig(Box::leak(format!("from_vec: {e}").into_boxed_str())))
}

/// Inverse of `tch_to_tensor3`: take a `Tensor3(seq, heads, dim)` and
/// produce a `[seq, heads*dim]` tch::Tensor on the requested device.
fn tensor3_to_tch(t3: &Tensor3, device: Device) -> Tensor {
    let (seq, heads, dim) = t3.shape();
    // Tensor3 stores seq×heads×dim contiguously; flatten heads/dim
    // by reading the row at each (seq, head) and concatenating.
    let mut flat = Vec::with_capacity(seq * heads * dim);
    for s in 0..seq {
        for h in 0..heads {
            flat.extend_from_slice(t3.row(s, h));
        }
    }
    Tensor::from_slice(&flat)
        .reshape([seq as i64, (heads * dim) as i64])
        .to_device(device)
}
