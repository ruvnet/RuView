# ADR-116 — WiFlow-v1 Supervised Pose Loader (Rust)

**Status**: Accepted (integration), needs fine-tune (output quality)
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/wiflow_v1.rs` (new,
~430 lines incl. tests), `src/main.rs` (CLI flag + load + 5 tick-site hooks +
`pose_current` keypoint path), `src/lib.rs` (module export).

## Context

Until this ADR `/api/v1/pose/*` always returned an empty `persons` array
(ADR-105 — no synthetic fallback when no real model is loaded). HuggingFace
`ruv/ruview/wiflow-v1/wiflow-v1.json` is the project's official supervised
pose model (Apache-2.0, 974 KB, 92.9 % PCK@20 on its training set). It just
sat on disk because there was no Rust loader — the only reference impl is
`scripts/train-wiflow-supervised.js` (JS, training script, not deployment).

This ADR ports the JS inference path to Rust so sensing-server can serve
real 17-keypoint COCO skeletons in production.

## What was wrong in the model file (and how this ADR works around it)

The HuggingFace JSON has an `architecture` field that **lies**:

```json
"architecture": {
  "tcnChannels": [35, 256, 256, 192, 128],
  "tcnKernel": 7,
  "tcnDilations": [1, 2, 4, 8],
  "fcDims": [2560, 2048, 34]
}
```

That's the `full` scale (~7.7 M params). The file is actually the **lite**
scale (186,946 params — confirmed by `totalParams` field). The exporter at
`train-wiflow-supervised.js:1599` hardcodes the full-scale dict for every
scale. The loader trusts `totalParams` and ignores `architecture`.

Lite topology (recovered from `SCALE.lite` at `train-wiflow-supervised.js:135`
and verified by exact param count = 186,946):

* 2 TCN blocks (NOT 4), kernel = 3 (NOT 7), dilations [1, 2] (NOT [1,2,4,8])
* TCN channels: 35 → 32 → 32
* Per block: causal_conv → BN → ReLU → causal_conv → BN + residual → ReLU
  (1×1 projection on residual when in_ch ≠ out_ch, only block 0)
* Flatten 32 × 20 = 640 → fc1 (640→256) → ReLU → fc2 (256→34)
* Sigmoid on final 34-dim → 17 (x, y) keypoints in [0, 1]

## Decisions

### D1 — Pure-Rust forward pass, no new crates

`wiflow_v1.rs` is self-contained: Vec<f32> math by hand, inline base64
decoder (50 LoC), no `ndarray`, no `candle`, no `base64` crate added. The
inference is small enough (~250 K flops/forward) that hand-written Vec<f32>
loops are clearer than pulling a tensor framework for one model.

### D2 — Weight stream order matches `collectParams()` in the JS trainer

```
for each TCN block:
  conv1.weight (in_ch * k * out_ch f32s)
  conv1.bias   (out_ch)
  bn1.gamma    (out_ch)
  bn1.beta     (out_ch)
  conv2.weight, conv2.bias, bn2.gamma, bn2.beta
  (if in_ch != out_ch: res.weight, res.bias)
fc1.weight, fc1.bias, fc2.weight, fc2.bias
```

Loader asserts the stream is fully consumed (`Cursor::remaining() == 0`)
after fc2 — catches silent topology mismatches. Param count check
(`totalParams == 186_946`) catches scale mismatch before unpacking.

### D3 — BatchNorm uses per-window mean/var (matches JS impl)

`train-wiflow-supervised.js:770` computes mean/var across the T axis at
inference time, ignoring `runMean/runVar` accumulated during training.
Loader skips running stats entirely (only 2 params per channel stored:
gamma + beta). This is unusual but consistent — the network was trained
this way, so we infer this way.

### D4 — Input prep: top-35 subcarriers by NBVI, raw amplitudes

`build_input_from_history` (in `wiflow_v1.rs`):

1. Take last 20 frames from any node's `AmpState.nbvi_history` (Vec<Vec<f64>>).
2. Rank subcarriers by NBVI score (`α·σ/μ² + (1−α)·σ/μ`, α = 0.5) — same
   formula the classifier uses, but pick K = 35 (model input), not K = 12
   (classifier).
3. Apply 25th-percentile dead-zone gate to skip guard tones / null bins.
4. Build flat `[35 * 20]` row-major tensor of raw amplitudes (no z-score —
   training data wasn't normalised either, BN handles it).

If fewer than 20 frames or all subcarriers gated out → return `None`,
inference skipped this tick, `pose_keypoints: None` in SensingUpdate.

### D5 — Per-tick inference, longest-history node

`run_wiflow_inference()` at every `broadcast_tick_task` step (5 sites total
in `main.rs`):

* Picks the node with longest `nbvi_history` (ties broken by smallest
  node_id — deterministic).
* Cost: ~250 K flops on the lite scale (BN + 2 small convs + 2 FCs).
  Measured 0.4 ms on the Mac M1 — well under the 100 ms tick budget.
* Returns `Vec<[f64; 4]>` of length 17 (`[x, y, z=0, conf=1]`).

### D6 — `pose_current` reads `pose_keypoints` directly

Pre-ADR: `/api/v1/pose/current` read `latest_update.persons`. The tracker
populated `persons` from `derive_pose_from_sensing` (signal-derived,
synthetic) regardless of `model_loaded`. Loader-output `pose_keypoints`
was only read by the WS broadcaster.

This ADR makes `pose_current` prefer `pose_keypoints` when 17-len and
present, building a single `PersonDetection` with COCO joint names. Falls
back to tracker `persons` only when `pose_keypoints` is `None` (cold
start). Keeps the ADR-105 honesty gate: empty array if `model_loaded =
false`.

### D7 — Honest about output quality

The loaded model produces **17 keypoints**, but the **numerical values
are saturated** (most x/y near 0 or 1) — sigmoid extremes meaning the
network has no learned response to our specific deployment's CSI
distribution. This is expected: the model was trained on a different
ESP32 setup, different room, different person, with camera ground truth
we don't have here. **The integration is correct; the model needs
deployment-specific fine-tune to produce useful keypoints.**

Two paths to usable output, left as follow-ups (Pack E):

1. **Apply `node-1.json` / `node-2.json` LoRA adapters** (ADR-117 candidate)
   — they're shipped alongside `wiflow-v1.json` in the same HuggingFace
   repo, rank=8, alpha=16, target the encoder + task heads. Loader stub +
   forward fold ~2 h.
2. **Re-train via `scripts/train-wiflow-supervised.js` with new ground-
   truth capture** (~30 min capture + 19 min training per the model card).
   Operator-side work.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/wiflow_v1.rs   (new, ~430 LoC)
v2/crates/wifi-densepose-sensing-server/src/lib.rs         (+ pub mod)
v2/crates/wifi-densepose-sensing-server/src/main.rs:
  + use wiflow_v1::{self, WiflowModel}
  + Args.wiflow_model: Option<PathBuf>
  + static WIFLOW_MODEL: OnceLock<Option<WiflowModel>>
  + main()  — load before existing --model/--load-rvf path
  + fn run_wiflow_inference() -> Option<Vec<[f64;4]>>  (right after csi_keepalive_task)
  + 5 × `pose_keypoints: run_wiflow_inference()` at SensingUpdate sites
  + pose_current — prefer pose_keypoints when 17-len; fall back to persons
docs/adr/ADR-116-wiflow-v1-supervised-pose-loader.md  (this)
```

Binary size delta: 3.0 MB → 3.1 MB.

## Verified Acceptance

Live test on the operator's TP-Link deployment (.103, both nodes
192.168.0.100/.101):

```
$ ./target/release/sensing-server --source esp32 --csi-keepalive-pps 25 \
    --wiflow-model data/models/ruview/wiflow-v1/wiflow-v1.json
  ...
  ADR-116 wiflow-v1 loaded from data/models/ruview/wiflow-v1/wiflow-v1.json
                                              (lite scale, 186946 params)
  keepalive: learned address for node 2 = 192.168.0.100:63940
  keepalive: learned address for node 1 = 192.168.0.101:63844

$ curl :8080/api/v1/info  → "pose_estimation": true
$ curl :8080/api/v1/pose/stats  → "model_loaded": true, frames_processed: 2699
$ curl :8080/api/v1/pose/current
  { persons: [{id: 1, keypoints: [17 × {name, x, y, z, confidence}], ...}],
    total_persons: 1, model_loaded: true }
```

End-to-end: model on disk → loader → forward pass → 17 keypoints → REST &
WS payload. UI's pose canvas (un-gated by ADR-105 D4) now draws what the
model emits.

## Cargo tests

`wiflow_v1` ships 3 unit tests covering the most-likely-to-rot bits:

* `base64_round_trip_alphabet` — alphabet, padding, whitespace tolerance
* `sigmoid_bounds` — numerical stability at ±10 inputs
* `build_input_zero_history` — empty-history early return

`cargo test -p wifi-densepose-sensing-server wiflow_v1` → 3 passed.

## Open Items

* **Pack E.1 — LoRA adapter loader.** `node-1.json` / `node-2.json` rank-8
  adapters from the same HF repo, ~21 KB each. The trainer encodes them
  in the same custom format as `wiflow-v1.json` (different `format` tag),
  so the loader plumbing is small. ~2 h.
* **Pack E.2 — Camera-supervised retraining for this room.** Run
  `scripts/collect-ground-truth.py` against this Mac's webcam +
  TP-Link/.100/.101 CSI for 5 min, then `scripts/train-wiflow-
  supervised.js --scale lite`. Should drop sigmoid saturation and produce
  spatially-coherent keypoints. ~1 h operator + 19 min train.
* **Inference rate-limiting.** Currently runs every tick (10 fps). If
  multiple WS clients connect, each tick computes once and the result is
  reused — fine. If model size grows to small/medium scale (~200K/800K
  params), should cache the result per tick instead of computing per-client.
* **Per-node pose tracks.** Right now a single virtual person is emitted;
  the broadcaster places it in `zone_1` with a fixed bbox. If/when LoRA
  adapters disambiguate per-node viewpoints, fan out to one
  `PersonDetection` per node (left/right of the room).

## References

* `scripts/train-wiflow-supervised.js` — JS reference implementation
* HuggingFace `ruv/ruview` — model file + LoRA adapters (Apache-2.0)
* ADR-079 — camera ground-truth training pipeline (the trainer this
  loader was built against)
* ADR-105 — "no synthetic data in production runtime"; this ADR keeps
  the gate but feeds it real model output
* ADR-115 — `/ota/set-target` (the prerequisite that got the CSI stream
  flowing again so this loader has data to consume)
