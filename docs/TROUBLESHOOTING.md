# RuView Troubleshooting Guide

Known issues and fixes from the rebase-to-upstream branch (upstream #301).

---

## 1. Node not appearing in /api/v1/nodes

**Symptom:** ESP32-S3 node associates with WiFi, LED blinks, but no CSI frames arrive at the server. Node missing from `/api/v1/spatial/nodes`.

**Root cause:** After USB flash, the node enters a limping state where WiFi associates but the UDP CSI sender silently fails. The SoftAP + mDNS stack initializes but the CSI callback never fires.

**Fix:** Power cycle the node (unplug USB, wait 2s, replug). If that doesn't work, send DTR reset via serial: `python -m serial.tools.miniterm --dtr 0 COMx 115200` then Ctrl+C.

**Prevention:** Firmware 0.8.0+ includes a watchdog that detects zero CSI frames for 30s and triggers a software reset automatically. Nodes 1-10 are still on old firmware and lack this recovery (OTA-vs-BLE chicken-and-egg; see issue #6).

---

## 2. Person count stuck at 1

**Symptom:** `estimated_persons` always returns 1 regardless of how many people are in the room.

**Root cause (ADR-044):** Eight converging bugs:
1. `score_to_person_count` had a ceiling of 3
2. `fuse_multi_node_features` used `.max()` instead of sum — N identical readings collapsed to 1
3. Four `.max(1)` clamps forced minimum count to 1 even when absent
4. `field_model.estimate_occupancy` capped at `.min(3)`
5. Normalization saturated (dividing by hardcoded thresholds instead of adaptive p95)
6. No field model auto-calibration — eigenvalue path never activated
7. Vitals-path clamps were asymmetric
8. Tomography produced one blob (CC=1) so dedup gave wrong count

**Fix applied (Waves 1-3):**
- Wave 1 (`9cc5f604`): ceiling 3→10, `.max()` → sum/3 aggregation, softened `.max(1)` clamps
- Wave 2 (`306f1262`): RollingP95 adaptive normalization, field_model 30s auto-calibration, vitals clamp symmetry
- Wave 3 (`c3df375a`+`0d4bfb09`+`6ac70ddf`): CC flood-fill infrastructure, lambda 0.1→5.0, threshold 0.01→0.15, CC>1 gate

**Current state:** `estimated_persons` = 6-8 for 5 bodies (3 humans + 2 dogs). Overcounts because the sum/3 dedup factor is a guess. Tomography still produces one blob (CC=1), so the CC path doesn't activate. Runtime-configurable lambda would help tune without redeployment.

---

## 3. Heart rate / breathing rate jitter

**Symptom:** HR and BR readings jump wildly between frames. BR CV was 23.3%, HR CV was 12.9%.

**Root cause (ADR-045):** 11 ESP32 nodes each compute independent vitals. The server used last-write-wins — whichever node's UDP packet arrived last overwrote the global vitals. At ~20 fps per node, this meant vitals randomly interleaved from different vantage points every 50ms.

**Fix applied (`46fbc061`):** Best-node selection. Each node's vitals are smoothed independently via median filter + EMA. The node with the highest combined `breathing_confidence + heartbeat_confidence` is selected as authoritative. Result: BR CV 23.3% → 12.6%, HR CV 12.9% → 11.6%.

**Known limitation:** The `wifi-densepose-vitals` crate has a superior 4-stage pipeline (bandpass → Hilbert envelope → autocorrelation → peak detection) but is not yet wired into the sensing server. The current `VitalSignDetector` uses a simpler FFT approach with 4 BPM frequency resolution.

---

## 4. Signal quality shows 50% always

**Symptom:** The dashboard signal quality gauge was always stuck at ~50%.

**Root cause:** Signal quality was a hardcoded placeholder value, not derived from actual CSI data.

**Fix applied:** ADR-044 Wave 2 replaced the fake gauge with RollingP95 adaptive normalization. The UI honesty pass (`b2070ab4`) added beta tags to unvalidated metrics, replaced the fake gauge with per-node pill indicators, and surfaced the actual per-node signal data.

---

## 5. Dashboard freezes every 2-4 seconds

**Symptom:** The spatial view and dashboard would freeze, then reconnect, creating a visible stutter every 2-4 seconds.

**Root cause:** The WebSocket broadcast channel's `recv()` returned `Err(Lagged)` when a client fell behind. The server treated this as a fatal error and dropped the connection. The client immediately reconnected, creating a connect/disconnect cycle.

**Fix applied (`581daf4f`):**
- Server: `Lagged` error → `continue` (skip missed frames instead of disconnecting)
- Server: 30s ping/pong keepalive to prevent Caddy proxy idle timeouts
- Result: 154 frames over 8 seconds sustained, zero disconnects

---

## 6. OTA update crashes at 59%

**Symptom:** OTA firmware update via `/api/v1/firmware/download` progresses to ~59% then the node crashes with `StoreProhibited` on Core 1.

**Root cause:** NimBLE BLE advertising/scanning runs on Core 1. During OTA, the HTTP client also runs on Core 1. BLE and OTA compete for stack space, and the BLE scan callback triggers a memory access violation during the OTA write.

**Fix:**
1. Stop NimBLE advertising and scanning before calling `esp_https_ota_begin()`
2. Increase httpd stack from 4KB to 8KB (`CONFIG_HTTPD_MAX_REQ_HDR_LEN` and task stack)
3. Resume BLE after OTA completes or fails

**Caveat:** Nodes running old firmware (1-10) can't receive this fix via OTA because the crash happens during the OTA itself. These nodes must be USB-flashed with firmware 0.8.0+ first, then future OTA updates will work. Node 11 was USB-flashed with the watchdog firmware and can receive OTA updates.

---

## 7. Can't SSH to babycube via LAN

**Symptom:** `ssh thyhack@10.0.10.10` hangs at banner exchange. Ping works, TCP port 22 is open, but SSH never completes the handshake.

**Workaround:** Use the Tailscale IP instead:
```
ssh thyhack@100.90.238.87
```

**Not the cause:** CrowdSec. The 10.0.0.0/8 range is whitelisted in CrowdSec (`cscli decisions list` shows no active decisions for LAN IPs). The banner hang occurs before any authentication attempt, so it's not a firewall block.

**Suspected cause:** Unknown. Possibly MTU/fragmentation issue on the LAN segment, or a network stack bug in the babycube's NIC driver. The Tailscale overlay network (WireGuard UDP) bypasses whatever is causing the LAN TCP issue.

---

## 8. Right USB-C port doesn't work on some ESP32-S3 boards

**Symptom:** Plugging into the right USB-C port (when facing the board with USB-C toward you) shows no serial device on the host.

**Fix:** Use the left USB-C port. On most ESP32-S3-DevKitC boards, the left port is the USB-to-UART bridge (CP2102/CH340) used for flashing and serial monitor. The right port is the native USB (USB-JTAG) which requires different drivers and isn't used by the RuView firmware.

---

## 9. Docker Desktop on Windows drops UDP from multiple ESP32 nodes

**Symptom:** Two or more ESP32 nodes are flashed, provisioned, and visibly transmit on the network — `tcpdump`/Wireshark on the Windows host shows datagrams from every node — but inside the Docker container only one source IP arrives. `/api/v1/sensing/latest` shows a single node and the live UI freezes or only tracks one body. Reported in #374 (4-node bench) and reproduced in #386 (6-node demo, RuView v0.7.0).

**Root cause:** Docker Desktop on Windows runs the engine inside a WSL2 / Hyper-V VM. Inbound UDP from the host LAN is forwarded through `vpnkit` / `vEthernet` and the multi-source-IP datagrams are demultiplexed onto a single virtual socket. The first source-IP "wins"; subsequent unique sources are silently dropped at the VM boundary. This is a Docker Desktop limitation, not a sensing-server bug — `host.docker.internal` and `--network host` do not help (host networking is not implemented for the Linux engine on Windows).

**Fix:** Run the bundled UDP relay on the host so every forwarded datagram arrives from the same loopback source IP, which Docker passes through unchanged.

```powershell
# 1. Start the relay (PowerShell or any terminal)
python scripts/udp-relay.py --listen-port 5005 --forward-port 5006

# 2. Edit docker/docker-compose.yml — change the ESP32 UDP mapping from
#       - "5005:5005/udp"
#    to
#       - "5006:5005/udp"

# 3. Bring the stack up
docker compose -f docker/docker-compose.yml up
```

ESP32 nodes still target the host on `--target-ip <host>:5005` — no firmware re-provisioning is needed. The relay is `scripts/udp-relay.py` (stdlib only, no extra deps). Verify with `--verbose` that each node's source IP appears at least once before forwarding stabilises on a single ephemeral relay port.

**Prevention:** Linux and macOS hosts are unaffected; the relay only needs to run on Docker Desktop for Windows. If Docker Desktop ships per-source UDP forwarding (tracked at [docker/for-win#1144](https://github.com/docker/for-win/issues/1144) and related), this workaround can be retired.

**Prior art:** PR #413 (`txhno`) proposed a docs-only writeup of the same workaround; this entry supersedes it.

---

## 10. `404` on the visualization page when running sensing-server

**Symptom:** `sensing-server` starts cleanly, logs `HTTP server listening on http://localhost:3000`, but loading `http://localhost:3000/` (or `/ui/index.html`) returns `404 Not Found`. Reported in #188.

**Root cause:** The default `--ui-path ../../ui` is resolved relative to the binary's *current working directory*, not the binary location. When the binary is launched from anywhere other than `crates/wifi-densepose-sensing-server/`, the relative path doesn't reach the UI assets and Axum's static file handler returns 404.

**Fix:** Pass an absolute UI path, run the binary from the crate directory, or use the Docker image (which bundles the UI under `/app/ui`).

```bash
# Option A — absolute path (recommended for production)
sensing-server --source esp32 --udp-port 5005 --http-port 3000 \
  --ws-port 3001 --ui-path /absolute/path/to/ui

# Option B — run from the crate dir (works for local dev / cargo run)
cd v2/crates/wifi-densepose-sensing-server
cargo run -- --source esp32

# Option C — Docker (no path config needed)
docker compose -f docker/docker-compose.yml up sensing-server
```

**Prevention:** Track future work in #188 to fall back to a path resolved relative to the executable when the cwd-relative path doesn't exist, so the binary works regardless of where it's launched.

---

## 11. Boot loop on `--edge-tier 1` or `--edge-tier 2`

**Symptom:** ESP32-S3 boots normally with `--edge-tier 0`, but flashing the same firmware with `--edge-tier 1` or `2` produces a boot loop. Serial output reaches `cpu_start` and `heap_init`, then resets repeatedly. Reported in #438 against firmware `v0.4.3.1-esp32-3-g66e2fa083-dir`.

**Root cause:** Edge tiers 1 and 2 enable the on-device DSP pipeline on Core 1. In the affected build, the `edge_dsp` task ran a tight per-frame loop without yielding, so the FreeRTOS task watchdog tripped on Core 1 and panicked. Tier 0 is passthrough only and doesn't activate the pipeline, so the watchdog never fires there.

**Fix:** Flash the [v0.4.3.1-esp32](https://github.com/ruvnet/RuView/releases/tag/v0.4.3.1-esp32) release or later — the DSP task yield fixes have shipped on `main` since the build in the report.

```bash
# Verify what version you're on (look for "App version" in serial output on boot)
python -m serial.tools.miniterm COM7 115200
# Expect: "App version: v0.4.3.1-esp32" or higher
```

If the boot loop persists on a release build, capture a full serial trace including the watchdog backtrace and reopen #438 with the new build hash.
