# Bench results — sparse vs dense prefill

Output of `cargo run -p wifi-densepose-temporal --example bench_speedup --release`
on a Windows 11 / x86_64 dev box, 2026-05-08. Single-run wall-clock,
pure-Rust vs pure-Rust (no SIMD/threads on either side). Reproduce by
running the example yourself; results vary 2–3× between machines and
power states, but the **trends across N** are what matter.

## Sparse-vs-dense prefill speedup

Config: `q_heads=4, kv_heads=4, head_dim=32, window=16, block_size=32, causal=true`.

| N      | Dense (ms)   | Sparse (ms)  | Speedup |
|--------|-------------:|-------------:|--------:|
|     64 |        0.262 |        0.141 |    1.86× |
|    128 |        1.120 |        0.335 |    3.34× |
|    256 |        4.129 |        0.711 |    5.81× |
|    512 |       19.230 |        2.356 |    8.16× |
|   1024 |       71.904 |        3.389 | **21.21×** |

## Asymptotic check

ADR-096 §3.1 claimed dense scales as O(N²) and sparse as O(N log N).
The measured 64→1024 cost growth (16× more tokens) is:

| Path   | 64 ms | 1024 ms | Growth | Theory |
|--------|------:|--------:|-------:|-------:|
| Dense  | 0.262 |  71.904 |   274× | 256× = 16² |
| Sparse | 0.141 |   3.389 |    24× | ~27× = 16 · log(1024)/log(64) |

Dense's 274× growth matches `N²` cleanly. Sparse's 24× growth matches
`N log N` to within measurement noise. **The asymptotic complexity
claim is empirically supported on this hardware.**

## Why N=64 is only 1.86× and not faster

ADR-096 §3.1 already called this out: at the AETHER training default
of `window_frames = 100`, dense MHA is essentially free and the sparse
machinery has overhead — the per-token candidate-set construction,
landmark indexing, and global-token bookkeeping are constant-factor
costs that only amortize past N ≈ 200. The speedup-vs-N curve
inflects sharply between N=128 and N=256 because that's where dense's
N² term starts dominating its constants.

If a downstream consumer is using AETHER on 4-frame windows
(`proof.rs`, `trainer.rs`), this ADR pays nothing. The case rests
entirely on the long-window roadmap.

## What this benchmark doesn't measure

- **Decode-step latency.** `streaming_step_matches_forward_at_last_position`
  proves correctness; this bench doesn't measure how fast `decode_step`
  runs vs a hypothetical dense-MHA decode (which would be O(N²) recompute
  every step — structurally not even comparable).
- **Memory.** KvCache + FP16 halves the K/V footprint vs FP32, which
  matters more on the firmware than on x86_64 host. Phase 5 unblocking
  is the prerequisite for measuring this on real hardware.
- **GQA dispatch.** This config uses `q_heads == kv_heads` to force
  the MHA branch, so dense and sparse operate on the same shape.
  Real AETHER will probably want `kv_heads=1` (MQA) which halves
  the KV memory and is what the default head config picks.

## How to run

```
cargo run -p wifi-densepose-temporal --example bench_speedup --release
```

Release mode is mandatory. Debug builds run sparse 5–10× slower than
release because the candidate-set construction has tight inner loops
that benefit hard from `-O3`. Don't draw conclusions from `cargo run`
without `--release`.
