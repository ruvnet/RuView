//! ADR-096 §3.2 streaming-decode test: token-by-token `step()` against
//! a `KvCache` should match a single-shot `forward()` over the same
//! Q/K/V at the final position. This is the structural advantage
//! dense MHA can't follow — proving it stays correct under streaming
//! is what the §5 validation gate would care about most.

use wifi_densepose_temporal::{
    AetherTemporalHead, TemporalBackendKind, TemporalHeadConfig, Tensor3,
};

fn make_qkv(seq: usize, q_heads: usize, kv_heads: usize, dim: usize) -> (Tensor3, Tensor3, Tensor3) {
    let mut q = Tensor3::zeros(seq, q_heads, dim);
    let mut k = Tensor3::zeros(seq, kv_heads, dim);
    let mut v = Tensor3::zeros(seq, kv_heads, dim);
    for s in 0..seq {
        for h in 0..q_heads {
            for d in 0..dim {
                let val = ((s * 31 + h * 7 + d) as f32).sin() * 0.1;
                q.set(s, h, d, val);
            }
        }
        for h in 0..kv_heads {
            for d in 0..dim {
                let val = (((s * 17 + h * 3 + d) as f32).cos()) * 0.1;
                k.set(s, h, d, val);
                v.set(s, h, d, val * 0.5);
            }
        }
    }
    (q, k, v)
}

fn slice_token(t: &Tensor3, idx: usize) -> Tensor3 {
    let (_, heads, dim) = t.shape();
    let mut out = Tensor3::zeros(1, heads, dim);
    for h in 0..heads {
        for d in 0..dim {
            out.set(0, h, d, t.get(idx, h, d));
        }
    }
    out
}

fn config_mha_small() -> TemporalHeadConfig {
    // Equal q/k heads forces the `forward` MHA branch — `decode_step`
    // upstream is wired to this branch, not the GQA branch (which has
    // its own decode path coming in upstream's roadmap).
    TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: 2,
        kv_heads: 2,
        head_dim: 16,
        window: 8,
        block_size: 4,
        causal: true,
    }
}

#[test]
fn streaming_step_matches_forward_at_last_position() {
    let cfg = config_mha_small();
    let head = AetherTemporalHead::new(&cfg).expect("construct");

    let seq = 16usize;
    let (q, k, v) = make_qkv(seq, cfg.q_heads, cfg.kv_heads, cfg.head_dim);

    // Reference: single-shot forward over the full sequence.
    let reference = head.forward(&q, &k, &v).expect("forward");

    // Streaming: append k/v one token at a time, decode the new q.
    let mut cache = head.make_cache(seq).expect("cache");
    let mut last_out: Option<Tensor3> = None;
    for t in 0..seq {
        let qt = slice_token(&q, t);
        let kt = slice_token(&k, t);
        let vt = slice_token(&v, t);
        last_out = Some(head.step(&qt, &kt, &vt, &mut cache).expect("step"));
    }
    let streamed = last_out.expect("at least one step");

    // Compare the streamed last-token output to the reference's
    // last-token output. Tolerance is generous because numerical
    // accumulation differs between the two paths even at exact
    // mathematical equivalence.
    let (s_seq, s_heads, s_dim) = streamed.shape();
    assert_eq!((s_seq, s_heads, s_dim), (1, cfg.q_heads, cfg.head_dim));
    let mut max_abs_err: f32 = 0.0;
    for h in 0..cfg.q_heads {
        for d in 0..cfg.head_dim {
            let a = streamed.get(0, h, d);
            let b = reference.get(seq - 1, h, d);
            let err = (a - b).abs();
            if err > max_abs_err {
                max_abs_err = err;
            }
        }
    }
    // 1e-3 absolute is a comfortable bound for activations of this
    // magnitude (~0.1 input scale). Tighten if the kernel ever
    // promises closer match.
    assert!(
        max_abs_err < 1.0e-3,
        "streaming/forward divergence at last token exceeds 1e-3: max_abs_err = {max_abs_err}"
    );
}

#[test]
fn step_rejects_multi_token_q() {
    let cfg = config_mha_small();
    let head = AetherTemporalHead::new(&cfg).expect("construct");
    let mut cache = head.make_cache(8).expect("cache");

    // Build a 2-token Q/K/V — `step` must reject (its contract is
    // single-token decode).
    let (q, k, v) = make_qkv(2, cfg.q_heads, cfg.kv_heads, cfg.head_dim);
    let err = head.step(&q, &k, &v, &mut cache).err().expect("rejected");
    let s = format!("{err}");
    assert!(
        s.contains("single-token") || s.to_lowercase().contains("seq"),
        "expected single-token rejection, got: {s}"
    );
}

#[test]
fn make_cache_returns_kvcache_with_correct_shape() {
    // Smoke test that the convenience wrapper plumbs the right dims
    // into KvCache::new — the upstream constructor takes
    // (capacity, kv_heads, dim, block_size) and we want to make sure
    // we're not transposing any of those.
    let cfg = config_mha_small();
    let head = AetherTemporalHead::new(&cfg).expect("construct");
    let mut cache = head.make_cache(32).expect("cache");

    // Append one token shaped for kv_heads × head_dim — should not error.
    let (_, k, v) = make_qkv(1, cfg.q_heads, cfg.kv_heads, cfg.head_dim);
    let kt = slice_token(&k, 0);
    let vt = slice_token(&v, 0);
    cache.try_append(&kt, &vt).expect("append shape ok");
}
