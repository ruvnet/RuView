# ADR-104 — Per-Subcarrier Drift Presence Channel + NBVI FP-Rate Validation

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`AMP_BASELINE_PER_SUB`, `AMP_DRIFT`, `amp_drift_for_node`,
`amp_drift_max`, `amp_node_level`, `amp_classify_from_latest`,
`nbvi_select_top_k` Step 3), `scripts/record-baseline.py`
(`per_subcarrier_mean` already saved).

## Context

After ADR-103 the classifier triggers `present_still` only when the
**broadband mean** of the NBVI-selected subset drops by ≥ 25 % from
the loaded baseline. This works when the operator's body crosses the
line of sight between AP and sensor — direct-component attenuation
dominates. But:

1. **Off-axis presence**: the operator sitting at a desk to the side
   of the AP-sensor line modulates only a handful of subcarriers
   (the ones whose Fresnel zone happens to brush their body). The
   *broadband* mean barely shifts; ADR-103 says `absent` even though
   someone is clearly in the room.
2. **NBVI Step 3**: Pace's full NBVI pipeline picks top-K by raw NBVI
   score, then **validates** each candidate K by counting false
   positives the motion detector would produce on the calibration
   buffer, and keeps the K with the lowest FP rate. We were taking
   the raw top-12 without validation — fragile if one of the chosen
   subcarriers happens to overlap a noise source.

## Decisions

### D1 — Spectral drift score as a second presence channel

`amp_presence_override` per node now also computes a **spectral
drift score**:

```
drift_k = (current_amp[k] - baseline_amp[k]).abs() / baseline_amp[k]    for baseline[k] > 1.0
drift   = mean(drift_k) across kept subcarriers
```

`current_amp[k]` = mean of the recent `AMP_SHORT_WIN` (90) frames'
amplitude at subcarrier `k`. `baseline_amp[k]` = the
`per_subcarrier_mean` vector saved by ADR-103's recording script.

Per-node drift is stashed in `AMP_DRIFT: HashMap<u8, f64>` so
`amp_node_level` (per-node) and `amp_classify_from_latest` (global)
can use it. Threshold `AMP_DRIFT_PRESENCE_THRESH = 0.10` (10 %
average per-subcarrier deviation) is empirical and consistent with
the broadband-ratio trigger (drop ≥ 25 %, drift ≥ 10 %).

### D2 — Trigger order in classifier

Per node (`amp_node_snapshot`):

```
1. CV ≥ 6× baseline_cv  → active
2. CV ≥ 3× baseline_cv  → present_moving
3. drift ≥ 10 %         → present_still   ← ADR-104 (off-axis)
4. mean / baseline < 0.75 → present_still ← ADR-101 (in-path)
5. otherwise            → absent
```

Global (`amp_classify_from_latest`) uses MAX CV / MAX drift / ANY
baseline-drop across nodes. Either drop OR drift fires `present_still`.

### D3 — Opportunistic loading

`per_subcarrier_mean` was already being written by
`scripts/record-baseline.py` (line ~132, written as a list of
~56 floats per node) but the server ignored it. Now `load_baseline_file`
parses it and populates `AMP_BASELINE_PER_SUB`. If absent (older
`baseline.json` from before this ADR) → drift stays 0.0 → no behaviour
change. Re-trigger calibration via the ADR-107 REST endpoint or auto-
recalibrate to populate the field and activate the drift channel.

### D4 — NBVI FP-rate validation (Step 3 of Pace's spec)

`nbvi_select_top_k` no longer returns the literal top-K. After
ranking by NBVI score (Steps 1+2), it evaluates each candidate
K ∈ `{6, 8, 10, 12, 16, 20}` clamped to the available subcarrier
pool:

* For each K: compute per-frame broadband mean over the top-K
  subset across the quiet window.
* Slide a sub-window (length `AMP_SHORT_WIN/3 ≈ 30` samples, stride
  `sub_window/2`) and count windows where rolling CV exceeds the
  moving-gate threshold (0.10).
* Pick the K with the **smallest FP count**. Ties broken by smallest
  total NBVI score (less noisy subset wins).

Result: a subset that's stable AND non-FP-producing on the calibration
window. If a top-12 NBVI candidate sneaks in a subcarrier overlapping
a noise source, the FP count surfaces it and a smaller K wins instead.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - statics: AMP_BASELINE_PER_SUB, AMP_DRIFT
  - helpers: amp_baseline_per_sub_init, amp_drift_init,
             amp_drift_for_node, amp_drift_max
  - load_baseline_file: parse per_subcarrier_mean → AMP_BASELINE_PER_SUB
  - amp_presence_override: drift computation + stash
  - amp_node_level: drift trigger (uses MAX for cross-node)
  - amp_node_snapshot: per-node drift trigger (overrides MAX)
  - amp_classify_from_latest: any-node drift trigger in global fusion
  - nbvi_select_top_k: Step 3 FP-rate validation
docs/adr/ADR-104-per-subcarrier-drift-presence.md  (this)
```

Implementation commit: `6212b17e`.

## Verified Acceptance

Server boot log (using existing v1 baseline.json without
`per_subcarrier_mean`):

```
baseline: loaded 2 node overrides from data/baseline.json
          (node1=27.04, node2=14.72; node1_cv=2.62%, node2_cv=3.65%)
```

Without `per_subcarrier_mean` in the file, drift is identically 0
and the classifier behaves exactly as ADR-103. To activate the
drift channel: re-record via the ADR-107 REST endpoint or wait for
auto-recalibrate; new `baseline.json` carries the
`per_subcarrier_mean` vector and drift becomes live.

NBVI Step 3 validation runs on every refresh tick. With K=12 being
the "safe" default that always passes (clean low-CV window in the
operator's deployment) and smaller Ks not improving FP=0, the picker
keeps K=12 in steady state. Defends against future drift in channel
conditions where a previously-clean subcarrier picks up interference.

## Open Items

* **Per-subcarrier baseline AGE check** — the per-sub vector reflects
  the channel at calibration time. As the channel slowly drifts (other
  WiFi clients on the AP, temperature, etc.) the per-sub baseline ages
  faster than the broadband-mean baseline. Need: if `last_written_sec_ago`
  > N hours AND drift consistently > threshold → flag for
  re-calibration. Defer to a future ADR-109.
* **Per-subcarrier delta in UI** — `raw.html` only shows broadband
  bars + global classification. A small "drift" sparkline per node
  would let the operator see the off-axis channel firing. ~30 min.
* **Phase-domain drift** — currently amplitude-only. Phase delta vs
  baseline phase would catch even subtler movement (chest-wall sub-mm
  motion during breathing). Requires phase baseline in `baseline.json`,
  which the recording script doesn't yet save. ~1 h script + ~30 min
  server.

## References

* ADR-101 — broadband classifier; this ADR adds a parallel channel.
* ADR-102 — NBVI; this ADR adds Step 3 validation per Pace's spec.
* ADR-103 — persistent baseline; `per_subcarrier_mean` already written.
* ADR-107 — REST calibrate endpoint; how the operator refreshes the
  per-sub vector on demand.
* [`docs/references/espectre-techniques.md`](../references/espectre-techniques.md)
  §1.Step 3.
