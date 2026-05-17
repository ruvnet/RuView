# ADR-107 — REST Baseline Calibration + Auto-Recalibrate

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `v2/crates/wifi-densepose-sensing-server/src/main.rs`
(`baseline_get`, `baseline_calibrate`, `auto_recalibrate_task`,
`capture_baseline_to_disk`, `BASELINE_BUS`), `static/raw.html`
(`calibrate empty` button), CLI flags
`--auto-recalibrate-quiet-sec` / `--auto-recalibrate-min-age-sec`.

## Context

ADR-103 introduced a persistent empty-room baseline at
`data/baseline.json` so the classifier no longer needed a 60 s warm-up
after every server restart. To refresh it the operator had to:

1. Step out of the room.
2. SSH / open a terminal, run `python scripts/record-baseline.py
   --duration 90`.
3. Wait for the "saved" message.
4. Restart the sensing-server (so it reloads the file).
5. Walk back in.

Steps 2, 4 are friction. The operator asked to remove them so a
fresh device that just wants to monitor a room doesn't need a CLI
or a restart. Two changes:

* **`POST /api/v1/baseline/calibrate`** — fires the same record-and-
  trim pipeline from inside the server, hot-reloads the override map
  on success. UI button in `raw.html` triggers it.
* **Auto-recalibrate background task** — silently refreshes the
  baseline when the classifier reports `absent` and CV stays low for
  a long-enough window, without any operator action.

## Decisions

### D1 — `capture_baseline_to_disk` in-process

Pure-Rust port of `scripts/record-baseline.py`:

1. Subscribe to `BASELINE_BUS` (a `tokio::sync::broadcast::Sender<String>`
   that mirrors every WS JSON message published by the broadcaster).
2. Collect `duration_sec` of per-node `(t, amplitudes, rssi)`.
3. Trim `trim_sec` from head and tail.
4. Slide `clean_window_sec` window across, pick lowest-CV chunk per
   node.
5. Compute FULL-broadband mean/p50/p95/std/CV% (same schema as
   ADR-103 v2; reload uses the same `load_baseline_file`).
6. Write `data/baseline.json` (configurable via JSON body `out`).
7. Call `load_baseline_file(path)` to hot-reload `AMP_BASELINE_OVERRIDE`
   and `AMP_BASELINE_CV`.

### D2 — `BASELINE_BUS` broadcast forwarder

Decouples baseline capture from individual WS clients. A small task
spawned at startup subscribes to `AppState.tx` and re-publishes every
message into `BASELINE_BUS`. Capture subscribers don't need a WS
connection or any external network path.

### D3 — `POST /api/v1/baseline/calibrate`

Optional JSON body: `{ duration_sec, trim_sec, clean_window_sec, out }`.
Defaults: 90 / 15 / 30 s and `data/baseline.json`. Returns immediately
with `{ "started": true, "hint": "..." }`. Subsequent calls while a
job is running return `{ "started": false, "reason": "calibration
already running" }`.

### D4 — `GET /api/v1/baseline`

```json
{
  "nodes": { "1": {"full_broadband_p95": …, "full_broadband_cv_pct": …}, … },
  "last_written_sec_ago": <i64>,
  "calibration_status": "idle" | "running" | "running (auto)"
                      | "complete" | "complete (auto)" | "error: …"
}
```

UI polls this every 2 s while a calibration is running to drive the
button state machine.

### D5 — Auto-recalibrate background task

Wakes every 5 s. State machine:

* Read latest `classification.motion_level` and `confidence` (=CV).
* `quiet = (motion_level == "absent") && (cv < 0.08)`.
* If `quiet` is true continuously for `--auto-recalibrate-quiet-sec`
  (default 1800 = 30 min) **AND** the last baseline write is older than
  `--auto-recalibrate-min-age-sec` (default 3600 = 1 h), kick off
  `capture_baseline_to_disk(90, 5, 45, "data/baseline.json")` in the
  background.
* On error, log + set `calibration_status` so the UI surfaces it.

The 30-minute / 1-hour defaults are conservative: a person briefly
walking through doesn't reset the baseline; long-term drift from
WiFi reconfiguration or furniture rearrangement does. `--auto-
recalibrate-quiet-sec 0` disables entirely.

### D6 — `raw.html` button

`calibrate empty` next to the existing `reset` button. Click →
`confirm()` reminds operator to step out → POSTs the endpoint → polls
status every 2 s, updating the inline pill `recording… 12/90 s` →
`baseline updated ✓` on success. Disables itself while running.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs
  - statics: BASELINE_LAST_WRITTEN, BASELINE_CALIBRATION_STATUS, BASELINE_BUS
  - fn capture_baseline_to_disk          (D1)
  - fn auto_recalibrate_task             (D5)
  - fn baseline_get                      (D4)
  - fn baseline_calibrate                (D3)
  - routes /api/v1/baseline + /api/v1/baseline/calibrate
  - Args { auto_recalibrate_quiet_sec, auto_recalibrate_min_age_sec }
  - main(): bus init + auto-recalibrate spawn
v2/crates/wifi-densepose-sensing-server/static/raw.html
  - <button id="calibrateBtn">             (D6)
  - <span id="calibStatus" class="pill">   (D6)
  - JS: startCalibrate(), polling loop
docs/adr/ADR-107-auto-recalibrate-and-rest-baseline.md  (this)
```

One impl commit so far: `0f373467`. UI button + ADR are in this
follow-up.

## Verified Acceptance

Boot log shows the new task wired:

```
baseline: loaded 2 node overrides from data/baseline.json
          (node1=27.04, node2=14.72; node1_cv=2.62%, node2_cv=3.65%)
Auto-recalibrate enabled: trigger after 1800s of `absent`+low-CV,
                          min 3600s between writes
CSI keepalive: 25 ICMP pkt/s/node (interval 0.040s)
```

REST endpoints live:

```
GET  /api/v1/baseline             →  current state + last_written_sec_ago
POST /api/v1/baseline/calibrate   →  { "started": true }
```

End-to-end smoke test (5 s capture window for speed):

```
POST → { started: true, duration_sec: 5 }
… 8 s elapsed …
GET  → { calibration_status: "complete", last_written_sec_ago: 13 }
file: /tmp/test_baseline.json contains n_samples=86 per node + full_broadband_*
```

The hot-reload was visible immediately: `GET /api/v1/baseline.nodes`
showed the new (capture-window) values before any server restart.

## Out of scope / open

* **UI: progress bar instead of pill text** — current state shows
  textual `recording… 12/90 s`. Could be a thin progress bar.
* **Multiple baseline profiles** — only one `data/baseline.json` per
  server. Future: name-scoped baselines for different deployment
  contexts (day / night, summer / winter).
* **Quiet detection that uses CV alone** — currently AND-gated with
  `motion_level == "absent"` which itself depends on the loaded
  baseline. Risk: if the loaded baseline is *bad*, classifier may
  never report `absent`, auto-recalibrate never fires. Mitigation:
  REST endpoint stays available; first call out of the box is always
  manual via the UI button.

## References

* ADR-100 — gain lock (the prerequisite that makes baseline meaningful).
* ADR-101 — classifier whose `motion_level`/`confidence` drives the
  quiet-detector.
* ADR-103 — persistent baseline file (this ADR adds two ways to
  refresh it).
* ADR-105 — no synthetic data (auto-recalibrate is *real* data, not
  synthesized — it just runs without operator intervention).
* ADR-106 — keepalive (ensures the capture window has enough raw CSI
  frames to give a meaningful percentile).
* [`scripts/record-baseline.py`](../../scripts/record-baseline.py)
  — original CLI workflow, kept for headless use.
