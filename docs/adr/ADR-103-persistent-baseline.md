# ADR-103 — Persistent Empty-Room Baseline + Universal Threshold

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`AMP_BASELINE_OVERRIDE`, `AMP_BASELINE_CV`, `load_baseline_file`,
`amp_node_level`), `v2/data/baseline.json`, `scripts/record-baseline.py`.

## Context

ADR-101's classifier relies on a `baseline` value per node — the
mean amplitude the room exhibits when empty. Pre-ADR-103 the baseline
was the rolling 95 %ile of the last 1 200 samples (≈ 60 s) of
broadband mean. That meant every server restart triggered a "step
outside for 60 seconds" ritual before the detector worked, and if
the operator stayed in the room longer than ~4 min the baseline
silently drifted down to the *occupied* amplitude — making
`present_still` under-trigger forever after.

Additionally, motion gates were hard-coded to the operator's
deployment (10 % / 22 % CV) — wouldn't transfer to a different room
with different noise floor.

## Decisions

### D1 — Persistent baseline file at `data/baseline.json`

JSON schema **v2** (per node):

```json
{
  "version": 2,
  "captured_at": "ISO-8601",
  "duration_sec": 90.0,
  "trim_head_sec": 15.0,
  "trim_tail_sec": 15.0,
  "clean_window_sec": 30.0,
  "method": "record → trim head/tail → find lowest-CV sub-window → FULL-broadband stats per node",
  "nodes": {
    "1": {
      "full_broadband_mean":  26.11,
      "full_broadband_p50":   26.16,
      "full_broadband_p95":   27.04,   ← used as `baseline`
      "full_broadband_std":    0.68,
      "full_broadband_cv_pct": 2.62,   ← used to normalize gates (D3)
      "rssi_dbm":             -52.3,
      "n_samples":            149,
      "per_subcarrier_mean":  [..56 floats..]
    }
  }
}
```

Loader (`load_baseline_file`) reads at server startup. Path is
`$RUVIEW_BASELINE_FILE` or `data/baseline.json` by default. Missing
or unparseable file = log warning + fall back to rolling p95 (= old
ADR-101 behaviour, no breaking change).

Per-node lookup priority: `full_broadband_p95` → `full_broadband_mean`
→ legacy `p95_amp` → legacy `mean_amp`. v1 baselines load but get
warning about NBVI-drift incompatibility.

### D2 — FULL broadband for baseline comparison, NBVI for CV

The persisted baseline must be comparable across server restarts.
NBVI top-12 re-selects on each boot (ADR-102 D4), so a NBVI-subset
mean recorded today doesn't match a NBVI-subset mean tomorrow even
in the same empty room. Fix:

`amp_presence_override` keeps two short windows:

| Field | Source | Used for |
|---|---|---|
| `short` | NBVI-subset broadband mean | CV (motion sensitivity) |
| `short_full` | **all non-zero subcarriers** mean | baseline drop check |

The recording script also computes full-broadband statistics from
the captured frames. Both sides of `mean / baseline` ratio are
full-broadband ⇒ stable across NBVI selection.

### D3 — Universal threshold via baseline-CV normalization

(Pace's Problem #3.) Hard-coded gates are deployment-tuned. Fix:
normalize the runtime CV by the empty-room CV measured during
calibration:

```
norm_cv = current_cv / baseline_cv
gates:  norm_cv ≥ 3.0  → present_moving
        norm_cv ≥ 6.0  → active
```

Both `amp_node_level` (per-node) and `amp_classify_from_latest`
(global) use the same normalization. When no calibration is loaded,
fall back to absolute gates `0.10 / 0.22` (the deployment-tuned
values) — keeps backwards compatibility.

`AMP_BASELINE_CV` is a separate per-node map loaded alongside
`AMP_BASELINE_OVERRIDE`. The CV value is the FULL-broadband CV % from
the calibration file divided by 100.

### D4 — Recording script `scripts/record-baseline.py`

CLI helper (Python 3, requires `pip install websockets`). Connects
to the live `ws://localhost:8765/ws/sensing`, records `duration` (90
s default), then:

1. Trim `trim_head_sec` (15 s default) and `trim_tail_sec` (15 s
   default) to discard door-open / re-entry transients.
2. Slide a `clean_window_sec` (30 s default) sub-window across the
   trimmed buffer, pick the one with the lowest broadband CV.
3. Per node, compute full-broadband mean / median / p95 / std / CV %
   and rssi mean over that cleanest window.
4. Also compute per-subcarrier mean across the cleanest window (saved
   as diagnostic for future per-subcarrier delta classifier).
5. Write `v2/data/baseline.json` (path overridable via `--out`).

Operator workflow now: step out, run script, come back, restart
server. One-time per deployment (or after room rearrangement). No
recurring ritual.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs   # ~120 lines added
v2/data/baseline.json                                  # new, gitignored?
scripts/record-baseline.py                             # new helper
docs/adr/ADR-103-persistent-baseline.md                # this ADR
```

## Verified Acceptance (operator's deployment, 2026-05-17)

```
boot: baseline: loaded 2 node overrides from data/baseline.json
      (node1=27.04, node2=14.72;
       node1_cv=2.62%, node2_cv=3.65%)
```

Empty room, immediately after restart (no warmup wait):

```
GLOBAL: absent  CV=5.0%
  node 1 ratio=0.93, norm_cv=0.80×
  node 2 ratio=0.93, norm_cv=0.83×
```

Sitting in node 2 path (off-axis from node 1):

```
GLOBAL: present_still  CV=8.1%
  node 1 ratio=1.05, norm_cv=1.2×   (not in path, no drop)
  node 2 ratio=0.70, norm_cv=1.7× ← drop fires present_still
```

Walking:

```
GLOBAL: active  CV=28-36%
  node 1 norm_cv=4-6×, node 2 norm_cv=7-9× ← well above 6× gate
```

Universal-threshold gates `3.0 / 6.0` map to the same absolute
12 % / 22 % we hand-tuned earlier — but now any-room-portable.

## Open Items

* ✅ **REST endpoint POST /api/v1/baseline/calibrate** — closed in
  ADR-107 D3 + UI button D6.
* ✅ **Per-subcarrier baseline comparison** — closed in ADR-104
  (per-sub drift channel consumes `per_subcarrier_mean`).
* ✅ **Auto-recalibrate on long quiet periods** — closed in ADR-107 D5
  (30-min quiet + 1-h cooldown defaults).

## References

* ADR-100 — gain lock.
* ADR-101 — classifier consumes the baseline.
* ADR-102 — NBVI selection drift was the root cause of D1/D2.
* [`docs/references/espectre-techniques.md`](../references/espectre-techniques.md)
  — Pace's full technique catalogue including Problem #3 normalization.
