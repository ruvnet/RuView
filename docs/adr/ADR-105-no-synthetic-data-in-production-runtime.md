# ADR-105 — No Synthetic Data in Production Runtime

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(REST handlers under `/api/v1/pose/*`, `/api/v1/info`,
`derive_pose_from_sensing`, `generate_signal_field`).

## Context

After we pulled the upstream Docker UI (`ruvnet/wifi-densepose:latest`)
and pointed it at our backend via `--ui-path /tmp/wdp_ui/ui`, the
operator inspected the rich SPA and noticed several panels showing
data we have no business showing:

* **Pose dashboard rendered a 17-keypoint skeleton** even though no
  DensePose model is loaded. Trace: `derive_pose_from_sensing` →
  `derive_single_person_pose` synthesised a geometric placeholder
  with keypoint `confidence = 0.0` but plausible-looking coordinates.
* **`/api/v1/pose/stats.average_confidence` was the literal `0.87`**
  hard-coded in the handler.
* **`/api/v1/pose/zones/summary` invented four zones** (`zone_1..4`)
  marked `clear`, even though no zone configuration exists on this
  deployment.
* **`/api/v1/info.features.pose_estimation` was permanently `true`**
  regardless of whether a model was actually loaded.
* **`SignalField` (the 20×20 room-heatmap in WS payload) was
  procedurally generated** by mapping subcarrier index `k` to angle
  `2π·k/N` and dropping Gaussian hotspots at radius proportional to
  variance. A single sensor has no directional information — the
  resulting heatmap had no correspondence to where anything actually
  was in the room. UI rendered a believable spatial visual that was
  entirely a fiction.

All five were cosmetic noise hiding the real capability gap. Operator
asked for boots-on-the-ground honesty: surface real ESP32-derived
state and nothing else.

## Decisions

### D1 — `derive_pose_from_sensing` returns empty

The function body is now `Vec::new()`. The legacy heuristic
(`derive_single_person_pose` + bone-length tables) is unreachable
from production paths but left in the source for the day a real
trained pose model is wired in. All call sites compile unchanged
and just get an empty vector when there is no model.

### D2 — `/api/v1/pose/current` gated on `model_loaded`

```rust
let persons = if s.model_loaded {
    s.latest_update.as_ref().and_then(|u| u.persons.clone()).unwrap_or_default()
} else {
    Vec::new()
};
```

Response now includes `"model_loaded": false` so the UI can decide
whether to render a placeholder ("No pose model loaded") or hide the
panel entirely.

### D3 — `/api/v1/pose/stats` drops the fake confidence

The hard-coded `"average_confidence": 0.87` is removed. Only
counters that come from real frame ingest remain
(`total_detections`, `frames_processed`) plus `model_loaded`.

### D4 — `/api/v1/pose/zones/summary` reports actual zone state

```json
{ "presence": <real>, "zones_configured": 0, "zones": {} }
```

No more invented `zone_1..4`. When the operator configures real
zones (open work), they get added here.

### D5 — `/api/v1/info.features.pose_estimation` reflects reality

```rust
"pose_estimation": s.model_loaded,
```

### D6 — `generate_signal_field` returns zero-filled grid

The body is now:

```rust
let grid = 20usize;
return SignalField {
    grid_size: [grid, 1, grid],
    values: vec![0.0; grid * grid],
};
```

UI renders blank instead of a synthesised spatial map. This is the
truthful state until a real multistatic localizer is wired (per
ADR-008 multi-AP attention or the `MultistaticFuser` already in
state). 77 lines of procedural-art code deleted.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - fn api_info               (D5)
  - fn pose_current           (D2)
  - fn pose_stats             (D3)
  - fn pose_zones_summary     (D4)
  - fn derive_pose_from_sensing (D1)
  - fn generate_signal_field  (D6)
docs/adr/ADR-105-no-synthetic-data-in-production-runtime.md  (this)
```

Two commits:

* `9aa027e9` — D1..D5 (REST handlers + `derive_pose_from_sensing`)
* `30244d27` — D6 (`generate_signal_field` stub)

## Verified Acceptance

`/api/v1/sensing/latest` snapshot, deployment idle:

```
signal_field          grid=[20,1,20], 400 values, 0 non-zero        (was: random hotspots)
pose_keypoints        null                                          (was: 17-point heuristic)
persons               null                                          (was: synthesised array)
posture               null                                          (was: heuristic string)
signal_quality_score  null
enhanced_motion       null
vital_signs.br_bpm    null (smoothed_br ≤ 1.0)
vital_signs.hr_bpm    null

— still real —
features.mean_rssi    -59 dBm                                       ✓
features.variance     8.64                                          ✓
classification        absent / present_still / present_moving / active per ADR-101
```

`/api/v1/pose/current`:

```json
{"persons": [], "total_persons": 0, "model_loaded": false, "source": "esp32"}
```

`/api/v1/info`:

```json
{"features": {..., "pose_estimation": false, ...}}
```

## Out of scope (already correct or developer-mode)

* `--source simulate` already exits with code 2 (parallel agent change).
* `--pretrain` / `--train` synthetic-fallback paths are explicit
  dev-mode CLI flags. They never touch the runtime sensing path and
  are out of scope for this ADR.
* `vital_signs` was already gated: `breathing_rate_bpm = Some(_)` only
  when smoothed value > 1.0 BPM; otherwise `None`. No spurious BPM
  reported.
* `enhanced_motion` / `enhanced_breathing` / `bssid_count` come from
  `pipeline.process(&multi_ap_frame)` which consumes real CSI. When
  the multi-BSSID pipeline is inactive they are `None`. Left alone.

## Open Items

* **UI badges for "no model"** — `raw.html` already renders correctly
  on empty pose data; the richer Docker UI still tries to render a
  skeleton from `pose_current` even when the array is empty. Need
  a small UI patch: hide the pose canvas when `model_loaded == false`.
* **Real signal_field** via multistatic fusion — when ≥ 2 nodes are
  active, `MultistaticFuser` can produce a physically meaningful
  spatial map. ADR-104 will cover wiring it through.

## Closed

* **Honest `enhanced_*` fields** — both `enhanced_motion` and
  `enhanced_breathing` now carry a uniform `n_aps_used: u8` field
  alongside the legacy `contributing_bssids` / `bssid_count`
  counts. Consumers can gate on `n_aps_used >= 2` before trusting a
  multi-AP enhancement. (commit 598a4b2f)

## References

* ADR-101 — classifier (only emits real-derived `motion_level`).
* ADR-103 — persistent baseline (only emits real-derived
  baseline/threshold).
* [`docs/references/espectre-gap-analysis.md`](../references/espectre-gap-analysis.md)
  — separate item list for what would replace each of the now-empty
  outputs with real data.
