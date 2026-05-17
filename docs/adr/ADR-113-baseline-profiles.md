# ADR-113 — Multiple Baseline Profiles (Day/Night)

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`resolve_baseline_profile`, `baseline_profile_watch`,
`--baseline-profile` CLI flag). Closes the "Multiple baseline profiles"
item in CHECKLIST.

## Context

The empty-room baseline that ADR-103 / ADR-104 store in
`data/baseline.json` is captured at one point in time. The channel state
it reflects is sensitive to:

* People walking through corridors / adjacent apartments at night vs.
  day (different building-wide ambient WiFi traffic).
* AC / refrigerator compressor duty cycles (broadband noise at the
  ~Hz scale that changes per-time-of-day).
* Sunlight on building walls (~mm-scale thermal expansion changes
  multipath).

In the current deployment we observe the `absent` baseline mean shift
by ~3-5 % between 14:00 and 04:00 — small but enough to push the CV
of a stationary subcarrier across the ADR-103 threshold and trigger
false `present_still` flags overnight.

A single baseline can't model both regimes simultaneously. The lowest-
complexity fix is to keep two: a day baseline and a night baseline,
loaded at startup and hot-swapped at the day/night boundary.

## Decisions

### D1 — `--baseline-profile` selector with four modes

```
--baseline-profile {single,auto,day,night}    (default: single)
```

| Mode     | Behaviour                                                                                  |
|----------|--------------------------------------------------------------------------------------------|
| `single` | Legacy. Load `RUVIEW_BASELINE_FILE` or `data/baseline.json`. No watch task. **Default.**   |
| `auto`   | Pick day/night by local hour. Hot-reload at 07:00 / 21:00 transitions.                     |
| `day`    | Force `data/baseline.day.json`. No auto switching.                                         |
| `night`  | Force `data/baseline.night.json`. No auto switching.                                       |

Default is `single` so existing deployments don't have to migrate.
Operators opt in by recording two profiles + flipping the flag.

### D2 — Day window: 07:00–20:59 local

Hard-coded for now. The split matches the ambient-WiFi pattern in
this deployment (residential building, no commercial traffic).
Tunable in code (future ADR can parameterise if a second deployment
needs different hours), but a flag is premature parameter sprawl.

`chrono::Local::now().hour()` drives the choice — no UTC offset
arithmetic; the OS provides the local hour directly.

### D3 — Filename convention

```
data/baseline.day.json
data/baseline.night.json
data/baseline.json          (legacy / single-profile fallback)
```

Same JSON schema as ADR-103 v2 (`full_broadband_*`,
`per_subcarrier_mean`, optionally `per_subcarrier_phase_mean` per
ADR-104). The recording script and REST endpoint can write to any of
the three paths via `--out` / `out` body field — no schema change.

### D4 — Missing-file fallback to `data/baseline.json`

If a requested profile file doesn't exist (e.g., operator set
`--baseline-profile auto` but only recorded `baseline.json`), the
server logs a warning and loads the legacy single-baseline file
instead. This makes the migration path "set the flag, then start
recording per-profile baselines one at a time" — no big-bang switch.

### D5 — Hot-reload via `baseline_profile_watch`

Background task fires every 5 min, re-resolves the profile, and if the
profile tag changed (day → night or vice versa) calls
`load_baseline_file` on the new path. `load_baseline_file` already
hot-swaps in place — the per-node override maps and per-subcarrier
baselines update without touching live frame ingest.

5 min cadence means transitions land within 5 min of the schedule —
acceptable lag for a baseline whose channel-side variance is on the
~hour timescale.

A `static` `CURRENT_BASELINE_PROFILE` mutex tracks the loaded tag so
the watch avoids redundant disk reads when nothing changed.

### D6 — Watch is a no-op outside `auto`

`single`, `day`, and `night` modes don't need switching — those are
"set once at startup". The watch task logs a one-line "disabled"
message and returns immediately. Saves a tokio task slot and
suppresses log noise on the common single-profile deployment.

## Trade-offs

* **Operator has to record two baselines.** Twice the operator time
  (~5 min × 2). Unavoidable for the use case.
* **Hard-coded 07:00 / 21:00 split.** A different deployment (office,
  shift-work) would want different hours. Defer to a future ADR; for
  this deployment the residential cadence works.
* **No smooth interpolation between profiles.** At 20:59 we use day,
  at 21:00 we use night — a step transition. For amplitude/baseline
  comparison the step is fine (the classifier already smooths over
  multiple frames). A weighted blend across the transition window
  would be feasible but adds complexity for limited gain.
* **No more than two profiles.** Seasonal (summer/winter), weekday/
  weekend etc. would need either more flags or a config-file driven
  approach. Out of scope.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - --baseline-profile CLI flag (D1)
  - resolve_baseline_profile (D1, D2, D3, D4)
  - baseline_profile_file_or_fallback (D4)
  - baseline_profile_watch background task (D5, D6)
  - CURRENT_BASELINE_PROFILE static + init helper (D5)
  - startup uses resolve_baseline_profile (D1)
  - spawn baseline_profile_watch alongside other watches (D5)

docs/adr/ADR-113-baseline-profiles.md  (this)
```

## Verified Acceptance

* `cargo build --release -p wifi-densepose-sensing-server` clean.
* `cargo test --release -p wifi-densepose-sensing-server
  --no-default-features` — 326 tests pass.
* `sensing-server --help` shows the new `--baseline-profile` flag
  with the four-mode help text.
* Running with `--baseline-profile single` (default) keeps the
  existing log line `baseline-profile: starting in 'single' mode →
  data/baseline.json` and disables the watch task with `Baseline
  profile watch disabled (--baseline-profile single)`.
* Running with `--baseline-profile auto` while no `baseline.day.json`
  exists logs `baseline-profile day: file data/baseline.day.json not
  found, falling back to data/baseline.json` then proceeds.

## References

* ADR-103 — persistent baseline storage + JSON schema this ADR reuses.
* ADR-104 — per-subcarrier amplitude + phase drift; both consume
  whatever baseline the active profile loads.
* ADR-107 — `POST /api/v1/baseline/calibrate` can write into any of
  the three paths via the `out` body field, so operators can record
  each profile via the same UI button.
