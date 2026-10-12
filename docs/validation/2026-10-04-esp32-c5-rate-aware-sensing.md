# ESP32 C5 rate aware sensing qualification (bring-up)

Status: **bring-up record** — device-side rate + stability qualified on 2.4 GHz;
the full 300 s controlled run, the end-to-end WebSocket table, and the 5 GHz HE
pass are PENDING. Mirrors the C6 record
[`2026-08-31-esp32-c6-rate-aware-sensing.md`](2026-08-31-esp32-c6-rate-aware-sensing.md).
See [ADR-383](../adr/ADR-383-esp32-c5-firmware-extension.md).

## Scope

Qualifies, on real ESP32-C5 silicon: that the firmware builds, flashes, boots,
associates to WiFi, captures CSI, and sustains a raw CSI callback rate above the
hardware acceptance floor without errors or reboots. Does **not** yet qualify
heartbeat, respiration, gesture, pose, identity or person-count accuracy against
labelled ground truth, the 5 GHz HE path, or the end-to-end fused WebSocket
coverage (those are P4/P5 in ADR-383).

## Hardware and firmware

| Field | Measured value |
|-------|----------------|
| Board | ESP32-C5-WROOM-1 revision v1.0, 16 MB flash, no PSRAM |
| Logical node | 1 |
| Firmware after | 0.8.12 + ESP32-C5 extension (branch `feat/esp32c5-firmware`) |
| Toolchain | ESP-IDF v5.5 (`esp32c5` preview target) |
| C5 app image | 1,151,440 bytes |
| C5 app SHA 256 | `3577a9082d8de4703b8e0830ad70d65f65f6c04f9f696d74250574cd423a1281` |
| C5 bootloader SHA 256 | `a523d2c877fe719e4f780a3f76ab740800187524fbf6daabe427320ee4c4ecf5` |
| OTA slot size | 4,194,304 bytes (one of two 4 MB slots, 16 MB layout) |
| OTA headroom | 3,042,864 bytes, 73 percent |
| Live partition | `ota_1`, `ota_state: valid` (self-marked valid, no rollback) |

## Software gates

| Gate | Result |
|------|--------|
| ESP32-C5 IDF 5.5 build | PASS (`Project build complete`) |
| Flash + image hash verify on C5 | PASS (`Hash of data verified`, all four segments) |
| Boot + onboarding on C5 | PASS (`ESP32-C5 CSI Node` banner, CSI collector + serial onboarding up) |
| NVS provisioning on C5 | PASS (`provision.py --no-stub` fix; WiFi + aggregator config written) |
| Host unit tests (rate/occupancy, ADR-110 encoding, mmWave predicate) | NOT RUN this session |
| ESP32-C6 / S3 cross-compile | NOT RE-RUN this session (gates generalized, not rebuilt for C6/S3) |

## Bring-up finding — TWT lockup

With `CONFIG_C6_TWT_ENABLE=y` (the C6 default, inherited), the C5 hit a hard
`CPU_LOCKUP` roughly one second after boot, immediately after the WiFi driver
logged `Connected AP does not support setup individual TWT agreement`. TWT
negotiation locks up the C5 *preview* WiFi driver against a non-iTWT AP. Fixed by
`CONFIG_C6_TWT_ENABLE=n` for C5 (TWT is a power feature, irrelevant to CSI). With
TWT disabled the node runs indefinitely stable.

## Device-side result (2.4 GHz, bring-up windows)

Measured from the firmware's own CSI callback counter (serial) and the raw CSI
UDP stream to the aggregator (UDP port 5006). Windows are 6–26 s, not the
full 300 s — a controlled 300 s run is pending.

| Device observation | Result |
|---------------------|--------|
| Node IP (DHCP) | assigned by the LAN (omitted) |
| AP channel (auto-detected) | 5 (2.4 GHz) |
| Uptime at measurement | ~14 min, continuous |
| Raw callback mean | **32.7 pps** (20 s UDP), 34.9 pps cumulative (cb #29200 / 837 s) |
| Raw callback range | median 34 pps, per-second up to 39 pps |
| CSI frame size | 32 through **632 bytes** (256-bin HE width — not 64-bin HT) |
| Edge DSP cadence | 8 Hz configured (actual DSP-Hz measurement pending) |
| ENOMEM / UDP send-fail / watchdog / reboot | 0 observed |

## End-to-end WebSocket result

PENDING — requires the full sensing-server/aggregator that serves `/api/v1/mesh`
and `/api/v1/fusion` (the minimal `scripts/ruview-sensing-server.py` does not).

## 5 GHz HE result (P4)

PENDING — re-provision onto a UNII-1 non-DFS channel (36/40/44) and confirm the
HE frame remains 256-bin at 5 GHz. NOTE: 256-bin HE frames (up to 632 B) are
already observed at 2.4 GHz HE20 on IDF v5.5.0, so the ADR-110 "needs IDF ≥ 5.5.2
for HE" caveat does **not** appear to bite on this silicon/toolchain.

## Result and limitation

The ESP32-C5 CSI node captures CSI and sustains **32.7 pps raw** (≥ 20 pps
acceptance floor) with HE-width frames and zero steady-state errors on 2.4 GHz.
The TWT lockup is understood and worked around. Not yet qualified: the full 300 s
controlled run with DSP-Hz range, the end-to-end fused coverage, and 5 GHz.

## Acceptance test

Raw callback yield ≥ 20 pps: **PASS (32.7 pps)**. DSP cadence within ±1 Hz of
configured, zero steady-state ENOMEM/send-fail/watchdog/panic/reboot: **PASS on
the observed windows** (full 300 s run pending). Server parse/coverage/freshness
and 5 GHz: **PENDING**.

## 2026-10-05 qualification (two nodes, 5 GHz)

MEASURED. Two ESP32-C5-WROOM-1 boards (revision v1.0, 16 MB flash, 8 MB in-package PSRAM), logical nodes 6 and 7, ESP-IDF v5.5.2, branch `feat/esp32c5-firmware` with the GPIO17/18 flash-bus fix. Both joined the AP on channel 40 (5200 MHz, 11ax). The 5-minute run used the same app with the console routed to UART0 (local overlay, app SHA 256 `91954e3c89f27fce7948e511256931b3096425a8f61012d60697bdf62c9dde85`) so device counters were readable through the board's UART bridge; the committed image logs to USB-Serial-JTAG instead. The server was sensing-server built from `fix/multinode-loop-freeze-upstream` (PR #2157), UDP 5006, source allowlist on.

| Gate (pass bar) | Node 6 | Node 7 |
|---|---|---|
| Raw callback yield (>= 20 pps) | 42.3 pps (`yield` mean 41.6) | 41.9 pps (`yield` mean 41.2) |
| Edge DSP cadence (8 Hz +/- 1) | 8.0-8.1 Hz | 8.0-8.2 Hz |
| ENOMEM / send fail / panic / watchdog / lockup | 0 / 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 / 0 |
| Resets in run | 1 (power-on) | 1 (power-on) |
| Server frames received | 8238 (27.5 fps) | 8221 (27.4 fps) |
| Server parse failures, allowlist drops | 0, 0 | 0, 0 |

Server `/health` reported `processing.state = live` in 58 of 60 samples; the two `idle` samples were before the nodes restarted when the console ports opened. Result: **PASS** for raw yield, DSP cadence, device stability and server parse/freshness. The minimum `yield` sample (5 pps) is the boot interval.

That run used `acquire_csi_force_lltf = 1`, so frames carried 53 bins. After setting it to 0 (same day, committed image with USB-JTAG console, app SHA 256 prefix `c0db522ed08c4ab8`), a 75 s capture gave 2133 and 2166 CSI frames (about 28.5 fps per node), with 97-98 % HE-SU frames of **245 bins** (490 B I/Q) and the rest legacy 53 / HT 57 bins. A 40 s server run parsed all of them, locked both nodes' grid gate on 245, and logged no warnings.

### 5-minute re-run on the 245-bin build

MEASURED, same two nodes and setup, app with `force_lltf = 0` and the console on UART0 (app SHA 256 `f064e8eb30e0256e08d4bb67273aac00fbc597d3e95c963037ac563155fd0db4`).

| Gate (pass bar) | Node 6 | Node 7 |
|---|---|---|
| Raw callback yield (>= 20 pps) | 40.3 pps (`yield` mean 39.6) | 39.6 pps (`yield` mean 39.0) |
| `yield` samples below 20 pps | 5 of 304 | 4 of 304 |
| Edge DSP cadence (8 Hz +/- 1) | 8.0-8.2 Hz | 8.0-8.1 Hz |
| ENOMEM / send fail / panic / watchdog / lockup | 0 / 0 / 0 / 0 / 0 | 0 / 0 / 0 / 0 / 0 |
| Resets in run | 1 (power-on) | 1 (power-on) |
| Server frames received | 8461 (28.2 fps) | 8067 (26.9 fps) |
| 245-bin HE-SU share | 98.0 % | 97.9 % |
| Server parse failures, allowlist drops | 0, 0 | 0, 0 |

The low `yield` samples are one dip of about 4 s that hit both nodes at the same time (about 142-146 s into the run), plus one 19 pps sample on node 6. A simultaneous dip on two boards points at the AP or the channel rather than the node; the cause was not investigated. The CSI callback length on both nodes was 490 B (245 bins).

A separate 5-minute server-only run on the same build saw a different mix: 77 % of frames were 245-bin and about 22 % were 53-bin legacy frames, with the share moving between 57 % and 94 % per 500-frame window. The mix follows the traffic on the channel. The server's majority grid gate stayed on 245 in both runs.


### 2.4 GHz qualification

MEASURED. Same nodes, app built with `CSI_WIFI_BAND_2G_ONLY` and the console on UART0. Both nodes joined channel 5. 5-minute run:

| Gate (pass bar) | Node 6 | Node 7 |
|---|---|---|
| Raw callback yield (>= 20 pps) | 41.7 pps (`yield` mean 40.9) | 41.6 pps (`yield` mean 40.9) |
| `yield` samples below 20 pps | 3 of 308 | 3 of 308 |
| Edge DSP cadence (8 Hz +/- 1) | 8.0-8.2 Hz | 8.0-8.2 Hz |
| ENOMEM / send fail (steady state) | 0 / 0 | 0 / 0 |
| ENOMEM / send fail (startup burst) | 6 / 5 | 6 / 5 |
| Panic / watchdog / lockup | 0 / 0 / 0 | 0 / 0 / 0 |
| Server frames received | 8692 (29.0 fps) | 8344 (27.8 fps) |
| 245-bin HE-SU share | 98.1 % | 97.9 % |
| Server parse failures, allowlist drops | 0, 0 | 0, 0 |

The ENOMEM events all fell in a burst about 300 ms after Got IP, and the sender's backoff recovered within 300 ms. The C6 record shows the same kind of startup backoff. The low `yield` samples are again one dip of about 3 s that hit both nodes at the same moment. HE-SU frames on 2.4 GHz also carry 245 bins.

The Wi-Fi driver keeps the band mode in its own NVS. A band pinned by one image stayed in force after reflashing an image built with AUTO, until the firmware was changed to set the band, AUTO included, on every boot.

### 802.15.4 time-sync

MEASURED. App built with `C6_TIMESYNC_ENABLE=y` (802.15.4 channel 26), both nodes, 3 minutes each.

- **Wi-Fi on 2.4 GHz (channel 5):** each node sent about 1750 beacons with 0 TX failures, and both reported `rx#0`.
- **Wi-Fi on 5 GHz (channel 40):** each node sent about 1750 beacons with 0 TX failures. Node 7 received nothing. Node 6 received one frame that didn't match the beacon magic.

So the C5's raw 802.15.4 RX path delivers no peer beacons. That's the same result as the C6 in #762, and it isn't a 2.4 GHz coexistence effect.

With 802.15.4 enabled, node 6 also aborted once on its first boot after each flash (`lock_acquire_generic`, a lock taken from interrupt context), then ran normally.

ESP-NOW time-sync between the same two nodes worked throughout: 1652-1678 of 1701 beacons matched. The C5 default stays `C6_TIMESYNC_ENABLE=n`.

**Root cause and retest.** The zero-RX result came from two bugs in `c6_timesync.c`:
1. RX was armed once at init. After each beacon TX the driver returns to idle, not RX, so each node listened only until its first beacon. Fixed with `esp_ieee802154_set_rx_when_idle(true)`.
2. The RX callback runs in ISR context and called `ESP_LOGI`. That takes a lock and aborts, which was the boot abort. Logging moved to the timer task.

After both fixes, two 3-minute runs on both nodes:
- **Results:** each node received 245-345 frames, and there were no aborts.
- **Wi-Fi blocked:** authentication timed out repeatedly (`auth -> init`, reason 2), and the STA only joined after about 179 s. Setting the 15.4 coex priorities to their lowest (idle = `IEEE802154_IDLE`, TX/RX = `LOW`) didn't change this.
- **Corrupted frames:** a dumped frame had the beacon's PHY length (27) and frame control (`41 88`), but its body was the first 4 bytes repeated: `1b 41 88 00 1b 41 88 00 ...`. Only 1 frame per node passed the magic check.

**Minimal reproducer.** A two-board IDF-only app using the same radio calls. 90 s to 3 min runs, ~850-1750 frames sent per board:

| Run | RX intact on A / B | Wi-Fi |
|---|---|---|
| Wi-Fi off, both boards TX | 0 / 1 (first run); 835-851 of 851 in 3 reruns, incl. IDF 5.5.4 | n/a |
| Wi-Fi off, A RX-only, B TX-only | 844 / - | n/a |
| Wi-Fi off, RX re-armed from a task after each TX | 846 / 851 of 851 | n/a |
| Wi-Fi STA on, no coex enable | 1729 / 1725 | never connected (reason 2, then 201) |
| Wi-Fi STA on, `esp_coex_wifi_i154_enable()` | 721 / 715 of 849 | Got IP in 3.6 s, 0 disconnects |

**What this shows:**
- RX itself works.
- In the first run, the driver didn't return to RX after the node's own TX despite `rx_when_idle`; a task-context `esp_ieee802154_receive()` fixed it. Three later reruns of that same case without the re-arm (two on IDF 5.5.2, one on 5.5.4) received 835-851 of 851 frames, so that failure isn't reproducible. The re-arm is kept as a defensive measure.
- Wi-Fi starvation was a missing coex enable.
- The minimal app never corrupted a frame. In RuView, that came from the beacon being built in a stack buffer that the async transmit read after it had been reused: the dumped "payload" contained RAM and register addresses (`0x4082F000`, `0x600C5090`).

**RuView with all four fixes.** `c6_timesync.c` now has RX re-armed in task context, no logging in the ISR, a static TX buffer, and coex enabled. Both nodes, 3 min, Wi-Fi auto (5 GHz ch 40):
- **Wi-Fi:** Got IP in 5.9 s.
- **15.4:** the follower received 708 beacons, all 708 matching. Leader hand-over worked at 109 s. No aborts.
- **ESP-NOW sync:** 1371-1375 of 1701 matched.
- **Cost:** the CSI callback `yield` mean over the last 60 s was 26.5 pps, against about 40 pps with 15.4 off, because 15.4 shares the radio. About 33 % of 15.4 TX attempts were refused by the coex arbiter.

802.15.4 time-sync works now but stays opt-in (`C6_TIMESYNC_ENABLE=n` by default) because of the CSI cost. ESP-NOW remains the default.

### Empty-room occupancy (uncalibrated)

MEASURED 2026-10-05. Both C5 nodes on 5 GHz channel 40, running the default image. The room was left empty, and a 540 s raw UDP capture was taken. The first 60 s are excluded.

**Device side (edge vitals packets):**

| Node | Vitals packets | `presence=false` (bar >= 30) | `presence=false` with `n_persons>0` (bar 0) | Flagged present while empty |
|---|---|---|---|---|
| 6 | 448 | 440 | 0 | 8 (1.8 %) |
| 7 | 452 | 405 | 0 | 47 (10.4 %), with 2-4 persons; motion flag set on every packet |

This **passes** the bar used in the C6 occupancy record. Node 7 still shows a real false-presence rate on an empty room.

**Server side, uncalibrated.** The same 480 s was replayed at original pacing through sensing-server from `main` (7c8aeace) and from #2132:
- **Result:** both reported `presence=true`, `present_moving` and `estimated_persons=1` in 478 of 480 one-second samples. There were 0 parse failures.
- **#2132 changed nothing here.** Its threshold fix acts near the absent/present boundary, and this signal (`motion_band_power` median about 25) sits far above it. The most likely cause is that the server's heuristic motion features don't fit C5 frames (mixed 245- and 53-bin shapes, a different amplitude scale) without calibration. That isn't proven.
- **Status: FAIL (uncalibrated).**

**Calibrated occupancy is still pending.** The server requires at least 600 s of empty-room calibration (`CALIBRATION_DURATION_S`, #1756) before its hold-out and calibrated presence evidence apply. That's longer than this capture. It needs a new empty-room session of about 20-25 min: 600 s to calibrate, then a held-out window.
