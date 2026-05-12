//! Numerical A/B test for ADR-096 §5: do Dense and SparseGqa produce
//! comparable outputs on the same input?
//!
//! Background. Sparse attention is *structurally* an approximation —
//! it skips edges that the local window + log-stride + landmark
//! pattern decided wouldn't matter. The §5 validation gate cares
//! about whether that approximation degrades downstream metrics
//! (contrastive loss, rank-1 accuracy, Spearman correlation), not
//! whether outputs are bit-equal. This file establishes the *direct*
//! output-level error envelope so the gate can be calibrated against
//! it.
//!
//! Two regimes:
//!
//! 1. **Sparse pattern is dense.** When window ≥ N AND block_size ≥ N
//!    AND every position is global, sparse and dense visit the same
//!    edge set. Output divergence then reflects only floating-point
//!    accumulation order, which is a tight bound (~1e-5 for f32 sums
//!    of ~100 terms at 0.1 magnitude).
//!
//! 2. **Sparse pattern is sparse.** Default config drops most edges
//!    at long N. Output divergence here is the *real* approximation
//!    error — and the §5 gate's tolerances apply downstream of it.

use ruvllm_sparse_attention::Tensor3;
use wifi_densepose_temporal::{
    AetherTemporalHead, TemporalBackendKind, TemporalHeadConfig,
};

fn make_qkv(seq: usize, heads: usize, dim: usize) -> (Tensor3, Tensor3, Tensor3) {
    let mut q = Tensor3::zeros(seq, heads, dim);
    let mut k = Tensor3::zeros(seq, heads, dim);
    let mut v = Tensor3::zeros(seq, heads, dim);
    for s in 0..seq {
        for h in 0..heads {
            for d in 0..dim {
                let qv = ((s * 31 + h * 7 + d) as f32).sin() * 0.1;
                let kv = (((s * 17 + h * 3 + d) as f32).cos()) * 0.1;
                q.set(s, h, d, qv);
                k.set(s, h, d, kv);
                v.set(s, h, d, kv * 0.5);
            }
        }
    }
    (q, k, v)
}

fn max_abs_err(a: &Tensor3, b: &Tensor3) -> f32 {
    let (s, h, d) = a.shape();
    assert_eq!((s, h, d), b.shape(), "shape mismatch");
    let mut max_err = 0.0f32;
    for ti in 0..s {
        for hi in 0..h {
            for di in 0..d {
                let e = (a.get(ti, hi, di) - b.get(ti, hi, di)).abs();
                if e > max_err {
                    max_err = e;
                }
            }
        }
    }
    max_err
}

fn mean_abs_err(a: &Tensor3, b: &Tensor3) -> f32 {
    let (s, h, d) = a.shape();
    let mut sum = 0.0f32;
    let mut n = 0usize;
    for ti in 0..s {
        for hi in 0..h {
            for di in 0..d {
                sum += (a.get(ti, hi, di) - b.get(ti, hi, di)).abs();
                n += 1;
            }
        }
    }
    sum / n.max(1) as f32
}

#[test]
fn dense_and_sparse_agree_when_sparse_pattern_is_dense() {
    // Saturate the sparse pattern: window ≥ N means the local-window
    // primitive includes every causal predecessor, so the attention
    // edge set is identical to dense MHA's. The remaining gap is
    // floating-point accumulation order (sparse goes
    // window-then-stride-then-landmark, dense goes naive 0..i).
    let seq = 32;
    let heads = 4;
    let dim = 16;
    let (q, k, v) = make_qkv(seq, heads, dim);

    let dense_cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::Dense,
        q_heads: heads,
        kv_heads: heads,
        head_dim: dim,
        window: seq, // saturate
        block_size: seq,
        causal: true,
    };
    let sparse_cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        ..dense_cfg.clone()
    };

    let dense = AetherTemporalHead::new(&dense_cfg).expect("dense");
    let sparse = AetherTemporalHead::new(&sparse_cfg).expect("sparse");

    let d = dense.forward(&q, &k, &v).expect("dense forward");
    let s = sparse.forward(&q, &k, &v).expect("sparse forward");

    let max_err = max_abs_err(&d, &s);
    let mean_err = mean_abs_err(&d, &s);

    // 1e-4 covers a generous f32-summation-order envelope at 0.1
    // input magnitude. If this ever blows up, either the saturation
    // assumption is wrong (window/block_size no longer covers
    // everything) or the kernel changed semantics.
    assert!(
        max_err < 1.0e-4,
        "saturated-pattern max_abs_err exceeds 1e-4: max={max_err} mean={mean_err}"
    );
}

#[test]
fn dense_and_sparse_diverge_predictably_at_long_n() {
    // The interesting case: real sparse pattern (window << N), real
    // approximation. We don't assert a specific error bound here —
    // that's what ADR-096 §5's validation gate calibrates. We only
    // check the numbers come out finite and plausible (per-position
    // outputs stay within a few × the input magnitude after
    // attention-weighted averaging — softmax can't blow them up).
    let seq = 256;
    let heads = 4;
    let dim = 16;
    let (q, k, v) = make_qkv(seq, heads, dim);

    let dense_cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::Dense,
        q_heads: heads,
        kv_heads: heads,
        head_dim: dim,
        window: seq, // dense — placeholder; ignored by Dense backend
        block_size: seq,
        causal: true,
    };
    let sparse_cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: heads,
        kv_heads: heads,
        head_dim: dim,
        window: 16,    // realistic sparse window
        block_size: 32,
        causal: true,
    };

    let dense = AetherTemporalHead::new(&dense_cfg).expect("dense");
    let sparse = AetherTemporalHead::new(&sparse_cfg).expect("sparse");

    let d = dense.forward(&q, &k, &v).expect("dense forward");
    let s = sparse.forward(&q, &k, &v).expect("sparse forward");

    let max_err = max_abs_err(&d, &s);
    let mean_err = mean_abs_err(&d, &s);

    // Sanity bounds. Inputs are scaled to 0.1, attention is a softmax
    // average so outputs stay in roughly [-0.1, 0.1]. If max_err > 1.0
    // something is structurally broken (NaN, underflow, etc).
    assert!(
        max_err.is_finite() && mean_err.is_finite(),
        "non-finite error: max={max_err} mean={mean_err}"
    );
    assert!(
        max_err < 1.0,
        "implausibly large divergence: max={max_err} mean={mean_err}"
    );

    // Print the numbers so they're visible when running `cargo test --
    // --nocapture`. These are what ADR-096 §5's gate would calibrate
    // against on real AETHER inputs.
    eprintln!(
        "dense_vs_sparse @ N={seq}, window=16, block=32: max_abs_err={max_err:.6e}, mean_abs_err={mean_err:.6e}"
    );
}
