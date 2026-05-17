# ADR-110 — TP-Link WISP Deployment + RSSI-Δ Presence Detector

**Status**: Accepted
**Date**: 2026-05-15
**Scope**: `v2/crates/wifi-densepose-sensing-server/`,
deployment of TP-Link TL-WR841N as a dedicated CSI AP for room01/room02.

## Context

After ADR-098 made the RuView FW boot cleanly and FW5.47 fallback gave real
motion, the deployed sensors still produced unreliable presence in the
operator's home environment. Investigation revealed two compounding factors:

1. **Ambient WiFi noise.** Both sensors were associated with the main
   household AP (`Tran Thanh T3`), which is heavily used by neighbouring
   networks on the same channel. Per-frame broadband variance in an *empty*
   room measured higher than when the operator was sitting at the desk
   — the multipath geometry plus neighbour traffic dominated the CSI
   signal.
2. **The wrong feature.** Even on a clean channel, CSI variance does not
   monotonically track human presence at multi-meter range. A stationary
   body modifies multipath consistently (variance drops), while an empty
   room exhibits more multipath spread (variance rises). The host DSP
   features `variance`, `motion_band_power`, and `spectral_power` all
   showed this inversion at the deployed sensor locations.

Three one-minute measurements collected with TP-Link as the isolated AP,
sensors connected only to it:

| Feature | STILL (sitting) | WALK (room loop) | EMPTY |
|---|---|---|---|
| `variance` mean | 29.7 | 33.7 | **35.8** |
| `motion_band_power` mean | 49.8 | 54.6 | **57.4** |
| `spectral_power` mean | 161 | 172 | 172 |
| `mean_rssi` mean (dBm) | -59.13 | -59.12 | -58.98 |
| **`mean_rssi` std** | **0.60** | **1.02** | **0.35** |

Only **standard deviation of mean_rssi** monotonically separates the three
states. The human body physically perturbs RF path loss to the sensor:
absent → flat RSSI, still → small fluctuations from breathing/microtremor,
walking → large per-second swings.

## Decisions

### D1 — Isolate sensors on a dedicated AP (TP-Link TL-WR841N, WISP mode)

The household AP serves dozens of clients across multiple channels and is
constantly retransmitting management frames for neighbours and BT-coex
overlay. We deployed a TP-Link TL-WR841N in **WISP mode**:

* TP-Link associates with `Tran Thanh T3` over WiFi as a single client.
* TP-Link runs its own NAT and broadcasts a clean SSID (`TP-Link_8340`,
  WPA2-PSK, fixed channel) on the 2.4 GHz band.
* Sensors are provisioned to associate only with `TP-Link_8340`.
* TP-Link's NAT forwards their UDP/5006 packets to the Mac on the
  household subnet (Mac stays connected to `Tran Thanh T3` for internet,
  no LAN reconfiguration on the host side).

Empirical effect: per-minute broadband variance in an empty room dropped
from **50.7** (on `Tran Thanh T3`) to **35.8** (on `TP-Link_8340`).

### D2 — Replace CSI-variance presence detector with rolling RSSI MAD-Δ

The host-side classifier in `sensing-server` runs `extract_features_from_frame`
→ `smooth_and_classify` and outputs `motion_level` ∈ {`absent`, `present_still`,
`present_moving`, `active`} based on a `motion_score` derived from CSI
amplitude variance + temporal change-points. On the deployed geometry the
score crosses thresholds for body-far-from-sensor cases but not for body-near-
sensor stationary cases; the `present_still` band especially is unreliable.

We add an **RSSI-based override** layered after the existing classifier:

* Per-node rolling window of the last 120 frame RSSI samples (~10 s at
  12 Hz).
* Metric: **mean absolute delta of consecutive RSSI values** (MAD-Δ).
  This is more robust than standard deviation for the int8-quantised RSSI
  the WiFi driver reports — a single 1-dB step in a quiet window
  inflates std but contributes minimally to MAD-Δ.
* Thresholds (calibrated empirically; see D3):
  * `d < 0.20` → `absent`
  * `0.20 ≤ d < 0.55` → `present_still`
  * `0.55 ≤ d < 1.10` → `present_moving`
  * `d ≥ 1.10` → `active`
* Confidence is surfaced as the raw `d` value during the tuning phase so
  that downstream UIs (the calibration console at `static/spectrum.html`)
  can drive threshold refinement on new deployments.

The CSI-based features are preserved in the `features.*` block so that
downstream consumers (vital signs, signal-quality estimator, multi-node
fusion) continue to operate.

### D3 — Threshold calibration via UI-assisted "tell me your state" protocol

Tunable thresholds are per-deployment. The procedure documented for the
operator:

1. Open `http://localhost:8091/spectrum.html` (also reachable via Tailscale
   at the Mac's `100.x.y.z:8091`).
2. Confidence on that page shows the raw RSSI-Δ for the user's environment.
3. With a stopwatch:
   * Leave the room for 60 s. Record median `d`.
   * Sit at the workstation for 60 s. Record median `d`.
   * Walk the loop for 60 s. Record median `d`.
4. Thresholds = midpoints between consecutive medians.

For the operator's room (TP-Link AP at `192.168.1.14`, sensors at .17 / .19):

| State | `d` median (target) | `d` measured (operator) |
|---|---|---|
| absent | should be near 0 | **0.49** (empty room) |

The operator's empty-room baseline of `d ≈ 0.49` is *higher* than the
heuristic 0.20 threshold the code currently ships with. This is consistent
with the int8 quantisation: even an empty channel jitters by ±1 dB
across consecutive frames. Final threshold tuning for this deployment is
**still pending** — the captures for `sit` and `walk` are needed to set
the boundaries. The code surfaces `d` via `confidence` to let the
operator capture those next two states.

## Files Touched

```
v2/crates/wifi-densepose-sensing-server/src/main.rs   # RSSI MAD-Δ + override
v2/crates/wifi-densepose-sensing-server/static/spectrum.html  # live console
v2/crates/wifi-densepose-sensing-server/static/calibrate.html # peak-tracker view
docs/adr/ADR-110-tplink-wisp-deployment-and-rssi-presence.md  # this ADR
```

## Verified Acceptance

| Criterion | Result |
|---|---|
| Sensors associate only with TP-Link AP (no `Tran Thanh T3` direct) | ✅ |
| Mac receives UDP/5006 packets via TP-Link NAT | ✅ (~12 Hz combined) |
| Empty-room ambient noise reduced vs household AP | ✅ (variance 50.7 → 35.8) |
| `confidence` field carries raw RSSI-Δ for live tuning | ✅ |
| Vital signs (breathing 9–11 BPM) continue to populate when occupied | ✅ |

## Open Items

* Threshold final-tune (sit + walk medians not yet measured on TP-Link).
* Replace MAD-Δ with `quantile(|Δ|, 0.9) - quantile(|Δ|, 0.1)` if
  occasional packet-rate hiccups inflate the simple mean.
* The TP-Link runs WISP NAT — all sensor source IPs collapse to one
  (`192.168.1.14` on the household side). The server discriminates nodes
  by **MAC address** parsed from the `CSI_LEAN` payload, not by source IP,
  so this works today. If we later switch FW back to raw `0xC5110001`
  binary frames (which carry MAC) the same discrimination holds. If
  `parse_esp32_vitals` (0xC5110002) becomes the upstream format,
  per-node state tracking needs a separate MAC-bearing field added to
  that packet.
* On longer test sessions: the `motion_band_power` and `variance` features
  remain present in `features.*` and are useful for vital-sign signal-quality
  estimation; do not strip them.

## References

* ADR-039 — Edge intelligence pipeline (host DSP path).
* ADR-098 — Earlier ESP32-S3 deployment fixes (CSI callback, OTA, mobile UI).
* RuView issue thread on RSSI-vs-CSI presence inversion (this ADR).
