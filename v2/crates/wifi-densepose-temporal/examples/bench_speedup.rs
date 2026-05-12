// Measure sparse-GQA prefill cost vs dense MHA at N = {64, 128, 256, 512, 1024}.
// ADR-096 §3.1 claimed 30–100× edge-evaluation reduction at long windows;
// this is the empirical check.
//
// Run with:   cargo run -p wifi-densepose-temporal --example bench_speedup --release
//
// Caveat: single-run wall-clock on one machine — not a rigorous benchmark.
// Trends across N matter more than the absolute numbers, and results vary
// 2–3× between machines / power states. The point is to confirm the
// magnitude of the speedup is what the ADR claimed, not a perf-engineering
// dashboard. For that, use criterion + a dedicated machine.

use std::time::Instant;

use ruvllm_sparse_attention::{dense_attention, AttentionBackend, SparseAttentionConfig, SubquadraticSparseAttention, Tensor3};
use wifi_densepose_temporal::{TemporalBackendKind, TemporalHeadConfig, AetherTemporalHead};

fn make_qkv(seq: usize, heads: usize, dim: usize) -> (Tensor3, Tensor3, Tensor3) {
    // Simple deterministic init — content doesn't matter for timing,
    // but we want each benchmark run to use the same numbers.
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

fn time_run<F: FnMut()>(label: &str, runs: usize, mut f: F) -> f64 {
    // 1 warmup + `runs` measurements. Wall clock; release-mode only is
    // meaningful (debug builds run sparse 5–10× slower than release).
    f();
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    let total_ms = start.elapsed().as_secs_f64() * 1000.0;
    let avg_ms = total_ms / runs as f64;
    println!("  {label:<36}  {avg_ms:>8.3} ms/run  ({runs} runs)");
    avg_ms
}

fn bench_at(seq: usize) -> (f64, f64, f64) {
    println!();
    println!("=== seq = {seq} ===");

    // MHA shape (q_heads == kv_heads) so dense_attention and the sparse
    // forward path operate on the same tensor shape — direct timing
    // comparison without GQA bookkeeping confounding the result.
    let heads = 4;
    let dim = 32;
    let (q, k, v) = make_qkv(seq, heads, dim);

    // Dense reference. dense_attention is the upstream's naive O(N²)
    // pure-Rust kernel — same scale, same shape, no SIMD acceleration —
    // a fair head-to-head against the equally-pure-Rust sparse path.
    let runs_dense = if seq <= 128 { 50 } else if seq <= 512 { 10 } else { 3 };
    let dense_ms = time_run(
        &format!("dense_attention (causal=true)"),
        runs_dense,
        || {
            let _ = dense_attention(&q, &k, &v, true).expect("dense forward");
        },
    );

    // Sparse via the AETHER head wrapper — same code path the production
    // training/inference would use, not the lower-level SubquadraticSparseAttention.
    // Window/block_size kept small so the sparse pattern actually drops
    // candidates at all benchmark lengths (otherwise at N=64 with default
    // config we'd touch the entire sequence and look the same as dense).
    let cfg = TemporalHeadConfig {
        backend: TemporalBackendKind::SparseGqa,
        q_heads: heads,
        kv_heads: heads, // MHA — match dense
        head_dim: dim,
        window: 16,
        block_size: 32,
        causal: true,
    };
    let head = AetherTemporalHead::new(&cfg).expect("construct head");
    let runs_sparse = if seq <= 128 { 50 } else if seq <= 512 { 30 } else { 10 };
    let sparse_ms = time_run(
        "AetherTemporalHead.forward (sparse)",
        runs_sparse,
        || {
            let _ = head.forward(&q, &k, &v).expect("sparse forward");
        },
    );

    // Also measure SubquadraticSparseAttention directly — bypasses our
    // wrapper, useful for confirming the wrapper isn't introducing
    // measurable overhead.
    let attn = SubquadraticSparseAttention::new(SparseAttentionConfig {
        window: 16,
        block_size: 32,
        global_tokens: vec![0],
        causal: true,
        use_log_stride: true,
        use_landmarks: true,
        sort_candidates: false,
    })
    .expect("construct attn");
    let raw_ms = time_run(
        "Subquadratic.forward (raw, no wrapper)",
        runs_sparse,
        || {
            let _ = attn.forward(&q, &k, &v).expect("raw sparse forward");
        },
    );

    let speedup = dense_ms / sparse_ms;
    println!("  -> sparse/dense speedup           {speedup:>6.2}×");

    (dense_ms, sparse_ms, speedup)
}

fn main() {
    println!("ADR-096 §3.1 empirical speedup check");
    println!("====================================");
    println!("Pure-Rust vs pure-Rust, no SIMD/threads, single-run wall-clock.");
    println!("Trends across N matter more than absolute numbers.");

    let lengths = [64, 128, 256, 512, 1024];
    let mut rows: Vec<(usize, f64, f64, f64)> = Vec::new();
    for &n in &lengths {
        let (dense_ms, sparse_ms, speedup) = bench_at(n);
        rows.push((n, dense_ms, sparse_ms, speedup));
    }

    println!();
    println!("Summary");
    println!("  N      dense (ms)   sparse (ms)   speedup");
    println!("  ----   ----------   -----------   -------");
    for (n, d, s, sp) in &rows {
        println!("  {n:<5}  {d:>10.3}   {s:>11.3}   {sp:>5.2}×");
    }
    println!();
    println!("ADR-096 §3.1 claim: ~30× edge reduction at N=8192,");
    println!("growing roughly N/log(N). At N=1024 the claim is ~5–10×;");
    println!("at N=64 the sparse machinery is overhead-bound (sparse may");
    println!("lose, see ADR-096 §3.1 'honest framing' paragraph).");
}
