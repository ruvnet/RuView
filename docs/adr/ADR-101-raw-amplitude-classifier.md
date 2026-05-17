# ADR-101 — Raw-Amplitude Presence/Motion Classifier

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`amp_presence_override`, `amp_classify_from_latest`,
`amp_node_level`, `amp_node_snapshot`).

## Context

After ADR-100 the AGC drift is gone and the broadband baseline is clean.
Before this ADR the live `classification.motion_level` was being driven
by the legacy DSP (variance + motion_band_power thresholds) plus an
RSSI MAD-Δ override from ADR-110. Both failed on the operator's
deployment: variance overlaps empty/sit/walk within noise, and RSSI
MAD-Δ overlaps within ±0.03 of 0.49 across all three states. The
operator could lie still in the path between AP and sensor and the
detector would silently report `absent`.

The 30 sec × 3 controlled captures done on 2026-05-17 (lying between
TP-Link AP and sensor 1, see ADR-100 *Verified Acceptance*) showed
that **the broadband CV of mean amplitude separates the three states
by 3-6× on this geometry**. EMPTY = 2.7-5 %, STILL = 3.7-5 %,
WALK = 12.5-29.7 %. EMPTY vs STILL are best separated by the
**mean-amplitude drop** (37 → 22 on the active sensor, -40 %).

This ADR replaces the RSSI MAD-Δ classifier with a pure-amplitude one
that uses both signals: CV for motion, baseline drop for static body.

## Decisions

### D1 — `amp_presence_override` per-node classifier

For each frame received on the raw-CSI path:

1. Push current full amplitude vector into the NBVI ranking buffer
   (`nbvi_history`, capacity 600 frames ≈ 30 s).
2. Periodically (every `NBVI_REFRESH_TICKS=200` calls, ~5 s) rank
   subcarriers by NBVI (see ADR-102) and pick the top-12.
3. Compute **broadband_mean** as the average of NBVI-selected
   subcarriers. Falls back to all non-zero subcarriers during warmup.
4. Push to two rolling windows:
   - `short` (90 samples ≈ 4.5 s) — for CV.
   - `long` (1200 samples ≈ 60 s) — for the rolling-fallback 95 %ile
     baseline.
5. Compute `cv = std(short) / mean(short)`.
6. Compute `baseline` — see ADR-103 for the persistent-override path.
7. Stash `(cv, mean_short, baseline)` per node in `AMP_LATEST` for
   cross-node fusion.
8. Run `amp_classify_from_latest` (D2 below) to produce the global
   `(level, presence, confidence)`.

Returns `None` until the short window is full so the very first
seconds after boot don't emit garbage.

### D2 — Cross-node fusion in `amp_classify_from_latest`

The deployment has two sensors with very different SNR (node 1 mean
amplitude ~22, node 2 mean ~9 on the operator's TP-Link). A single
bursty node should not flip the whole detector. We use:

* **MAX CV** across nodes for the motion gate. Any node seeing
  movement is enough — body modulates only the line-of-sight path
  it crosses, the other node may stay clean.
* **ANY baseline drop** → `present_still`. One well-placed node
  seeing the body is enough.

Decision (universal-threshold normalized — see ADR-103 D3):

```
norm_max_cv = max_cv / baseline_cv     (when calibration loaded)
gates:                                 fallback when no calibration:
  norm ≥ 6.0  → "active"               max_cv ≥ 0.22
  norm ≥ 3.0  → "present_moving"       max_cv ≥ 0.10
  any drop    → "present_still"        (same)
  otherwise   → "absent"               (same)
```

### D3 — Sticky 3-second motion hysteresis

After each fusion pass, a global `AMP_HOLD` counter is reset to
`AMP_MOTION_HOLD_TICKS = 120` whenever the candidate is `moving` /
`active`. Each subsequent quiet tick decrements the counter; the
prior motion label is kept until it expires (≈ 3 s at the ~40
combined classifier ticks/s). This bridges the brief CV dips between
walking steps so the GLOBAL doesn't flicker between `moving` and
`absent`.

### D4 — `amp_classify_from_latest` read-only entry point

The server has multiple `SensingUpdate` producers — the raw-CSI path
runs the full pipeline above, but the feature_state path (0xC5110006)
arrives without raw amplitudes. We expose a parallel read-only
classifier that pulls the latest stashed per-node `(cv, mean, baseline)`
from `AMP_LATEST` and runs the same fusion. The feature_state path
calls it so its emitted `classification` agrees with the raw-CSI
path's — no flicker between the two SensingUpdate sources.

### D5 — Per-node labels in `build_node_features`

`PerNodeFeatureInfo.classification` is overridden via
`amp_node_snapshot(node_id)`, which runs the same per-node
classifier (without cross-node fusion or hysteresis) against the
stashed `(cv, mean, baseline)` for that node alone. UI consumers
(raw.html badges) see each sensor's independent decision plus the
global fused one — useful for finding sensor placement without
moving them.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs   # ~230 lines added
v2/crates/wifi-densepose-sensing-server/static/raw.html  # per-node badges
```

## Verified Acceptance

| State | GLOBAL | CV | Per-node detail |
|---|---|---|---|
| EMPTY | `absent` | 4-6 % | both nodes baseline mean, low CV |
| STILL (lying, in node 1 path) | `present_still` | 3-8 % | node 1 mean drops 70 %, RSSI -20 dB |
| WALK | `active` | 12-36 % | node 2 CV explodes, RSSI swings ±5 dB |

Cross-state separation ratio = 3.4× on node 1 broadband mean, 5.9×
on node 2 CV, compared to ±0.02 inside ±0.10 noise with the old
RSSI MAD-Δ classifier from ADR-110.

## Open Items

* ✅ **Per-subcarrier baseline-drop** — closed in ADR-104 (per-sub
  drift channel with 10 % gate, triggers `present_still` even when
  broadband doesn't move).
* ✅ **Off-axis sit doesn't trigger** — closed in ADR-104 (drift
  channel catches off-line-of-sight body presence).
* ⏳ **CV saturates above ~30 %** — still open. Heavy-motion granularity
  (run vs jog vs jump) lost above the `active` gate. Would need a
  log-CV or rank-based metric to extend the dynamic range.

## References

* ADR-110 — first RSSI MAD-Δ attempt (superseded for `motion_level` /
  `presence` / `confidence`; helper kept as `#[allow(dead_code)]`).
* ADR-100 — gain lock that makes this classifier possible.
* ADR-102 — NBVI subcarrier selection that drives the CV computation.
* ADR-103 — persistent baseline + universal threshold normalization.
* [`docs/references/espectre-techniques.md`](../references/espectre-techniques.md)
  — full RuView ↔ ESPectre comparison.
