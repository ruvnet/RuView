# ADR-120 — Windowed Temporal Classifier (W-MLP)

**Status**: Accepted
**Date**: 2026-05-18
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs`
(`WindowedMlpModel`, `train_windowed_mlp_classifier`, `eval_windowed_mlp`,
`AdaptiveModel::classify_window`); `main.rs` (`AppStateInner.feature_window`,
`push_feature_window`, `adaptive_override` switching to window path).

## Context

ADR-119 added a small MLP (22 → 32 → 6) that improved accuracy from 49.58%
(LogReg) to **53.53%**. Loss flatlined at ~1.15 around epoch 10 of 30 —
clear signal that the **frame-level information ceiling** had been
reached for the 22-feature representation.

The dataset has 7 activity classes that differ primarily in **temporal
patterns**, not in any single frame:

* `walking` step cadence: ~2 Hz (visible in 0.5-second window)
* `transition` (sit-stand): ~0.5 Hz (visible in 2-second window)
* `waving` limb cadence: 1-2 Hz
* `active` (jumping): bursty / quasi-periodic at ~3 Hz
* `present_still` (sitting + standing merged): no temporal signature

Per-frame, `walking` and `active` and `waving` all look "moving" with
similar amplitude std/skew — they're disambiguated only by HOW the
amplitude pattern evolves over 1-2 seconds. A classifier that sees a
single frame can't tell them apart no matter how good the per-frame
features are.

## Decisions

### D1 — Stack 20 consecutive frames into a 440-d input

```
WINDOW_FRAMES   = 20  (~2 seconds at ~10 Hz tick rate)
N_FEATURES      = 22  (from ADR-118)
WINDOWED_INPUT  = 20 × 22 = 440
WINDOWED_HIDDEN = 64
```

Network: `440 → 64 ReLU → n_classes softmax`. ~28k weights total —
larger than the frame-level MLP's 3k, but still small enough to train
in <60s and serialize as JSON.

Training samples are built by sliding a window of 20 frames with **stride
5** within each recording (4× overlap). Windows do **not** cross recording
boundaries — each window inherits its source recording's class label.

On the 6-node 151k-frame set:
* 7 recordings × ~21k frames each = 151k frames total
* (21k − 20) / 5 ≈ 4,300 windows per recording
* Total: ~30k windowed samples
* Class balance is roughly preserved (each recording is one class)

### D2 — Manual backprop, same recipe as MLP

Same SGD + momentum 0.9 + weight decay 1e-4 + cosine LR decay. Base LR
lowered to 0.03 (vs MLP's 0.05) because the network is bigger. 25 epochs.
He initialisation, ReLU activation, softmax output, cross-entropy loss.

### D3 — `AdaptiveModel` carries all three classifiers, classify routes by availability

```rust
pub struct AdaptiveModel {
    pub weights: Vec<Vec<f64>>,     // ADR-118 legacy LogReg
    pub mlp: MlpModel,              // ADR-119 frame-level MLP
    pub windowed_mlp: WindowedMlpModel,  // ADR-120 (this) — primary
    // ...
}
```

`classify_window()` (new API) prefers `windowed_mlp` when trained AND
the caller has a 20-frame buffer. Falls through to frame-level MLP
when called with insufficient history. Old JSON model files load with
`MlpModel::default()` and `WindowedMlpModel::default()` filling absent
fields — backward compatible.

### D4 — Rolling buffer in `AppStateInner`, pushed per tick

```rust
struct AppStateInner {
    feature_window: VecDeque<[f64; N_FEATURES]>,  // capacity = WINDOW_FRAMES
    // ...
}
```

New helper `push_feature_window(&mut s, &features)` computes the 22-d
feature vector from current per-node amps, pushes to the back of the
buffer, evicts oldest when over capacity. Called at all three tick
sites where `adaptive_override` runs:
* `main.rs:~3030` — multi-BSSID tick handler
* `main.rs:~3225` — WiFi fallback tick handler
* `main.rs:~6510` — per-node loop in the broadcast tick task

`adaptive_override` (read-only over state) builds the 440-d input by
copying the buffer's last 19 entries + the current frame's features,
then calls `model.classify_window(&flat)`. Cold-start (buffer < 20)
falls back to `model.classify(&feat_arr)` — frame-level MLP.

## Verified Acceptance

Retrained on the same 6-node, 151,329-frame set used since ADR-118:

```
LogReg:    49.58%
MLP:       53.53%   (+3.95 vs LogReg)
W-MLP:     90.40%   (+36.87 vs MLP)
```

Per-class (frame-level MLP → W-MLP):

```
absent          41% → 100%   +59
present_still   99% → 100%   +1   (already saturated)
transition      36% →  86%   +50  (sit-stand cadence captured)
active          30% →  74%   +44  (jumping cadence captured)
waving          38% →  90%   +52  (gesture cadence captured)
present_moving  33% →  82%   +49  (walking step cadence captured)
```

Loss curve confirms breakout from the frame-level plateau:

```
MLP:     epoch  0 → 1.28 → epoch 29 → 1.14   (flat plateau)
W-MLP:   epoch  0 → 1.01 → epoch 24 → 0.25   (still trending)
```

Total cumulative improvement vs the start-of-session 2-node 15-feature
LogReg baseline:

```
40.4% → 90.40% = +50.0 percentage points
```

## Caveat — training vs generalization

90.40% is **training accuracy**. The W-MLP has ~28,800 weights trained
on ~30,200 windowed samples — capacity is comparable to dataset size,
so some overfitting is expected. True generalization performance will
only be measurable once an independent test set is captured.

Mitigations already in place:
* Weight decay 1e-4 regularises against memorisation
* Cosine LR decay with smooth annealing
* Stride 5 in window construction reduces near-duplicate samples
* Architecture stays small (one hidden layer) — limits overfit capacity

Recommended follow-up: record a 60-second held-out session per class
(separate from training), evaluate W-MLP cold, compare to training
accuracy. Expected drop: 5-15 pts for a healthy model.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs:
  + const WINDOW_FRAMES = 20, WINDOWED_INPUT = 440, WINDOWED_HIDDEN = 64
  + pub const N_FEATURES_PUB (for external buffer sizing)
  + pub struct WindowedMlpModel { w1, b1, w2, b2, n_classes }
  + impl WindowedMlpModel::{is_trained, forward}
  + AdaptiveModel.windowed_mlp field (serde-default)
  + AdaptiveModel::classify_window method
  + train_from_recordings builds recording_groups, slides windows,
    calls train_windowed_mlp_classifier
  + train_windowed_mlp_classifier (~150 LoC manual backprop)
  + eval_windowed_mlp helper
  + #[derive(Clone)] on Sample (for recording_groups Vec)
v2/crates/wifi-densepose-sensing-server/src/main.rs:
  + AppStateInner.feature_window: VecDeque<[f64; N_FEATURES_PUB]>
  + push_feature_window helper
  + adaptive_override switches to classify_window when buffer is full
  + 3 tick sites call push_feature_window before adaptive_override
docs/adr/ADR-120-windowed-temporal-classifier.md  (this)
```

## Out of Scope / Follow-ups

* **Held-out test set** — must record fresh data and evaluate the saved
  model cold. Critical to confirm 90% is not training-set memorisation.
* **TCN replacing stacked-MLP** — true 1D convolutions over time would
  use weights more efficiently (~5k vs 28k) and generalise better.
  Stack-MLP works but is parameter-heavy. Worth a follow-up if data
  scales 10×.
* **Sliding output smoothing** — `classify_window` emits one decision
  per tick (~10 Hz). Adjacent windows are 19/20 identical, so adjacent
  predictions should agree. They mostly do (98%+) but flicker at class
  boundaries — could apply a 3-tick majority filter.
* **`sitting` vs `standing` split** — both currently merge into
  `present_still`. The W-MLP gets them both right at 100% as a combined
  class. Splitting them would test whether temporal RF signatures
  differ between sitting (chair anchor) and standing (free body).
* **Class imbalance** — `present_still` has 2× the windows of other
  classes (sitting + standing both contribute). Acceptable since it's
  the "neutral" class, but oversampling minority classes might lift
  accuracy 1-2 pts further.
* **Smaller window size experiments** — 20 frames = 2 sec at ~10 Hz.
  Could try 10 frames (1 sec, faster reaction) or 30 (3 sec, more
  context). 20 was a reasonable first guess.

## References

* ADR-118 — feature decorrelation + multi-node (22-feature basis)
* ADR-119 — frame-level MLP (sibling classifier, fallback at cold start)
* ADR-101 — raw amplitude classifier (the path that calls
  `AdaptiveModel` via `adaptive_override`)
* ADR-105 — no synthetic data in production runtime; this ADR's
  confidence output is real model softmax probability, not a
  hardcoded value
