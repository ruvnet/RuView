# ADR-082: Pose Tracker Confirmed-Track Output Filter

| Field       | Value                                                                 |
|-------------|-----------------------------------------------------------------------|
| **Status**  | Accepted — implemented in commit landing this ADR                     |
| **Date**    | 2026-04-25                                                            |
| **Authors** | ruv                                                                   |
| **Issue**   | [#420 — "24 ghost people in the UI with 3× ESP32-S3 nodes"](https://github.com/ruvnet/RuView/issues/420) |
| **Depends** | ADR-026 (track lifecycle), ADR-024 (AETHER re-ID embeddings)          |

## Context

Multiple users running the Rust sensing server with 3 ESP32-S3 nodes have
reported the same symptom: the live UI renders 22–24 phantom skeletons that
flicker at high rate, while `GET /api/v1/sensing/latest` correctly reports
`estimated_persons: 1`. The problem is reproducible across both Docker and
native deployments and is independent of the firmware MGMT-only mitigation
shipped for #396.

The two-number contradiction (1 in the snapshot, ~24 in the WebSocket stream)
narrows the bug to the path that produces `update.persons`. That path is
`tracker_bridge::tracker_update` → `tracker_bridge::tracker_to_person_detections`
→ WebSocket frame.

### Pose tracker lifecycle (per ADR-026)

`signal::ruvsense::pose_tracker::TrackLifecycleState` has four states:

```
Tentative -> Active -> Lost -> Terminated
```

The state machine and its predicates:

| State        | `is_alive()` | `accepts_updates()` | Meaning |
|--------------|--------------|---------------------|---------|
| `Tentative`  | true         | true                | New detection, < 2 confirmed hits |
| `Active`     | true         | true                | Confirmed track, currently observed |
| `Lost`       | **true**     | false               | Confirmed track, missed `loss_misses` updates, still inside `reid_window` |
| `Terminated` | false        | false               | Removed on next `prune_terminated()` |

`PoseTracker::active_tracks()` filters by `is_alive()`, which means it returns
`Tentative ∪ Active ∪ Lost` — every track that has not yet been Terminated.

### Root cause

`crates/wifi-densepose-sensing-server/src/tracker_bridge.rs` exposes the
tracker output to the WebSocket stream via:

```rust
/// Convert active PoseTracker tracks back into server-side PersonDetection values.
///
/// Only tracks whose lifecycle `is_alive()` are included.
pub fn tracker_to_person_detections(tracker: &PoseTracker) -> Vec<PersonDetection> {
    tracker
        .active_tracks()
        .into_iter()
        .map(|track| { /* ... */ })
        .collect()
}
```

The doc comment is correct as a description of `is_alive()`, but `is_alive()`
is the wrong gate for *rendering*. `Lost` tracks have not received a
measurement in `loss_misses` ticks; they are kept around only so the
re-identification machinery can attempt to match them when a similar
detection reappears within `reid_window`. They are not currently observed and
must not appear as live skeletons in the UI.

With 3 ESP32-S3 nodes streaming CSI at ~10 Hz each, `derive_pose_from_sensing`
emits a per-node detection every tick. Detections that fall outside the
Mahalanobis gate (cost ≥ 9.0) cannot match an existing track, so a new
`Tentative` track is created and the previous one ages into `Lost`. With
`reid_window ≈ 30` ticks (~3 s at 10 Hz), up to 30 ticks × 3 nodes ≈ 90
phantom Lost tracks can co-exist before any of them reach `Terminated`.
The actually-observed-now person is one of them; the other ~22–89 are ghosts.

The snapshot endpoint `/api/v1/sensing/latest` reads `estimated_persons` from
the multistatic eigenvalue counter (`signal::ruvsense::field_model`), which
operates on the CSI data directly and reports 1. The WebSocket stream reads
`update.persons`, which is the unfiltered `is_alive()` set — hence the
22-vs-1 mismatch.

This is a documentation/implementation discrepancy in `tracker_bridge`, not a
flaw in the lifecycle state machine itself.

## Decision

Introduce a **confirmed-track filter** at the bridge boundary that returns
only tracks the UI is meant to render:

* `Active` — confirmed and currently observed; always render.
* `Tentative` — confirmed for the *current* tick (created or matched this
  cycle); render so first-frame visibility latency stays at one tick.
* `Lost` — **never** render. They exist only to support re-ID over the
  `reid_window` and have, by definition, not been observed for at least
  `loss_misses` ticks.
* `Terminated` — never render (already excluded by `is_alive()`).

### Naming

Add `PoseTracker::confirmed_tracks()` — the name reflects "tracks the system
is currently confirming a person is present at this position." Keep
`active_tracks()` unchanged so callers that legitimately need the re-ID set
(re-identification, soft-confidence overlays, debug UIs) still have it.

The bridge’s public surface stays the same; only the internal accessor
swaps. WebSocket consumers see the corrected `update.persons` automatically.

### Why include `Tentative`

A walking person’s first detection lands in `Tentative` until two consecutive
hits arrive (~0.1 s at 10 Hz). Excluding `Tentative` makes the UI
under-render by one tick on every entry; the gain (filtering out spurious
single-detection ghosts) is real but small relative to the much larger Lost
problem and isn’t worth the visible latency. If single-tick ghosts become
the dominant complaint after this ADR ships, escalate to `Active`-only and
revisit `birth_hits` calibration.

## Consequences

### Positive

* `update.persons.length` matches `estimated_persons` within ±1 (Tentative
  vs. Active hand-off frame) under steady state. #420 closed.
* No change to the lifecycle state machine, no change to `reid_window` or
  `loss_misses`, no change to the WebSocket schema. Pure filter at egress.
* `PoseTracker::active_tracks()` keeps its semantics for re-ID consumers;
  this avoids breaking ADR-024 (AETHER) call sites.

### Negative / risks

* Existing test `test_tracker_update_stable_ids` exercises three sequential
  identical-person updates and asserts the ID is stable across all three.
  Filtering Lost out doesn’t affect it (the track stays in `Tentative` →
  `Active`, never Lost during the test). Confirmed by reading the test;
  no regression expected.
* Single-tick `Tentative` exposure means very-spurious one-frame detections
  *can* still flicker briefly. Acceptable trade-off as discussed above.

### Neutral

* `prune_terminated()` and the existing transition logic
  (`predict_all` → `mark_lost` → `terminate`) are unchanged.

## Implementation

1. **`signal::ruvsense::pose_tracker`** — add:
   ```rust
   /// Tracks the UI is meant to render: Tentative + Active.
   /// Excludes Lost (re-ID candidates) and Terminated.
   pub fn confirmed_tracks(&self) -> Vec<&PoseTrack> {
       self.tracks
           .iter()
           .filter(|t| matches!(
               t.lifecycle,
               TrackLifecycleState::Tentative | TrackLifecycleState::Active
           ))
           .collect()
   }
   ```
2. **`sensing-server::tracker_bridge`** — change
   `tracker_to_person_detections` to call `tracker.confirmed_tracks()` and
   update the doc comment to describe the new contract.
3. **Regression test** in `tracker_bridge.rs::tests`:
   * Drive a track to `Active` over two updates.
   * Submit empty detections for `loss_misses + 1` predict cycles to push
     the track to `Lost`.
   * Assert `tracker_update(... empty ...)` returns an empty `Vec`.
4. **Validation**: workspace tests + ESP32-S3 on COM7 streaming round-trip.

## Validation

* `cargo test --workspace --no-default-features` — must stay green
  (≥ 1,538 passed, 0 failed; new regression test adds one).
* Live verification on ESP32 setup: WebSocket `update.persons.length`
  must equal `estimated_persons` ± 1 in steady state.

## Related

* ADR-026 — Track lifecycle state machine (this ADR doesn’t change it)
* ADR-024 — AETHER re-ID embeddings (uses `active_tracks()`, unchanged)
* PR #425 — Workspace `--no-default-features` build fix (unrelated, just
  the prior PR on this branch line)
* Issue #420 — original report
