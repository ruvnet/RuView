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
| **Step 3: rank + validate** | ✅ ADR-104 D4 (commit `6212b17e`) — K ∈ {6,8,10,12,16,20} sweep, smallest-FP wins, ties by smallest total-NBVI |
| Step 4: pick top-K (K=12) | ✅ ADR-102 |
| Amplitude only (no phase) | ✅ same |

All four NBVI steps shipped. If a noisy neighbour energy-overlaps the
top-K, the validator counts FPs over the quiet window and a tighter
(or different) K wins.

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
| **Phase 2 in FW after Phase 1** | ⏳ NBVI intentionally in server as rolling refresh (adapts to slow channel drift). Not planned in FW. |
| **NVS save of gain-lock** | ✅ ADR-108 (commit `3779bb76`) — `csi_cfg/gl_agc` + `gl_fft` |
| **NVS save of NBVI selection** | ⏳ NBVI lives server-side, doesn't apply |

After ADR-108 the FW boots → CSI ready in ~0.5 s (NVS restore) instead
of ~10 s (full 300-packet calibration). Adapting to room changes
without recalibration is now a "clear NVS keys" operation — open item
ADR-108 #1 will surface that as a REST endpoint.

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
| **Gain lock NVS persistence** | ✅ ADR-108 (`csi_cfg/gl_agc` + `gl_fft`) |
| **NBVI selection NVS persistence** | ⏳ server-side rolling, intentional |
| **Baseline NVS persistence** | ✅ on host disk via ADR-103 (`data/baseline.json`); not on sensor — server is required |
| **Threshold NVS persistence** | ✅ derives from baseline_cv loaded by ADR-103 |

If we ever ship to operators who don't run the Rust server (pure FW
+ HA), the server-side bits (NBVI / baseline / threshold) would have
to migrate to the sensor's NVS. Not on the current roadmap.

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

## Priority for RuView — current state

### ✅ Done in this session

| Item | Where |
|---|---|
| NVS persistence of gain-lock | ADR-108 (`3779bb76`) |
| FP-rate validation of NBVI (Step 3) | ADR-104 D4 (`6212b17e`) |
| `POST /api/v1/baseline/calibrate` + UI button | ADR-107 (`0f373467`, `45c1464c`) |
| Auto-recalibrate on long-quiet periods | ADR-107 (`0f373467`) |
| Per-subcarrier baseline comparison | ADR-104 (`6212b17e`) |
| Full complex CSI in WS (amp+phase+meta) | ADR-106 (`4daa2c9b`) |
| Sensor µs timestamp from FW | ADR-106 (`b787f40a`) |
| Managed-ping CSI keepalive (no ручной ping) | ADR-106 (`8489efe9`) |
| No synthetic data in production runtime | ADR-105 (`9aa027e9`, `30244d27`) |
| OTA flash via WiFi (8032 port) | `ota-pipeline.md` (`274984d3`) |

### ⏳ Still open / deferred, by impact

**Updated 2026-05-17** — Most of the original "still open" items shipped
during this session. The list below is now only items that are **out
of session scope** (HA / ESPHome / Web Serial / channel hopping per
operator constraints), or items that need operator action (camera-side
training capture).

| # | Item | Net benefit | Estimate | Status |
|---|---|---|---|---|
| 1 | **HA via MQTT** | sensor as HA entity, ecosystem reach | 1 day | Deferred (operator said: no new integrations) |
| 2 | ~~Fixed-replay test suite (2 000 packets)~~ | regression protection over the classifier + NBVI | ✓ **Done** — ADR-114 (`96225e27`); F1 = 1.000 on 1000 idle + 1000 motion fixtures |
| 3 | ~~Per-sub delta sparkline in `raw.html`~~ | operator sees off-axis drift channel firing in real time | ✓ **Done** — ADR-104 (`eec3ca6c`) drift sparkline + ADR-107 D6 progress bar (`432753e1`) |
| 4 | ~~`POST /ota/recalibrate` (clear NVS gain-lock)~~ | reset gain-lock without USB after AP swap or relocation | ✓ **Done** — ADR-109 (`f92807cd`) |
| 5 | ~~Track AP MAC in NVS alongside AGC/FFT~~ | auto-invalidate stale gain-lock on AP change | ✓ **Done** — folded into ADR-109 (`gl_ap_mac` key, same commit) |
| 6 | ~~Multi-AP signal_field via `MultistaticFuser`~~ | physically real spatial map | ✓ **Done** — ADR-112 (`c8ac60f6`); 320/400 cells non-zero on two live sensors |
| 7 | ~~Per-subcarrier baseline AGE check~~ | flag for re-calibration when channel slowly drifts | ✓ **Done** — ADR-104 staleness watch (`eec3ca6c`) — warns when baseline > 14400 s AND drift > 0.15 for ≥3 ticks |
| 8 | ~~Phase-domain drift (vs amplitude-only today)~~ | sub-mm chest-wall motion detection for vitals | ✓ **Done** — ADR-104 phase channel (`47dafab4`); requires empty-room re-record to activate (`per_subcarrier_phase_mean` not in current `baseline.json` v1 schema) |
| 9 | **Tailscale-target in NVS** | sensor stream keeps working when Mac roams networks | 30 min provision + reflash | Deferred (Mac stable on TP-Link, low ROI). **Alternative shipped: ADR-115 `/ota/set-target`** lets operator repoint via REST without USB/Tailscale. |
| 10 | **ESPHome native component (instead of MQTT bridge)** | tighter HA integration than #1 | 2-3 days | Deferred (operator said: no new integrations) |
| 11 | **Web Serial calibration game** | playful threshold tuning | 1 day | Deferred (operator said: no new integrations) |
| 12 | **Boot-time NBVI freeze in FW** | trade-off vs adaptive: don't adopt unless FP issues in real homes | 2 h | Deferred (server-side rolling NBVI working; no observed FP problem) |
| 13 | **Per-channel NVS cache for gain-lock** | only needed if channel hopping (ADR-029) re-activated | 1 h | Deferred (channel hopping not active) |
| 14 | **DensePose model train + load** | unlock pose estimation | 1-3 days | **Mostly done** — model loader shipped in **ADR-116** (`7cdd8f69`) with `ruv/ruview/wiflow-v1`. Output requires per-deployment fine-tune (camera-supervised capture) — operator-side work, scoped as Pack B / Pack E. |
| 15 | **`/ota/set-target` REST** *(new this session)* | repoint CSI aggregator without USB after Mac-IP / router change | — | ✓ **Done** — ADR-115 (`7d3e0c2d`) |
| 16 | **Process-hygiene + audit follow-ups** *(new this session)* | UDP loopback filter, ping pre-reap, `/` redirect, wiflow zero-pad, lock-clone optim, sensing-tab container, test-isolation guard, ADR/CHECKLIST consistency | — | ✓ **Done** — ADR-117 (this PR) |

## References

* [`espectre-techniques.md`](espectre-techniques.md) — technique catalogue
* [`ota-pipeline.md`](ota-pipeline.md) — WiFi-OTA recipe (port 8032)
* [ADR-100](../adr/ADR-100-gain-lock-baseline-stabilization.md) — gain lock
* [ADR-101](../adr/ADR-101-raw-amplitude-classifier.md) — classifier
* [ADR-102](../adr/ADR-102-nbvi-subcarrier-selection.md) — NBVI
* [ADR-103](../adr/ADR-103-persistent-baseline.md) — baseline persistence
* [ADR-104](../adr/ADR-104-per-subcarrier-drift-presence.md) — per-sub drift + NBVI FP-validation
* [ADR-105](../adr/ADR-105-no-synthetic-data-in-production-runtime.md) — no synthetic data
* [ADR-106](../adr/ADR-106-full-complex-csi-keepalive.md) — full complex CSI + keepalive
* [ADR-107](../adr/ADR-107-auto-recalibrate-and-rest-baseline.md) — REST + auto-recalibrate
* [ADR-108](../adr/ADR-108-fw-nvs-persist-gain-lock.md) — FW NVS persist gain-lock
* Pace, *How I Turned My Wi-Fi Into a Motion Sensor — Part 2*, Dec 2025
* `francescopace/espectre` on GitHub (GPLv3)
