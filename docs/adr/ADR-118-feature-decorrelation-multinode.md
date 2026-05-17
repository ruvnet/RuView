# ADR-118 — Feature Decorrelation + Multi-node Extractor (Adaptive Classifier)

**Status**: Accepted
**Date**: 2026-05-18
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs`
(`N_FEATURES`, `features_from_frame`, `features_from_runtime`), call sites in
`main.rs::adaptive_override`, `main.rs:~6200` per-node loop, and
`csi.rs::adaptive_override`.

## Context

After ADR-117 the adaptive_classifier produced **40.4% accuracy** on a
2-node, 7-class training set (52,857 frames). Adding 4 more sensors and
recording the same 7 classes at 6 nodes increased the set to **151,329 frames
(2.9× more data)** but accuracy only moved to **44.4%** (+4 pts).

Diagnostic Python audit (run against both datasets) found three architectural
defects in the feature pipeline, not the data:

| Defect | 2-node set | 6-node set |
|---|---|---|
| Constant feature (`amp_min = 0.00` across all frames — HT20 null subcarrier) | ✗ dead | ✗ dead |
| Multicollinear pairs `|r| > 0.85` | 17 pairs | 21 pairs |
| Top F-stat vs accuracy | F=1,516, acc 40.4% | F=15,497, acc 44.4% |

The 10× higher F-stat on 6-node data confirmed the **signal was getting
stronger** but the classifier couldn't extract it. Root cause:
`features_from_frame` used only `nodes.first()` — 5 of 6 sensors carried
**zero weight** in the feature vector. Adding nodes physically helped, but
only via the small contribution to the 7 aggregated server-level features.

Within a single node, the 8 subcarrier scalars were 90-99% correlated with
each other (mean ≈ std ≈ max ≈ p25/75/90 — they all measure "amplitude
level"). And the 4 energy features (variance, motion_band_power,
breathing_band_power, spectral_power) were 87-99% correlated. The 15-feature
space had effective rank ≈ 5.

## Decisions

### D1 — Drop the dead and redundant features

* **Dropped**: `amp_min` (constant 0), `amp_range = max − min ≡ max`
  (collinear), `motion_band_power`/`breathing_band_power`/`spectral_power`
  (all r > 0.95 with `variance`), `amp_mean`/`amp_max`/`amp_iqr`/`amp_kurt`
  (all r > 0.90 with `amp_std`).
* **Kept (globally)**: `variance`, `mean_rssi`, `dominant_freq_hz`,
  `change_points` — the 4 server-level features that retained marginal
  independence.

### D2 — Per-node features × all 6 nodes

For each node id `N ∈ {1..6}`, extract 3 features:

* `amp_std` — multipath spread (motion-sensitive)
* `amp_skew` — distribution asymmetry (sensitive to dominant scatterer
  position relative to this sensor)
* `amp_entropy` — spectral diversity (normalised to [0, 1])

Total: `4 + 6 × 3 = 22 features`. Each node's contribution lives at a fixed
offset (`base = 4 + (node_id - 1) × 3`) so 5 of 6 sensors are no longer
discarded.

Missing-node features are zero-padded; z-score normalisation (already in
the model from ADR-117 era) treats them consistently across train and
classify.

### D3 — `features_from_runtime` signature change

Old:

```rust
pub fn features_from_runtime(feat: &Value, amps: &[f64]) -> [f64; 15]
```

New:

```rust
pub fn features_from_runtime(
    feat: &Value,
    per_node_amps: &[(u8, &[f64])],
) -> [f64; 22]
```

Three call sites updated:

1. `main.rs::adaptive_override` (global state path) — new helper
   `current_per_node_amps()` reads `AMP_HIST.nbvi_history.back()` for each
   active node, then passes the slice.
2. `main.rs:~6200` (per-node loop in the broadcast tick task) — same
   helper, called once per tick.
3. `csi.rs::adaptive_override` (legacy, no live callers) — degraded to
   single-node fallback with `[(1u8, amps)]`; documented as emergency only.

### D4 — Old 15-feature model file is incompatible

`AdaptiveModel` serializes `[f64; N_FEATURES]` arrays. Loading a 15-array
into a 22-slot field fails. `data/adaptive_model.json` removed at deploy
time; first start re-runs `train_from_recordings` over the existing 7 train
files.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/adaptive_classifier.rs:
  * N_FEATURES: 15 → 22
  * New constants N_GLOBAL_FEATURES=4, N_PER_NODE_FEATURES=3, MAX_NODES=6
  * features_from_frame rewritten — multi-node + decorrelated
  * features_from_runtime signature changed
  * per_node_stats helper (3 scalars: std/skew/entropy)
  * Old subcarrier_stats removed
v2/crates/wifi-densepose-sensing-server/src/main.rs:
  + current_per_node_amps() helper (snapshots AMP_HIST.nbvi_history.back())
  + 2 call sites updated to pass &[(u8, &[f64])] instead of &[f64]
v2/crates/wifi-densepose-sensing-server/src/csi.rs:
  + adaptive_override updated to new signature (dead code path, kept for ABI)
data/adaptive_model.json: removed (15-feature incompatible)
docs/adr/ADR-118-feature-decorrelation-multinode.md (this)
```

## Verified Acceptance

Re-ran `POST /api/v1/adaptive/train` against the same 151,329-frame 6-node
recording set:

```
2-node, 15 features:    40.4%
6-node, 15 features:    44.4%    (+4.0 from more data)
6-node, 22 features:   49.58%    (+5.2 from feature engineering)
```

Total improvement: **+9.2 percentage points** from the baseline, on the
same hardware in the same room.

Live confidence distribution (10s samples post-retrain):

```
absent:           conf 0.30-0.85 (was 0.04-0.10 pre-ADR-118)
present_still:    conf 0.40-0.85
present_moving:   conf 0.30-0.50
active:           conf 0.27-0.45
transition:       conf 0.84-0.86 (high — model has clear signal for this)
waving:           conf — class not active during sample window
```

Confidence is now meaningful (model has separation), whereas pre-ADR-118 the
near-uniform 0.04-0.10 indicated the classifier was essentially flipping a
coin.

### Per-feature class separability (post-train, sep_ratio = between-class
spread / within-class std):

| Feature | sep_ratio | Verdict |
|---|---|---|
| `n6_std`  | 0.60 ★ | best — node 6 near door catches both motion + door state |
| `n2_std`  | 0.35   | second — node 2 far from AP, high modulation |
| `n6_skew` | 0.25   | useful |
| `n3_skew` | 0.26   | useful |
| `n2_skew` | 0.18   | marginal |
| `n4_std`  | 0.14   | marginal |
| `n1_*`    | 0.01-0.06 | near AP — almost no class signal |
| `n5_*`    | 0.01-0.05 | similar to n1 |
| all `entropy` features | 0.01-0.02 | **dead** — distribution shape doesn't vary by activity |
| `variance` (global) | 0.11 | weak |
| `mean_rssi` (global) | 0.01 | dead at this scale |

## Open Items

* **`*_entropy` features carry no signal** (sep_ratio ~0.01 across all 6
  nodes). Could be dropped: 22 → 16 features. Marginal expected gain (~1%),
  not worth a follow-up ADR right now.
* **Aggregated server features all sub-0.11** — `mean_rssi` / `dom_hz` /
  `change_pts` could go too. Would reduce to 12-13 truly useful features.
* **Logistic regression ceiling** — `n6_std` alone has sep_ratio 0.60 but
  a linear classifier can't fully exploit non-linear class boundaries.
  Next big lever is replacing the LogReg with a small MLP or random forest.
  Out of scope here.
* **`standing` and `sitting` recordings collapse to one class** — file
  naming maps both to `present_still`. They're physically distinct
  signatures (different RF profile) but the trainer treats them as one.
  Separating them in `classify_recording_name` would add a class but might
  lower accuracy due to inherent confusability — TBD via experiment.
* **Sensor placement matters more than algorithm tweaks** — n1/n5 (near AP)
  carry almost no class signal. Reposition them away from the AP if
  possible (closer to walking zone, farther from the line-of-sight to AP).

## References

* ADR-101 — raw amplitude classifier (the runtime classifier this adaptive
  model can override)
* ADR-117 — process hygiene + previous training infrastructure
* `data/recordings/archive_2node_2026-05-17/` — earlier 2-node training
  set, kept for comparison; not used by trainer (outside `recordings/`
  root scope)
