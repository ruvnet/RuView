# ESPectre Gap Analysis (full Pace Part-2 vs. RuView as of 2026-05-17)

Companion to [`espectre-techniques.md`](espectre-techniques.md). That
doc is the technique catalogue; this one is the **what's still
missing** breakdown, structured exactly along the sections of Pace's
*How I Turned My Wi-Fi Into a Motion Sensor — Part 2*.

## Problem #1: NBVI subcarrier selection

| Pace step | Status in RuView |
|---|---|
| Formula `α·σ/μ² + (1-α)·σ/μ`, α = 0.5 | ✅ ADR-102 |
| Step 1: quiet-window finder | ✅ ADR-102 v2 |
| Step 2: 25 %-percentile dead-zone gate | ✅ ADR-102 |
| **Step 3: rank + validate (run motion detector on the calibration buffer, pick K with lowest FP rate)** | ❌ raw ranking accepted |
| Step 4: pick top-K (K=12) | ✅ ADR-102 |
| Amplitude only (no phase) | ✅ same |

Step 3 absence means a noisy WiFi neighbour with energy concentrated
on our top-12 subcarriers would still get picked. Defence: validate.

## Problem #2: Gain Lock (AGC + FFT)

✅ **All done** — ADR-100. Median over 300 packets, `MIN_SAFE_AGC=30`
skip-on-strong-signal safety, ESP32-S3/C3/C6 platform guards.

## Problem #3: Universal threshold via baseline-variance normalization

✅ **Done** — ADR-103 D3. Pace's `scale = 0.25 / baseline_variance`
implemented as `norm_cv = cv / baseline_cv` with universal gates
`3×` (moving) / `6×` (active). Falls back to absolute gates when no
calibration loaded.

## Two-phase boot calibration (~10 s total)

Pace runs both phases as a single atomic boot sequence on the device:

```
PHASE 1 (3 s)  collect AGC/FFT → median → lock
PHASE 2 (7 s)  rank subcarriers with gain locked → save top-K to NVS
```

| Phase | Status in RuView |
|---|---|
| Phase 1 in FW | ✅ ADR-100 (`csi_collector.c::rv_gain_lock_process`) |
| **Phase 2 in FW after Phase 1** | ❌ NBVI lives in the server as a rolling refresh, not a boot-time freeze |
| **NVS save of both lock + selection** | ❌ each FW boot re-calibrates gain; NBVI re-ranks every server boot |

Doing Phase 2 in FW would mean reboot → ready in 0.5 s instead of
~10 s. Trade-off: doesn't adapt to room changes without explicit
re-calibration.

## Persisted calibration (NVS on the sensor)

Pace stores **everything** the algorithm needs in NVS on first boot,
so post-reboot the sensor is back in detect mode in well under a
second:

* AGC lock value
* FFT lock value
* Selected subcarrier indices
* Baseline variance
* User-tuned threshold

| Item | Status in RuView |
|---|---|
| WiFi creds + collector IP in NVS | ✅ `csi_cfg` namespace |
| **Gain lock NVS persistence** | ❌ recomputed on every FW boot |
| **NBVI selection NVS persistence** | ❌ recomputed on every server boot |
| **Baseline NVS persistence** | partial — server persists to disk (ADR-103), not on the sensor |
| **Threshold NVS persistence** | ❌ universal threshold loaded from `data/baseline.json` server-side |

If we ever ship to operators who don't run the Rust server (pure FW
+ HA), all of these become required.

## The Game (Web Serial calibration UI)

❌ **Not done.** Pace ships a browser-based reaction game at
`espectre.dev/game` that talks to the ESP32 directly over Web Serial
API (USB-CDC). The game shows a live motion bar, lets the user tune
threshold while playing, and persists the chosen threshold to NVS.

Our closest analogue is the read-only `raw.html` calibration console
(per-node amplitude bars + RSSI traces + classification badges)
served by sensing-server on `/static/raw.html`. No interactive
threshold tuning; no Web Serial path; no game.

## Testing

| Pace ships | RuView has |
|---|---|
| 500+ unit tests | small smoke tests in some crates |
| 90 % code coverage | not tracked |
| Fixed 2 000-packet reference capture (1 000 idle + 1 000 motion) | none — we test live on the operator's deployment |
| PlatformIO + pytest + ESPHome + Codecov on every push | partial — Rust `cargo test` only; 2 parser regression tests added by parallel agent (`csi.rs:751`) |

This is the largest reliability gap. A 2 000-packet replay against
the classifier would protect against silent regressions when we
re-tune thresholds or refactor NBVI.

## Native Home Assistant integration via ESPHome

❌ **Not done.** Pace's sensor shows up in HA the moment it's
flashed — `binary_sensor.motion_<room>` entity with attributes.
ESPHome handles MQTT / native API / device discovery automatically.

RuView publishes via WebSocket and REST only; would need either an
ESPHome component, an MQTT bridge, or a custom HA integration.

## Hardware support

* Pace supports ESP32-S3, ESP32-C3, ESP32-C5, ESP32-C6. Gain-lock is
  guarded on these targets only; ESP32 + ESP32-S2 fall back to no
  gain lock.
* RuView gain-lock code has the same `#if` guard so the same
  hardware list works — but we only have hands-on test data for
  ESP32-S3.

## What Pace announces for Part 3 (not yet shipped, not yet on our
## radar either)

* Gesture recognition
* Fall detection
* Person vs. pet classification

## Priority for RuView, ranked by expected impact

| # | Item | Net benefit | Estimate |
|---|---|---|---|
| 1 | NVS persistence + boot-time NBVI freeze in FW | reboot → ready in 0.5 s instead of ~10 s; survives server outage | 3-4 h |
| 2 | FP-rate validation of NBVI Step 3 | defence against noise-source subcarrier overlap | 1 h |
| 3 | `POST /api/v1/baseline/calibrate` + button in `raw.html` | calibrate from browser instead of CLI script | 30 min |
| 4 | Auto-recalibrate on long-quiet periods | drops the manual step entirely | 1-2 h |
| 5 | HA via MQTT (lighter than full ESPHome rewrite) | sensor as HA entity | 1 day |
| 6 | Fixed-replay test suite (2 000 packets) | regression protection | 1 day |
| 7 | Per-subcarrier baseline comparison (ADR-104 draft) | off-axis presence detection | 1 h |
| 8 | Web Serial calibration game | nice-to-have | 1 day |
| 9 | ESPHome native component (instead of MQTT bridge) | tighter HA integration | 2-3 days |

## References

* [`espectre-techniques.md`](espectre-techniques.md) — technique catalogue
* [ADR-100](../adr/ADR-100-gain-lock-baseline-stabilization.md) — gain lock
* [ADR-101](../adr/ADR-101-raw-amplitude-classifier.md) — classifier
* [ADR-102](../adr/ADR-102-nbvi-subcarrier-selection.md) — NBVI
* [ADR-103](../adr/ADR-103-persistent-baseline.md) — baseline persistence
* Pace, *How I Turned My Wi-Fi Into a Motion Sensor — Part 2*, Dec 2025
* `francescopace/espectre` on GitHub (GPLv3)
