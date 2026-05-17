# ADR-100 — PHY Gain Lock for Baseline-Stable CSI

**Status**: Accepted
**Date**: 2026-05-17
**Scope**: `firmware/esp32-csi-node/main/csi_collector.c`,
`v2/crates/wifi-densepose-sensing-server/static/raw.html`.

## Context

After ADR-099 deployed the TP-Link WISP AP and the operator captured three
controlled one-minute windows (empty / sit / walk), the RSSI MAD-Δ
classifier failed to separate the three states — measured `d` values
overlapped within ±0.03 of 0.49 while in-state spread was ±0.10. We
inspected the live amplitude spectrum on the new `raw.html` console and
saw a slow ±20-30 % broadband drift in the sensor amplitude even with
the room provably empty. The drift was indistinguishable from body
modulation at multi-meter range and dominated every downstream feature.

Francesco Pace's [ESPectre](https://github.com/francescopace/espectre)
project (GPLv3) traced the same artefact to the ESP32 PHY's automatic
gain control: AGC continuously rebalances the receiver gain per packet
so received frames stay in the optimal decoding range. For CSI sensing
this is a disaster — the same channel state arrives with a different
amplitude every packet because the gain stage shifts under it. Pace
documented two undocumented PHY routines in the IDF blob that freeze
AGC and FFT scaling, plus a calibration recipe (median of the first
300 packets) that is robust to brief startup activity.

## Decisions

### D1 — Port the ESPectre gain-lock to RuView FW

Added a self-contained block to `csi_collector.c`:

* **Overlay struct** `rv_phy_rx_ctrl_t` aliased over `wifi_csi_info_t.rx_ctrl`
  to read the hidden `agc_gain` (u8) and `fft_gain` (signed i8) fields.
* **Extern declarations** for the two PHY routines:
  ```c
  extern void phy_fft_scale_force(bool force_en, int8_t force_value);
  extern void phy_force_rx_gain(int force_en, int force_value);
  ```
* **Two-phase calibration** (`rv_gain_lock_process`):
  - Phase 1 (≤ 300 packets, ~6 s at the rate-gated 50 Hz callback):
    accumulate AGC and FFT samples into static arrays.
  - At the 300th packet: `qsort` both arrays, take the median, and
    call the two PHY routines to freeze gain.
* **Safety branch**: if median AGC < 30, skip the lock and log a
  warning. Forcing a low gain on a strong-signal deployment causes the
  RX path to freeze (empirically documented in ESPectre's
  `gain_controller.h`).
* **Supported targets**: ESP32-S3, ESP32-C3, ESP32-C6 only — older
  parts compile to a no-op stub. RuView ships on S3 so this is the only
  path we care about.

The hook is wired immediately after the existing rate-gate and MAC
filter in the CSI callback so calibration completes within the first
~6 s after the WiFi association, regardless of host traffic. After
that it short-circuits.

Tagged as ADR-100 in the source comment for traceability.

### D2 — Use the existing `raw.html` console (ADR-099, D2 reuse) as the verification UI

The console added in ADR-099 already streams `nodes[].amplitude` from
the existing WebSocket. No server-side change was needed. The HTML
displays a per-node bar histogram of all 56 active subcarriers plus
broadband mean amplitude and RSSI traces over the last 30 s. This is
the surface where the operator can watch — without any DSP, without any
classification — whether the gain-lock has actually flattened the
baseline.

### D3 — Geometry matters as much as gain-lock

A controlled three-state capture made on 2026-05-17 with both sensors
positioned so that the line `TP-Link AP → sensor` passes through the
operator (lying on the bed) confirmed both decisions. The summary
table appears under *Verified Acceptance* below. Earlier captures
(ADR-099) failed to separate states partly because the sensors were
placed off-axis from the AP-to-body line; with that geometry the body
never physically obstructs the CSI channel.

## Calibration values observed (real captures, this deployment)

| Node | Boot rate (low traffic) | Boot rate (ping flood) | AGC median | FFT scale median | Lock decision |
|---|---|---|---|---|---|
| room01 (192.168.0.101) | 0.3 fps | 30+ fps | **42–44** | −31 / −33 | **APPLIED** |
| room02 (192.168.0.100) | 0.3 fps | 30+ fps | **44** | −40 / −42 | **APPLIED** |

Both AGC medians are comfortably above the 30 safety threshold. The
calibration completes in ~6 s when there is any host traffic (a single
ping to the sensor at 10 pps is enough); on a totally idle channel
beacons drive the rate down to 0.3 fps and calibration would take ~17
minutes — practically we always have some traffic.

## Verified Acceptance — three-state separation

Geometry: TP-Link AP on the wall, both sensors at table-level on the
opposite side of the room, operator lying on the bed between AP and
sensors. 30 seconds per state, gain-lock active on both nodes,
`raw.html` open during capture, `target_ip` provisioned to the Mac's
TP-Link-side IP (192.168.0.103) so no upstream NAT is in the path.

| State | node 1 mean A | node 1 CV | node 1 sub-CV <5 % | node 2 mean A | node 2 CV | node 2 sub-CV <7 % |
|---|---|---|---|---|---|---|
| **EMPTY** (operator out) | **37.28** | **2.71 %** | **44/44** | 9.52 | 5.22 % | 26/44 |
| **STILL** (operator lying still on bed) | 22.43 | 3.70 % | 30/44 | 9.67 | 5.02 % | 24/44 |
| **WALK** (operator pacing the room) | 31.77 | **12.50 %** | 0/44 | 7.15 | **29.72 %** | 0/44 |

Observations:

* **Node 1 separates all three states** by mean amplitude alone: 37 →
  22 → 32. The body lying still blocks the direct path
  (40 % amplitude drop), then motion adds reflections back. The CV
  ladder 2.71 → 3.70 → 12.50 % is a second independent feature.
* **Node 2 separates STILL+EMPTY from WALK** by CV (5 → 30 %). Its
  geometry doesn't pick up a still body, only motion.
* **Compare to ADR-099** where empty/sit/walk differed by ±0.02 inside
  ±0.10 noise — we now have inter-state separation ratios of **×3.4 on
  node 1 and ×5.9 on node 2**. The signal is no longer dominated by
  baseline drift.

## Files Touched

```
firmware/esp32-csi-node/main/csi_collector.c       # gain-lock module + hook
v2/crates/wifi-densepose-sensing-server/static/raw.html   # already from ADR-099
docs/adr/ADR-100-gain-lock-baseline-stabilization.md      # this ADR
```

## Open Items

* ✅ **NBVI subcarrier selection** — closed in ADR-102 (server-side
  port with quiet-window finder).
* ✅ **Server-side RSSI parsing** — fixed by parallel agent in commit
  `3393c1e8` (parse_esp32_frame offset realignment + carrying RSSI
  through feature_state packets).
* ✅ **Calibration latency on an idle channel** — closed in ADR-106
  by the built-in managed-`ping` keepalive (drives sensor RX at
  25 pkt/s/node out of the box).
* ⏳ **NVS target_ip is hardcoded** — still open. Tailscale-target
  option not implemented; sensors still send to the Mac's TP-Link-
  side IP (192.168.0.103). Mac roaming still breaks the CSI stream.

## References

* ADR-039 — Edge intelligence pipeline (host DSP path).
* ADR-098 — Earlier ESP32-S3 deployment fixes.
* ADR-099 — TP-Link WISP deployment + first RSSI-Δ attempt (this ADR
  supersedes the threshold table in ADR-099, D3 — the RSSI MAD-Δ
  detector is left in place but no longer the primary signal).
* Francesco Pace, *How I Turned My Wi-Fi Into a Motion Sensor — Part 2*,
  Dec 2025 — source of the gain-lock recipe.
* `francescopace/espectre`, `components/espectre/gain_controller.{h,cpp}`
  on GitHub — reference implementation (GPLv3).
