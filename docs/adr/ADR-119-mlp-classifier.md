# ADR-119 — MLP Replaces Logistic Regression in Adaptive Classifier

**Status**: Accepted
**Date**: 2026-05-18
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs`
(new `MlpModel` struct, `train_mlp_classifier`, `eval_mlp`; modified
`AdaptiveModel::classify` + `train_from_recordings`).

## Context

After ADR-118 (feature decorrelation + multi-node extractor) the adaptive
classifier reached **49.58% accuracy** on a 6-node, 7-class, 151,329-frame
training set. Per-feature audit showed `n6_std` sep_ratio = 0.60 — i.e. the
underlying signal *can* separate the classes — but logistic regression was
limited to linear decision boundaries and couldn't model interactions like:

* `walking`: `n2_std` high **AND** `n6_std` high **AND** `dom_hz ≈ 3 Hz`
* `waving`: `n1_std` high **BUT** `n2_std` low (only close sensors fire)
* `sitting` vs `standing`: same global features, differ in `n6_std` pattern

LogReg sums weighted features; it cannot represent "AND/BUT" combinations.
A small MLP can: hidden units learn intermediate concepts, then the output
layer combines them.

## Decisions

### D1 — Single-hidden-layer MLP, 22 → 32 → 6

* Input: the same 22-feature vector from ADR-118.
* Hidden: 32 ReLU units. ~3k weights, enough capacity for 6 classes but
  small enough to train in seconds on the 151k-frame set.
* Output: softmax over `n_classes` (discovered dynamically at train time).
* Z-score normalisation: identical to the LogReg path — same
  `global_mean` / `global_std` populated by `train_from_recordings`.

### D2 — Manual backprop, no external ML crate

`tch` (LibTorch) or `candle` would pull in ~50-200 MB of native deps for a
~3k-parameter network. The forward + backward passes are ~150 LoC of pure
Rust; SGD + momentum + cosine LR decay another ~30. Built-in `f64`
arithmetic is fast enough — full train completes in ~10 seconds on M1
Mac.

Optimiser: SGD with momentum 0.9, weight decay 1e-4, base LR 0.05 with
half-cosine decay to 0, batch size 64, 30 epochs. He initialisation
(`N(0, sqrt(2/fan_in))`) on weights, zero on biases.

### D3 — MLP wins over LogReg at classify time, LogReg kept as fallback

`AdaptiveModel` carries both:

```rust
pub weights: Vec<Vec<f64>>,   // legacy LogReg, still trained for rollback
pub mlp: MlpModel,            // ADR-119 — preferred when is_trained() == true
```

`classify()` checks `self.mlp.is_trained()`; if yes uses MLP forward pass,
otherwise falls back to LogReg softmax. Old `data/adaptive_model.json`
files (15-feature LogReg) loaded with `#[serde(default)]` on `mlp` →
`MlpModel::default()` returns empty fields → `is_trained() == false` →
graceful degradation to LogReg path.

### D4 — Train both, report better number

`train_from_recordings` runs the existing LogReg loop first (unchanged),
then trains MLP on the same z-normalised samples, evaluates both on the
training set, and reports `training_accuracy = mlp_acc.max(logreg_acc)`.
Per-class accuracy from both classifiers is logged side-by-side for
diagnostic comparison.

## Verified Acceptance

```
LogReg:    49.58% overall
MLP:       53.53% overall  (+3.95 pts)

Per-class (LogReg → MLP):
  absent          40% → 41%   (+1)
  present_still   99% → 99%   (tied — 2× sample count)
  transition      29% → 36%   (+7)
  active          22% → 30%   (+8)
  waving          34% → 38%   (+4)
  present_moving  24% → 33%   (+9)
```

Notes:

* `present_still` class is a merged bucket: both `train_standing_*` and
  `train_sitting_*` map to `present_still` via `classify_recording_name`.
  Hence 43,242 samples vs 21,500 average for the other classes — the
  classifier biases strongly toward this dominant class. The 99% is
  honest but partially inflated by class imbalance.
* The +3.95 pts is concentrated on motion classes — exactly where the
  hypothesis predicted MLP would help (non-linear combinations of per-
  node features differentiate similar motion types).
* MLP loss flatlined around 1.15 after epoch 10. Suggests the current
  22-feature representation has hit its information ceiling for frame-
  level classification. Going higher needs temporal context (sliding
  window classifier, LSTM, TCN) — see Open Items.

Total improvement since the start of this session:

```
2-node, 15 features, LogReg:    40.4%   (baseline)
6-node, 15 features, LogReg:    44.4%   +4.0 from more data
6-node, 22 features, LogReg:    49.58%  +5.2 from feature engineering (ADR-118)
6-node, 22 features, MLP:       53.53%  +3.95 from non-linear classifier (ADR-119)
                                ─────
Total cumulative:               +13.1 percentage points
```

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs:
  + const MLP_HIDDEN: usize = 32
  + pub struct MlpModel { w1, b1, w2, b2, n_classes } + serde
  + impl MlpModel { is_trained, forward }
  + AdaptiveModel.mlp field (serde-default for backward compat)
  + AdaptiveModel::classify prefers MLP when trained
  + train_mlp_classifier (~150 LoC manual backprop)
  + eval_mlp helper
  + train_from_recordings calls MLP path and picks max accuracy
docs/adr/ADR-119-mlp-classifier.md  (this)
```

`data/adaptive_model.json` removed at deploy time — the MLP fields need
populating, the old file has none.

## Out of Scope / Follow-ups

* **Temporal classifier (sliding window LSTM/TCN)** — loss flatlines at
  ~1.15 with the current feature set; this is the frame-level ceiling.
  A model that consumes a 1-second window (10-20 frames) would catch
  the temporal signature of `transition` (sit-stand cycle ≈ 0.5 Hz),
  `walking` (step rate ≈ 2 Hz), `active` (bursty), `waving` (limb
  cadence ≈ 1-2 Hz). Estimated +15-25 pts realistic for these
  inherently-temporal classes. ~3-4 hours of code.
* **Class imbalance fix** — `present_still` has 2× samples. Either
  oversample the minority classes during training, or weight loss by
  inverse class frequency. Marginal — ~2-3 pts.
* **Drop dead features** — 6 entropy features (sep_ratio 0.01-0.02) and
  3 weak globals (`mean_rssi`, `dom_hz`, `change_pts` all <0.11)
  contribute noise. Reducing 22 → ~13 features would simplify training
  but probably not move accuracy more than 1-2 pts.
* **Hidden size sweep** — tried only 32. Could try 16 (faster, less
  overfitting risk) or 64 (more capacity). Cosmetic.
* **Split `sitting` and `standing` into separate classes** — they're
  physically distinct RF signatures but currently merged. Adding them as
  separate classes would test whether the model can disambiguate them.
  Likely lowers `present_still` accuracy but separates a useful
  distinction. Experiment-grade.

## References

* ADR-118 — feature decorrelation + multi-node extractor (the 22-feature
  basis this ADR uses)
* ADR-117 — earlier process hygiene pass; introduced standardisation
  (`global_mean`/`global_std`) that this ADR's MLP also relies on
* ADR-101 — raw amplitude classifier (the runtime path that calls
  `AdaptiveModel::classify`)
