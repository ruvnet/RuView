# ADR-096: AETHER Temporal Head via `ruvllm_sparse_attention::forward_gqa` + Streaming KV Cache

| Field       | Value                                                                                 |
|-------------|---------------------------------------------------------------------------------------|
| **Status**  | Proposed (2026-05-07)                                                                 |
| **Date**    | 2026-05-07                                                                            |
| **Authors** | ruvnet, claude-flow                                                                   |
| **Related** | ADR-014, ADR-016, ADR-024, ADR-095; upstream ADR-189, ADR-190, ADR-192                |
| **Branch**  | `feat/ruvllm-sparse-attention-edge`                                                   |
| **Tracking**| #513                                                                                  |

---

## 1. Context

ADR-024 ("Project AETHER") specifies a contrastive CSI embedding
model on top of the existing `CsiToPoseTransformer` backbone. It
adds a 2-layer projection head to the per-keypoint features and
trains it with InfoNCE + VICReg + (optional) cross-modal alignment.
The **temporal aggregation** that turns per-frame backbone features
into a window-level representation is described at the level of
"a transformer encoder over the CSI window" — but ADR-024 does not
pin a specific attention kernel. In the current code:

- `v2/crates/wifi-densepose-train/src/model.rs` uses
  `ruvector_attention::ScaledDotProductAttention` (line 34) and
  applies `apply_antenna_attention` over the antenna-path dimension
  and `apply_spatial_attention` over the spatial location dimension.
  Both are dense.
- The training-side temporal pooling currently runs at
  `window_frames = 100` by default (`config.rs:165`), with
  `proof.rs` and `trainer.rs` using shorter test windows of 4 and 2
  respectively.
- `v2/crates/wifi-densepose-signal/src/ruvsense/pose_tracker.rs`
  consumes a 128-dim AETHER re-ID embedding (line 22, 263) but does
  not perform the temporal aggregation itself — that happens
  upstream.

So the temporal head is a real seam in the codebase, but its
specific attention kernel is *currently dense* and *currently not a
named architectural decision*. This ADR makes that decision.

The vendored `ruvllm_sparse_attention` v0.1.1 (synced today,
released 2026-05-07) provides a different kind of temporal kernel:

- **Subquadratic O(N log N)** sparse attention (`forward`,
  `forward_flash`).
- **Grouped-Query / Multi-Query Attention** (`forward_gqa`,
  `forward_gqa_flash`) — shares K/V across query heads, the
  pattern Mistral-7B and Llama-3 use.
- **Streaming KV cache** (`KvCache`, `KvCacheF16`) with H2O
  heavy-hitter eviction, allowing token-by-token decode in
  **O(log T)** per step against an accumulated cache. See upstream
  ADR-189.
- **FastGRNN salience gate** for **near-linear O(N)** when the
  log-stride candidate set can be pruned.

These capabilities are qualitatively different from
`ruvector-attention` 2.0.4, which is what the workspace uses today
for spatial / antenna attention.

---

## 2. Decision

The AETHER temporal head will be implemented with
`ruvllm_sparse_attention::SubquadraticSparseAttention::forward_gqa`
for prefill, and `decode_step` against a `KvCache` (with the `fp16`
feature enabled) for streaming inference paths (online re-ID,
incremental embedding extraction during a tracked session).

Concretely:

1. `wifi-densepose-train` adds `ruvllm_sparse_attention` as a
   workspace dependency, **path-vendored** against
   `vendor/ruvector/crates/ruvllm_sparse_attention` so the workspace
   does not gain a crates.io publish dependency.
2. The AETHER block factory takes a feature flag
   (`temporal_head = "dense" | "sparse_gqa"`) selecting between the
   current dense MHA path and the new sparse-GQA path. The default
   for new training runs is `sparse_gqa`. Existing checkpoints
   continue to load on `dense`.
3. Signal-side consumers (the streaming embedding extraction used
   by `pose_tracker.rs` for re-ID updates) call `decode_step` rather
   than re-running prefill on every new frame — this is the
   structural win that dense MHA cannot provide.
4. We add an A/B benchmark gate (§5) before flipping the production
   default. The default *training* config can move first; the
   default *inference* config waits for the gate.

This ADR sanctions the swap. It does not perform the swap; that
lands in a follow-up implementation issue once both ADR-095 and
ADR-096 are accepted.

---

## 3. Quantitative argument

### 3.1 Edge-evaluation count

For a single attention layer over `N` frames:

| Path | Edge evaluations | At `N = 100` (today's default) | At `N = 1000` (10 s @ 100 Hz) | At `N = 8192` |
|------|------------------|--------------------------------|-------------------------------|---------------|
| Dense MHA | `N²` | 1.0 × 10⁴ | 1.0 × 10⁶ | 6.7 × 10⁷ |
| Sparse `forward` (window + log-stride + landmarks) | ~`N · (W + log N + N/B)` | 1.4 × 10⁴ | 1.4 × 10⁴ | 1.1 × 10⁶ |
| Sparse + FastGRNN | ~`N · (W + globals + K)` | constant in `N` | constant in `N` | constant in `N` |

Numbers for the sparse rows are taken from upstream's measured
table (`README.md:230-237`, "sparse-edge reduction vs causal dense
attention"): 8192 → 29.3× edge reduction, 16384 → 57.5×, 32768 →
113.2×.

**The honest framing:** at the *current* AETHER default of
`window_frames = 100`, dense MHA is essentially free and the
sparse machinery has overhead — the per-token cost in upstream's
benchmark is ~2.4 µs at `N = 256` and ~2.1 µs at `N = 128`. The
sparse path probably *loses* below `N ≈ 128`. It starts winning at
the 1 s + windows we'd realistically use for activity classification
(`N = 200` at 50 Hz, `N = 500` for breathing-quality), and pulls
ahead by 30–100× at the 10 s windows that long-context re-ID
benefits from.

### 3.2 Streaming decode

Where dense MHA structurally cannot follow is incremental decode.
Re-ID over a long-tracked person (a 5-minute session at 50 Hz =
15,000 frames) with dense MHA requires recomputing attention from
scratch every time the window slides. With `decode_step` against a
`KvCache`:

| Operation | Dense MHA | Sparse GQA + KV cache |
|-----------|-----------|-----------------------|
| Append one new frame to the embedding context | O(N²) | **O(log T)** |
| Memory growth | O(N · d) per recompute | O(T · d_kv) cached, evicted by H2O heavy-hitter |
| FP16 KV cache | n/a | available via `fp16` feature, halves memory |

This is the qualitative capability dense MHA lacks. Even at small
`N` where dense MHA is competitive on prefill, decode is structurally
different: amortised O(1) per new frame vs O(N²) recompute.

---

## 4. Approach

### 4.1 Workspace dependency

Add to `v2/Cargo.toml`:

```toml
[workspace.dependencies]
ruvllm_sparse_attention = {
    path = "../vendor/ruvector/crates/ruvllm_sparse_attention",
    default-features = false,
    features = ["fp16"]
}
```

`default-features = false` mirrors the rest of the workspace's
`--no-default-features` posture (and matches what ADR-095 does on
the firmware side, so both consumers have the same feature set).
We **do not** pull `parallel` here — rayon doesn't help with
inference-shaped batches at the sequence lengths we run, and it
breaks ADR-095's no_std build if the dependency leaks.

### 4.2 Crate placement

Two viable homes for the AETHER temporal head:

| Option | Tradeoffs |
|--------|-----------|
| **A. New `wifi-densepose-temporal` crate** | Cleanest. Unique import surface, easy to feature-gate. But: one more crate in the publishing order (CLAUDE.md crate table grows to 16). |
| **B. Add to `wifi-densepose-train`** | Co-located with the model; no new crate; simpler workspace graph. But: `wifi-densepose-train` is heavyweight (`tch`, full training stack), and signal-side consumers would have to depend on the whole training crate just to run inference. |

**Recommendation: A.** The temporal head is consumed by both
`wifi-densepose-train` (training) and `wifi-densepose-signal`
(inference, re-ID). Pulling those toward a shared third crate keeps
the dependency arrows clean. Also matches ADR-095's
`wifi-densepose-temporal` host-side training crate name —
deliberate convergence.

### 4.3 API sketch

```rust
pub struct AetherTemporalHead {
    backend: TemporalBackend,
    cache: Option<KvCache>,           // populated for streaming inference
}

pub enum TemporalBackend {
    Dense(DenseMha),                  // current ruvector-attention path
    SparseGqa(SubquadraticSparseAttention),
}

impl AetherTemporalHead {
    pub fn new(cfg: &TemporalHeadConfig) -> Self;

    /// Window-level prefill. Returns pooled [d_model] embedding.
    pub fn forward(&self, frames: &Tensor3) -> Vec<f32>;

    /// Incremental decode for streaming re-ID. Updates internal
    /// cache and returns pooled embedding given a single new frame.
    /// SparseGqa backend only.
    pub fn step(&mut self, frame: &Tensor3) -> Result<Vec<f32>, TemporalError>;
}
```

### 4.4 Selection rule

In `forward_auto`'s spirit, the head selects the path based on
`(window, n_q_heads, n_kv_heads)` of the model:

- `window ≤ 64` and dense MHA is in the checkpoint: use dense path.
- `n_q_heads != n_kv_heads`: use `forward_gqa`.
- `n_q_heads == n_kv_heads` and `window > 64`: use `forward`.
- Streaming (per-frame) inference: always `decode_step`.

---

## 5. Validation gate before flipping the inference default

We do not flip the production inference default until *all four*
of these pass on the most recent AETHER checkpoint:

1. **Contrastive loss within 1%** of the dense baseline at the same
   training budget (so the kernel substitution doesn't silently
   regress the loss surface).
2. **Re-ID rank-1 accuracy within 1 percentage point** of the dense
   baseline on the held-out test split.
3. **Spearman rank correlation ≥ 0.95** between dense-MHA and
   sparse-GQA top-50 nearest-neighbour orderings on the
   `env_fingerprint` and `person_track` HNSW indices (matches the
   ADR-024 §2.5.3 quantisation-rank-preservation criterion).
4. **Latency improvement ≥ 5×** at the deployed window length.

Any of (1)–(3) failing rolls back the default; the kernel can stay
in the codebase as opt-in, but is not what new training runs use.

---

## 6. Alternatives considered

| Option | Why rejected |
|--------|--------------|
| **Keep dense MHA, period** | Simple, but caps the practical window length. The 10 s + windows that long-context re-ID and breathing-quality scoring want are exactly where dense MHA hurts. We'd be locking in a ceiling for no reason. |
| **Use `ruvector-attention` 2.0.4 (already in workspace)** | It's what we use today for antenna and spatial attention. But it lacks GQA, lacks streaming KV cache, and its dependency story upstream is messy (`ruvector-attn-mincut` is stuck at 2.0.4 per the issue). It works, but it's not the right tool for *temporal* attention specifically. |
| **Wait for `ruvector-attention 2.x` to add GQA + KV cache** | Speculative; no published roadmap. Meanwhile `ruvllm_sparse_attention` shipped real artifacts on 2026-05-07 and is path-vendorable today. |
| **Use a non-attention temporal pooler (TCN / S4 / Mamba)** | All three are real options for time-series sensing; some research gives them a slight edge on long-horizon dependencies. But (a) we already have AETHER specified around attention in ADR-024, (b) the contrastive recipe is attention-tuned, (c) we'd be re-running the entire ADR-024 training story to swap to a different family. Switching to *sparse* attention preserves the ADR-024 mathematical apparatus exactly. |
| **`forward_gated_with_fastgrnn` immediately** | Tempting because it's the O(N) path. But the gate adds approximation error on top of the sparsity-induced approximation error. Phase the introductions: prove sparse-GQA matches dense first, then layer the gate on top in a follow-up. |

---

## 7. Consequences

### Positive

- **Long windows are no longer scary.** `window_frames = 1000` for
  10 s sessions becomes practical, not aspirational.
- **Streaming re-ID gets a structural speedup.** Per-frame decode
  cost goes from O(N²) to O(log T). Pose tracker cost is a real
  budget today; this shrinks it.
- **GQA fits the AETHER backbone better.** AETHER's per-keypoint
  cross-attention already has a query/key shape mismatch (17
  keypoint queries vs N CSI keys). GQA was designed for exactly
  this asymmetry.
- **Path-vendored, not crates.io-coupled.** No bind-time risk —
  the crate ships from the vendored copy of upstream, and the
  vendor was synced today (`e38347601`).
- **Same kernel, two consumers.** ADR-095 wants this on the MCU;
  this ADR wants it on the host. Path-vendoring once keeps the
  versions in lockstep.
- **Approximation error is bounded** by the local window +
  log-stride + landmark pattern. Upstream's measurement (`README.md`
  §FAQ) is "<1% perplexity on standard benchmarks" for the
  causal case; we measure ours via §5's gate.

### Negative

- **Adds a workspace dependency** the team has to know about.
  Mitigated by path-vendoring (no version-resolution risk).
- **Approximation error is not zero.** For high-precision re-ID
  this needs measurement. §5's gate is the safety net; if rank
  correlation drops below 0.95 we don't flip the default.
- **More moving parts in the temporal head.** Dense MHA has one
  knob (number of heads). Sparse GQA has window, log-stride,
  landmark block size, KV head count, and (later) gate top-K. We
  pay this in default-config tuning effort.
- **`KvCache` introduces session state** in a place that didn't
  have it. Code that previously called a stateless `forward(...)`
  now has to think about cache lifetime per tracked person. The
  pose tracker (`pose_tracker.rs`) already has per-track state, so
  the natural place for the cache is inside `PoseTrack`; needs a
  small lifecycle review.
- **Training and inference paths diverge slightly.** Training
  always uses `forward` (full window prefill). Inference uses
  `decode_step` for streaming. The two paths must be tested
  separately; upstream's `forward` and `decode_step` are unit-test
  parity-checked, but our wrapper has its own surface.

### Neutral

- ADR-024 is **not superseded.** The contrastive loss, the
  augmentation strategy, the projection head, the HNSW indices —
  all unchanged. This ADR makes a single architectural choice
  inside ADR-024's "temporal aggregation" black box.
- ADR-016 (RuVector training pipeline integration) is unaffected.
  The other RuVector crates (`mincut`, `attn-mincut`,
  `temporal-tensor`, `solver`, `attention`) keep their existing
  roles in `model.rs`.

---

## 8. Open questions

1. **What is the AETHER temporal head's actual current
   architecture in code?** ADR-024 specifies the projection head
   precisely (Linear → BN → ReLU → Linear → L2-norm) but the
   *temporal aggregation* before that is not pinned. The closest
   thing in `model.rs` today is `apply_antenna_attention` and
   `apply_spatial_attention`, which are over antenna and spatial
   axes, not the temporal axis. So this ADR is, in practice,
   choosing the temporal kernel for the *first time* — not
   replacing one. Worth confirming with the maintainer before the
   implementation PR uses language like "swap" rather than "add".
2. **What window length is the deployed AETHER tracker using
   today?** The training default is 100 frames (`config.rs:165`),
   but `proof.rs` uses 4 and `trainer.rs` uses 2. The realistic
   deployment number determines how much of the §3.1 quantitative
   argument is *currently* operative versus *future-state*. If the
   answer is "we run AETHER on 4-frame windows", sparse pays
   nothing today, and the case for this ADR rests entirely on the
   long-window roadmap. If 100 or more, sparse already pays.
3. **Is `FastGrnnGate` worth enabling for re-ID specifically?**
   Probably not — re-ID benefits from full-sequence visibility,
   and the gate's job is to *prune* long-range candidates. Save
   the gate for activity classification (where transient movement
   is the signal of interest, and saliency-based pruning matches
   the use case). Confirm via §5's accuracy gate when we get there.
4. **Does the cross-modal alignment loss (ADR-024 §2.2.4) need
   any change?** The cross-modal loss operates on pooled
   `z_csi` (already temporally aggregated) and pooled `z_pose`. As
   long as the temporal aggregator returns a comparable pooled
   vector, the loss is kernel-agnostic. Likely no change, but
   worth a smoke test.
5. **Where does the KV cache live for re-ID?** Per `pose_tracker.rs`,
   each `PoseTrack` already has lifecycle (create / update /
   evict). The natural place is `PoseTrack::kv_cache:
   Option<KvCache>`, populated when the track first emits an
   embedding. Eviction policy ties to `track.last_seen` — when
   the track is dropped, drop the cache. Spec-level sanity check
   only; needs a real design pass in the implementation PR.

---

## 9. Acceptance criteria

This ADR is **Accepted** once:

1. Maintainer review on #513 confirms the architecture and resolves
   §8.1 (the "first-time choice vs replacement" framing).
2. Open question §8.2 has a concrete answer (ideally a one-line
   pointer to the production training config).
3. The follow-up implementation issue is filed.

This ADR is **Implemented** once:

1. `wifi-densepose-temporal` (or equivalent) ships in the workspace
   with a default-off feature flag exposing both dense and
   sparse-GQA backends.
2. §5's four-gate validation has run on the most recent AETHER
   checkpoint and the result is published (witness-bundle
   compatible per ADR-028 if the run is reproducible).
3. The default for new training runs is `sparse_gqa`, with `dense`
   still selectable for back-compat.

---

## 10. Related

ADR-014 (signal SOTA), ADR-016 (RuVector training pipeline
integration), ADR-024 (AETHER contrastive CSI embedding — this
ADR fills in its temporal-aggregation black box), ADR-095
(on-ESP32-S3 temporal modeling — same crate, different consumer),
upstream ADR-189 (KV cache incremental decode — the basis for
streaming re-ID), upstream ADR-190 (GQA / MQA — what AETHER's 17
keypoint queries × N CSI keys asymmetry naturally maps onto),
upstream ADR-192 (no_std + alloc support — the structural change
that means the *same* kernel runs both on the host here and on
the MCU under ADR-095).
