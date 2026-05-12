//! Smoke tests for the AETHER sparse-GQA temporal head (ADR-096 §5 gate is
//! a separate accuracy benchmark; this file just proves the wiring works).

use wifi_densepose_temporal::{
    AetherTemporalHead, TemporalBackendKind, TemporalHeadConfig, TemporalError, Tensor3,
};

fn make_qkv(seq: usize, q_heads: usize, kv_heads: usize, dim: usize) -> (Tensor3, Tensor3, Tensor3) {
    // Deterministic synthetic CSI-like activations so the test is
    // reproducible across machines without bringing in `rand`.
    let mut q = Tensor3::zeros(seq, q_heads, dim);
    for s in 0..seq {
        for h in 0..q_heads {
            for d in 0..dim {
                let v = ((s * 31 + h * 7 + d) as f32).sin() * 0.1;
                q.set(s, h, d, v);
            }
        }
    }
    let mut k = Tensor3::zeros(seq, kv_heads, dim);
    let mut v = Tensor3::zeros(seq, kv_heads, dim);
    for s in 0..seq {
        for h in 0..kv_heads {
            for d in 0..dim {
                let kv = (((s * 17 + h * 3 + d) as f32).cos()) * 0.1;
                k.set(s, h, d, kv);
                v.set(s, h, d, kv * 0.5);
            }
        }
    }
    (q, k, v)
}

#[test]
fn sparse_gqa_forward_runs_at_aether_default() {
    let cfg = TemporalHeadConfig::default_aether();
    let head = AetherTemporalHead::new(&cfg).expect("construct");

    let (q, k, vt) = make_qkv(64, cfg.q_heads, cfg.kv_heads, cfg.head_dim);
    let out = head.forward(&q, &k, &vt).expect("forward");
    let (oseq, oh, od) = out.shape();
    assert_eq!(oseq, 64);
    assert_eq!(oh, cfg.q_heads);
    assert_eq!(od, cfg.head_dim);
}

#[test]
fn sparse_mha_path_runs_when_qkv_heads_match() {
    // q_heads == kv_heads forces the `forward` (non-GQA) branch.
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: 2,
        kv_heads: 2,
        head_dim: 16,
        window: 8,
        block_size: 4,
        causal: true,
    };
    let head = AetherTemporalHead::new(&cfg).expect("construct");
    let (q, k, vt) = make_qkv(32, 2, 2, 16);
    let out = head.forward(&q, &k, &vt).expect("forward");
    assert_eq!(out.shape(), (32, 2, 16));
}

#[test]
fn dense_backend_forward_runs_with_matching_shape() {
    // Dense_attention upstream requires q_heads == kv_heads (no GQA).
    // Use MHA shape; n_classes/n_layers don't matter for forward-only.
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::Dense,
        q_heads: 4,
        kv_heads: 4,
        head_dim: 16,
        window: 8,
        block_size: 4,
        causal: true,
    };
    let head = AetherTemporalHead::new(&cfg).expect("construct dense");
    let (q, k, v) = make_qkv(32, 4, 4, 16);
    let out = head.forward(&q, &k, &v).expect("dense forward");
    assert_eq!(out.shape(), (32, 4, 16));
}

#[test]
fn dense_backend_step_returns_streaming_error() {
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::Dense,
        q_heads: 4,
        kv_heads: 4,
        head_dim: 16,
        window: 8,
        block_size: 4,
        causal: true,
    };
    let head = AetherTemporalHead::new(&cfg).expect("construct dense");
    let cache_err = head.make_cache(32).err().expect("no cache for dense");
    matches!(cache_err, TemporalError::BackendDoesNotSupportStreaming);
}

#[test]
fn invalid_gqa_ratio_rejected_at_construction() {
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: 5,
        kv_heads: 2, // 5 % 2 != 0
        head_dim: 16,
        window: 8,
        block_size: 4,
        causal: true,
    };
    let err = AetherTemporalHead::new(&cfg).err().expect("rejected");
    matches!(err, TemporalError::InvalidConfig(_));
}

#[test]
fn long_window_at_aether_roadmap_target() {
    // ADR-096 §3.1 roadmap target: 10 s @ 100 Hz = 1000 frames. Verify
    // the kernel actually runs at this length so the long-window claim
    // is more than aspirational.
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: 4,
        kv_heads: 1,
        head_dim: 16,
        window: 64,
        block_size: 32,
        causal: true,
    };
    let head = AetherTemporalHead::new(&cfg).expect("construct");
    let (q, k, vt) = make_qkv(1000, 4, 1, 16);
    let out = head.forward(&q, &k, &vt).expect("forward at N=1000");
    assert_eq!(out.shape(), (1000, 4, 16));
}
