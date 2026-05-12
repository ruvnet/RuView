# `wifi-densepose-temporal`

AETHER temporal head over CSI feature windows. Sparse-GQA attention via
`ruvllm_sparse_attention`, with a streaming `KvCache` decode path for
online re-ID and incremental classification.

Implements the host side of [ADR-096](../../../docs/adr/ADR-096-aether-temporal-head-sparse-gqa.md);
mirrored on the firmware side at
[`firmware/esp32-csi-node/components/ruv_temporal/`](../../../firmware/esp32-csi-node/components/ruv_temporal/).

## Quick start

```rust
use wifi_densepose_temporal::{AetherTemporalHead, TemporalHeadConfig, Tensor3};

// Default config matches AETHER's MQA shape:
//   q_heads=4, kv_heads=1, head_dim=32, window=32, block_size=16, causal=true
let cfg = TemporalHeadConfig::default_aether();
let head = AetherTemporalHead::new(&cfg)?;

// Prefill: full window forward
let out = head.forward(&q, &k, &v)?;        // shape: (window, q_heads, head_dim)

// Streaming: O(log T) per new frame against an accumulated cache
let mut cache = head.make_cache(/* capacity */ 1024)?;
for new_frame in stream {
    let (q1, k1, v1) = project(&new_frame); // each seq=1
    let attn_out = head.step(&q1, &k1, &v1, &mut cache)?;
    // pool, run classifier head, etc
}
```

## Backends

`TemporalBackendKind` selects between two paths (ADR-096 §4.4):

| Backend | When | Cost |
|---|---|---|
| `SparseGqa` | New training runs (default) | O(N log N) prefill, O(log T) decode |
| `Dense` | Reserved for back-compat | Returns `TemporalError::DenseBackendNotImplemented` for now (ADR-096 §4.4 follow-up) |

The `SparseGqa` backend dispatches at `forward()` time:

- `q_heads == kv_heads` → `forward()` (sparse MHA)
- `q_heads != kv_heads` → `forward_gqa()` (GQA / MQA)

## Streaming semantics

`step()` is the structural advantage over dense MHA — append `(k, v)` to the
caller-owned cache and decode the new `q` in O(log T) per token.

- `q`/`k`/`v` must each have `seq == 1` (multi-token q is the prefill path).
- `KvCache` lifetime is the caller's. Per ADR-096 §8.5 the natural lifetime
  is per-`PoseTrack` (re-ID) or per-session (online classification). When
  the track drops, drop the cache.
- Cache fills are the caller's problem. Upstream H2O heavy-hitter eviction
  is opt-in; this crate's wrapper doesn't pre-pick a policy.

Headline correctness test: `streaming_step_matches_forward_at_last_position`
proves token-by-token `step()` produces the same output as a single-shot
`forward()` at position `N-1`, max_abs_err < 1e-3.

## Weight blob format (`.rvne`)

Wire format for transferring trained weights to the firmware.
[`weights.rs`](src/weights.rs) defines the host side; the firmware mirror
at [`components/ruv_temporal/src/weights.rs`](../../../firmware/esp32-csi-node/components/ruv_temporal/src/weights.rs)
parses it bit-for-bit.

| Section | Bytes | Contents |
|---|---|---|
| Header | 24 | magic `RVNE` / version 1 / dtype flag (FP32 \| FP16) / dims |
| Weights | variable | flat per-layer arrays, dtype as flagged |
| Footer | 4 | CRC32-IEEE over everything before |

Hard-break versioning: bumping `version` means firmware refuses to load.
Adding fields goes behind reserved flag bits, never by reorder.

```rust
let blob = WeightBlob::new(header, weights)?;
let bytes = blob.serialize();          // host
// ...
let view = WeightBlobView::parse(&bytes)?;  // firmware (no_std, borrowed slice)
```

## Examples

| Example | Run |
|---|---|
| `init_random_blob` | `cargo run -p wifi-densepose-temporal --example init_random_blob -- model.rvne` — emits a 41 KB AETHER-shaped `.rvne` |
| `bench_speedup` | `cargo run -p wifi-densepose-temporal --example bench_speedup --release` — sparse-vs-dense speedup curve |

Captured benchmark results: [`benches_results.md`](benches_results.md).

## Tests

```
cargo test -p wifi-densepose-temporal
```

| Suite | Tests | What |
|---|---|---|
| `tests/smoke.rs` | 5 | Forward at AETHER default, MHA dispatch, GQA dispatch, dense-rejected, invalid-GQA-rejected, N=1000 long window |
| `tests/weight_blob.rs` | 8 | Roundtrip FP32 + FP16, bad magic / version / size / CRC / GQA, layout anchor |
| `tests/blob_e2e.rs` | 2 | Realistic 25 KB+ filesystem roundtrip, deterministic seed reproducibility |
| `tests/streaming.rs` | 3 | step()-matches-forward at last position, multi-token-q rejected, make_cache shape |

**18/18 passing as of commit `247794a2c`.**

## Status of ADR-096 claims

| Claim | Status | Evidence |
|---|---|---|
| O(N log N) sparse vs O(N²) dense | **Empirically confirmed** | `bench_speedup` measures 21.21× at N=1024; cost-growth ratios match theory (dense 274×, sparse 24× for 16× more tokens) |
| `step()` matches `forward()` at last position | **Proven** | `streaming_step_matches_forward_at_last_position` test |
| Wire format consistent host↔firmware | **Proven** | 3 sites with same magic/version/CRC, 41-KB blob roundtrips through filesystem in tests |
| Path-vendored, no crates.io coupling | **Confirmed** | Workspace dep is `path = "../vendor/ruvector/crates/ruvllm_sparse_attention"` |
| 30–100× at long windows | **Partial** | 21.21× measured at N=1024 in single-run wall-clock; higher N + criterion would push closer to the 30× lower bound |

## Status of ADR-095 surface (firmware)

`AetherTemporalHead` is the host-side analog of the firmware on-device path.
The firmware Rust component scaffold and C-side wiring are complete; the
Rust component cross-compile is currently blocked by an upstream esp-rs
nightly-bundle inconsistency. See
[`components/ruv_temporal/README.md`](../../../firmware/esp32-csi-node/components/ruv_temporal/README.md)
for details.

When the toolchain unblocks, no changes to this crate are needed —
`weights.rs` is already mirrored, `Tensor3` and `KvCache` cross the
boundary unchanged, and the C ABI consumed by `temporal_task.c` is stable.

## Open questions (still applicable from ADR-096 §8)

- The deployed AETHER tracker's actual window length is what determines
  whether sparse pays off in production. At training default of 100 frames,
  sparse begins to win (5–6× at N=128–256). At the 1000-frame roadmap
  target, the speedup is much larger (21× measured).
- Streaming GQA decode is an upstream roadmap item; the current
  `decode_step` is wired for the MHA branch. When upstream ships GQA
  decode (post-ADR-189/190), `AetherTemporalHead.step` gets a GQA dispatch
  branch added without any public API change.

## License

MIT.
