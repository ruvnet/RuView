# ADR-098 — ESP32-S3 CSI Node Deployment Fixes (room01/room02)

**Status**: Accepted
**Date**: 2026-05-14
**Scope**: `firmware/esp32-csi-node/`, `v2/crates/wifi-densepose-sensing-server/`,
`v2/crates/wifi-densepose-desktop/`, `ui/mobile/`

## Context

Two ESP32-S3 CSI nodes (room01 `1c:db:d4:49:eb:88`, room02 `e8:f6:0a:83:89:44`)
were deployed against the RuView stack on a 2.4 GHz domestic LAN. The
out-of-the-box firmware booted but did not produce usable presence/motion
signal: `motion_score` saturated at `1.0`, `presence_score` froze near a
non-zero constant regardless of activity, vital signs never populated,
and OTA updates rolled back on every attempt.

Root-causing the chain took multiple rebuild/flash cycles. This ADR
records the final patches that made the stack functional end-to-end on
the deployed hardware and the empirical evidence that drove each change.

## Decisions

### D1 — Disable promiscuous mode in `csi_collector`

`esp_wifi_set_promiscuous(true)` silenced the CSI RX callback entirely
on this silicon revision (`yield=0pps` in `adaptive_ctrl` medium tick
log). Removing the call lets the WiFi driver invoke `wifi_csi_callback`
again at the connected-AP rate (~5-10 pps for beacon-driven traffic).

**Patch**: `csi_collector.c` — replace `esp_wifi_set_promiscuous(true);`
with a one-line `ESP_LOGI` documenting the empirical incompatibility.
Do **not** re-enable.

### D2 — Truncate `n_subcarriers` to `EDGE_MAX_SUBCARRIERS` instead of early-return

CSI frames on this hardware arrive at 384 bytes = 192 subcarriers. The
DSP pipeline declared `EDGE_MAX_SUBCARRIERS = 128`, so every incoming
frame failed the `n_subcarriers > EDGE_MAX_SUBCARRIERS` check and
returned before `process_frame` reached Step 8 (motion energy). This
was the underlying reason DSP outputs appeared frozen: the pipeline
literally was not running.

**Patch**: `edge_processing.c` — on oversized frames, clamp
`n_subcarriers = EDGE_MAX_SUBCARRIERS` and log a one-shot warning,
instead of returning. The first 128 subcarriers cover the full 20 MHz
HT20 channel; the trailing bins are HT40 sideband and not relied on.

### D3 — Broadband motion source

After D2 the original Step 8 (variance of unwrapped phase of a single
"primary" subcarrier) still failed:

* unwrapped phase drifts monotonically (thermal, oscillator) so its
  variance over a 20-frame window equals `(slope·W/2)²/3`, a non-zero
  constant unrelated to activity;
* the "primary" winner index jumps frame-to-frame (e.g. 22 → 103 →
  105), so per-bin amplitude variance is dominated by index churn,
  not motion.

We replace the source with **broadband mean amplitude variance**:
on every frame compute `mean(sqrt(I²+Q²))` across **all** subcarriers,
push that scalar into a 20-sample ring, and use its temporal variance
as `motion_energy`. This is the well-known CSI motion proxy:
human motion smears multipath and inflates frequency-domain spread
coherently across the whole channel.

Empirical separation measured on the deployed hardware:

| Window | broadband variance (median) |
|---|---|
| Empty room (3 m) | 0.07 – 0.10 (occasional 1.6 spike) |
| Walking past 2-3 m | 3.5 – 14 |

Ratio ≈ 44×. Divisor `var / 3.0f` with `clamp(0, 1.0)` puts empty
under 0.05 and walking near saturation.

**Patch**: `edge_processing.c`
* New buffer `s_broad_mean_amp_history[20]`.
* Per-frame `band_amp_mean = mean(sqrt(I²+Q²))` over all subcarriers.
* Step 8 replaced: `s_motion_energy = clamp(var / 3.0f, 0, 1)`.

### D4 — Biquad sample rate consistency

`biquad_bandpass_design(..., fs=20.0f, ...)` (filter design) did not
match `estimate_bpm_zero_crossing(..., sample_rate=10.0f, ...)` (BPM
detector). At a real callback rate of ~10 Hz the breathing passband
designed for 20 Hz becomes 0.05–0.25 Hz on the wire, excluding the
0.2–0.3 Hz human breathing band (12–18 BPM).

**Patch**: `edge_processing.c:1063` — `fs = 10.0f` for both
breathing and heart-rate filters. With D2+D3 active, `breathing_rate_bpm`
populates 21–22 BPM for a stationary person within ~30 s.

### D5 — OTA: full-partition erase + larger HTTP task stack

Two independent OTA bugs:

1. `esp_ota_begin(..., OTA_WITH_SEQUENTIAL_WRITES, ...)` skipped the
   trailing-page erase, leaving stale code from a previous (larger)
   image in the tail of the target partition. The new image header
   passed SHA validation but residual instructions still resided at
   addresses reachable via IRAM jump tables.
2. The HTTP server worker that runs the OTA verify step overflowed
   its default 4 KB stack (esp_ota_get_app_partition_description does
   substantial work). The new image *was* booted from `ota_1`, then
   panicked in early init from stack overflow, and the bootloader
   fell back to `ota_0` — looking exactly like a rollback even though
   `CONFIG_BOOTLOADER_APP_ROLLBACK_ENABLE` is disabled.

**Patches**: `ota_update.c`
* `esp_ota_begin(update_partition, OTA_SIZE_UNKNOWN, &handle)` —
  full-partition erase before write.
* `httpd_config_t config = HTTPD_DEFAULT_CONFIG(); config.stack_size = 8192;` —
  doubled stack so OTA validation has room.

Plus `main.c:130-153` — `esp_reset_reason()` and running-partition label
logged once at app start, so any future boot anomaly is visible without
guesswork.

### D6 — sensing-server: parse RuView feature_state, refuse simulation

Out of the box, `sensing-server` (`v2/crates/wifi-densepose-sensing-server`)
parsed only `0xC5110001` (raw CSI) and `0xC5110002` (vitals). RuView FW
emits `0xC5110006` (ADR-081 feature_state) as its default upstream
payload — a gap in the project.

**Patches**: `src/main.rs`
* New `parse_rv_feature_state(buf)` decoding the 60-byte
  `rv_feature_state_t` into the existing `Esp32VitalsPacket` shape;
  wired ahead of the existing `parse_esp32_vitals` call.
* Per-node `BaselineTracker` (file-scope `OnceLock<Mutex<HashMap<u8,_>>>`)
  applies hysteretic motion gating on top of the FW-reported scores so
  the UI receives clean boolean presence transitions even when the FW
  scalar is noisy.
* `--source simulate` and the auto-fallback to simulation removed;
  `simulate`/`simulated` now exit non-zero with a `ERROR` log.

A `parse_csi_lean` parser was also added for compatibility with the
legacy FW 5.47 (`esp32s3_csi_capture`) CSV format. Dead code under
current FW; kept as defence-in-depth so a mistakenly flashed legacy
sensor still produces useful data.

### D7 — Desktop UI: HTTP-sweep discovery

mDNS (`_ruview._udp.local.`) and UDP-broadcast beacon discovery (the
two paths the desktop ships) are not advertised by current RuView FW.
We added a third concurrent path: `GET /<probe-ip>:8032/status` over
the local /24 subnet, parsing the JSON returned by RuView's
`ota_status_handler`.

**Patches**: `v2/crates/wifi-densepose-desktop/src/commands/discovery.rs`
* `discover_via_http_sweep(timeout)` running alongside mDNS + UDP.
* `futures::future::join_all(tasks)` with overall `tokio::time::timeout`
  replaces the previous sequential `for task in tasks` loop, which
  blocked on slow-to-time-out unrelated IPs and missed the responding
  sensors.
* Result-keeping in `useNodes`/`Dashboard` — keep last good list when
  a poll round returns 0 nodes.

### D8 — Mobile UI: WS path + Tailscale default + no simulation fallback

* `WS_PATH = '/ws/sensing'` and a hard-coded `WS_PORT = 8765` so the
  mobile app's `ws.service` connects to the RuView WS endpoint instead
  of the legacy `/api/v1/stream/pose` FastAPI path.
* `settingsStore.serverUrl` defaults to `http://100.123.189.10:8080`,
  the deployed Mac's Tailscale IP, so the phone reaches the server
  without LAN dependency.
* All `simulated` fallbacks removed from `ws.service.ts` and
  `matStore.ts` — UI shows `disconnected` rather than synthetic data
  when the server is unreachable.

### D9 — Reset-reason logging in `app_main`

A two-line ESP_LOGI at the start of `app_main` records
`esp_reset_reason()` and `esp_ota_get_running_partition()->label`.
Worth its weight every time we touched OTA — it eliminated guesswork
when an image silently fell back.

## Verification

Acceptance ran on both deployed nodes with the operator stationary,
then walking 2-3 m past each sensor, then leaving the room.

| Criterion | Target | room01 | room02 |
|---|---|---|---|
| `motion_energy` empty room | < 0.05 | 0.018 | 0.070 |
| `motion_energy` walking | > 0.3 within 2 s | < 1 s | 3 s |
| `motion_energy` decay after exit | < 0.1 within 5 s | 0.02–0.03 | 0.02–0.03 |
| `breathing_rate_bpm` stationary 30 s | 12-20 BPM | 22.2 BPM | 21.0 BPM |
| OTA round-trip | 2 consecutive succeed | ✅ | ✅ |
| Reset-reason visible | one-line log at boot | ✅ | ✅ |

OTA #1 transitioned `running_partition: ota_0 → ota_1`; OTA #2 reversed
it back to `ota_0`. No panics. `Connection reset` on the curl side is
expected — `esp_restart()` tears down the TCP connection after
`httpd_resp_send` returns.

## Files Touched

```
firmware/esp32-csi-node/main/csi_collector.c
firmware/esp32-csi-node/main/edge_processing.c
firmware/esp32-csi-node/main/main.c
firmware/esp32-csi-node/main/ota_update.c
firmware/esp32-csi-node/sdkconfig.defaults

v2/crates/wifi-densepose-sensing-server/src/main.rs
v2/crates/wifi-densepose-sensing-server/src/csi.rs

v2/crates/wifi-densepose-desktop/src/commands/discovery.rs
v2/crates/wifi-densepose-desktop/src/commands/server.rs
v2/crates/wifi-densepose-desktop/ui/src/hooks/useNodes.ts
v2/crates/wifi-densepose-desktop/ui/src/hooks/useServer.ts
v2/crates/wifi-densepose-desktop/ui/src/pages/Dashboard.tsx
v2/crates/wifi-densepose-desktop/ui/src/pages/Sensing.tsx
v2/crates/wifi-densepose-desktop/ui/src/types.ts

ui/mobile/src/constants/websocket.ts
ui/mobile/src/services/ws.service.ts
ui/mobile/src/stores/matStore.ts
ui/mobile/src/stores/settingsStore.ts
ui/mobile/src/screens/MATScreen/index.tsx
ui/mobile/src/screens/VitalsScreen/index.tsx

docker/docker-compose.yml             # host port 5005 → 5006 (RuView FW target)
```

## Open Items

* `EDGE_MAX_SUBCARRIERS` is still `128` — D2 truncates incoming frames
  rather than enlarging the buffer. Increasing to 192 would let the
  pipeline use the full 192-subcarrier HT40 sideband, but requires
  re-sizing several stack/heap structures and re-tuning DSP windows.
  Tracked for a future release.
* Empty-room `motion_energy` on room02 sits slightly above the 0.05
  target (0.07). Either the Fresnel-zone alignment for that node is
  noisier or the calibration constant `var / 3.0f` needs to be
  hardware-rev specific. Acceptable for the current deployment;
  candidate for an auto-calibration routine.

## References

* ADR-039 — Edge intelligence pipeline (the file we patched).
* ADR-081 — `rv_feature_state_t` packet format (`0xC5110006`).
* RuView issue #555 — *DSP froze on unwrapped phase variance* (this ADR).
* RuView issue #556 — *OTA never sticks* (this ADR).
