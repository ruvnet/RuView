# ADR-102 — NBVI Subcarrier Selection (server-side)

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`AmpState.nbvi_*`, `nbvi_select_top_k`).

## Context

Each ESP32-S3 CSI frame carries 56 active subcarriers on the HT20
20 MHz channel. The amplitudes per subcarrier have very different
SNR depending on frequency-selective fading: in the operator's
deployment subcarriers `k=6..11` and `k=22..26` sit at CV ≈ 6 % when
the room is empty, while subcarriers `k=38..43` (middle of the band,
near the LTF nulls) sit at CV ≈ 11 % — pure channel noise, no
information about the room.

ADR-101's classifier computes broadband-mean CV. Averaging over all
56 subcarriers means the noisy ones drag the baseline CV up to
5-7 %. That blunted the motion gates and we had to push them up to
10-22 %, losing sensitivity to subtle motion.

## Decisions

### D1 — Port Francesco Pace's NBVI to the server (not the FW)

Formula (ESPectre, GPLv3):

```
NBVI(k) = α · (σ_k / μ_k²) + (1 - α) · (σ_k / μ_k),     α = 0.5
```

* `σ_k / μ_k²` — penalises weak subcarriers (a quiet bin with mean ≈ 0
  gets `∞` and is filtered out).
* `σ_k / μ_k` — standard coefficient of variation; rewards stability.
* `α = 0.5` — empirically balanced (per Pace's α-sweep tests).

**Where**: in the server, not in FW. Pros: trivial to retune per
deployment, no flash cycle, single source of truth across two FW
variants we ship (`runbot_csi_node` and `esp32s3_csi_capture`). Cons:
we lose the ability to *only emit* selected subcarriers (would save
UDP bandwidth) — but at ~25 fps × 56 × 2 bytes = 2.8 KB/s per node,
bandwidth isn't a concern.

### D2 — Top-K with K = 12

Selected at server boot once `nbvi_history` has 90+ samples; then
re-selected every `NBVI_REFRESH_TICKS = 200` calls (~5 s of combined
classifier ticks). The selected indices live in
`AmpState.nbvi_selected`.

K=12 matches ESPectre's default. Smaller K = less averaging
smoothing; larger K = drags in worse subcarriers.

### D3 — Dead-zone gate at 25 % of median mean

Before NBVI scoring, drop any subcarrier whose mean amplitude is
below `0.25 × median(all subcarrier means)`. Guard tones (FW reports
amp[0] = 0 for DC), edge bins, and dead frequencies are excluded so
they can't "win" with σ/μ² → ∞.

### D4 — ESPectre Step 1: quiet-window finder

Naive NBVI ranking over the *entire* history is biased if a body
walked through during the calibration buffer. ADR-102 v2 adds the
quiet-window finder from Pace's Step 1:

1. Slide an `AMP_SHORT_WIN=90`-sample window across `nbvi_history`
   with stride `AMP_SHORT_WIN/3 = 30`.
2. For each window, compute the CV of its per-frame broadband mean.
3. The window with the lowest CV is "quietest".
4. Per-subcarrier mean and std for NBVI scoring use **only that
   window**.

If history is smaller than one window, the whole buffer is used.
Stride 30 (overlap of 60) keeps wall-clock cost trivial for 600
frames.

### D5 — `mean_for_baseline` uses FULL broadband, not NBVI

NBVI top-K re-selects between server restarts (different "quietest"
window may give different ranking). That made the persisted baseline
value incomparable across restarts (see ADR-103 D1). Fix: ADR-101
classifier keeps a parallel `short_full` ring buffer of FULL
broadband means (all non-zero subcarriers, no NBVI filter). When
ADR-103's persistent override is active, the baseline-drop check
compares full-broadband short window to full-broadband baseline.
NBVI subset is still used for CV (motion sensitivity is what NBVI
shines at — full broadband mean is just the integral level).

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - struct AmpState
  - nbvi_select_top_k()
  - amp_presence_override() (broadband_mean computation)
```

## Verified Acceptance (operator's deployment, 2026-05-17)

Idle empty-room CV, sensing-server with 2 pps housekeeping ping:

| | Full 56 subc | NBVI top-12 |
|---|---|---|
| node 1 (rssi -53 dBm) | ~5.0 % | **3.1 %** |
| node 2 (rssi -67 dBm) | ~7.0 % | **3.9 %** |

Reduction 38-44 %. The lower baseline let ADR-101 gates be tightened
from `15 % / 30 %` down to `10 % / 22 %` for moving/active without
raising the false-positive rate — subtler motions like waving while
sitting near a sensor now trigger.

## Open Items

* ✅ **Step 3 FP-rate validation** — closed in ADR-104 D4 (commit
  `6212b17e`). K ∈ {6,8,10,12,16,20} sweep, smallest-FP wins; ties
  broken by smallest total-NBVI score.
* **Persist NBVI selection** — `AMP_BASELINE_OVERRIDE` (ADR-103)
  persists baseline scalar but not the chosen subcarrier indices.
  After server restart NBVI re-ranks from scratch; in deployments
  where the channel changes over hours we'd want to re-rank anyway,
  so for now this is correct, not an open item.
* **FW boot-time NBVI freeze** — ESPectre's Pace freezes NBVI for
  the lifetime of the boot. Trade-off vs our adaptive rolling
  refresh. Worth exploring if FP rate is a problem in real homes.

## References

* ADR-100 — gain lock (gives NBVI a stable per-subcarrier baseline).
* ADR-101 — classifier that consumes NBVI selection.
* ADR-103 — persistent baseline + universal threshold normalization.
* [Pace's *Part 2*](https://medium.com/@francesco.pace/how-i-turned-my-wi-fi-into-a-motion-sensor-part-2-62038130e530)
  + [francescopace/espectre](https://github.com/francescopace/espectre)
  on GitHub (GPLv3).
* [`docs/references/espectre-techniques.md`](../references/espectre-techniques.md).
