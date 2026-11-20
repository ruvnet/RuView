# ADR-373: Host device access layer — ESP32, mmWave, LiDAR

- **Status**: proposed
- **Date**: 2026-09-30
- **Deciders**: RuView maintainers; acceptance pending
- **Tags**: hardware, esp32, mmwave, lidar, serial, udp, mcp, least-authority
- **Related**: ADR-018 (CSI frame), ADR-063 (mmWave fusion), ADR-320
  (sensor HAL), ADR-369 (npm operator surface), ADR-370 (flashing),
  ADR-374 (remote hosts), `integrations/iphone-lidar`

## Context

Operators and agents need to answer basic hardware questions, such as "is the
node streaming?", "is the radar talking?" and "does the LiDAR see the room?",
before calibration, fusion or training. Until now each modality had its own
tool:

- the ESP32 serial monitor;
- `scripts/mmwave_fusion_bridge.py`;
- the iPhone LiDAR web relay;
- the sensing server itself.

None of these was reachable through the npm harness, its MCP server or its
SDK.

The package must stay free of runtime dependencies (ADR-263). Node has no
serial API. Hardware access is also authority: an MCP client should not get
it by default.

## Decision

Add `src/devices/` to `@ruvnet/ruview` (0.7.0) with four read-oriented tools.

| Tool | Transport | What it returns |
|---|---|---|
| `ruview_devices_scan` | pyserial port list | ports with USB VID:PID, bridge chip, likely roles (esp32 / mmwave / rplidar), and the command that confirms each role |
| `ruview_esp32_capture` | `node:dgram` UDP, receive-only | per node: packet counts by kind (magics `0xC5110001`–`0xC5110007`), CSI rate, sequence loss, RSSI, antennas/subcarriers/frequency, latest device vitals |
| `ruview_mmwave_read` | serial | MR60BHA2 (60 GHz, 115200) and LD2410 (24 GHz, 256000) frames, checksum errors, presence, distance, device-reported heart and breathing rate |
| `ruview_lidar_read` | serial or WebSocket | RPLIDAR SCAN summary (points, revolutions, range, angular coverage), or iPhone relay depth statistics |

### Rules

1. **Firmware-identical parsing.**
   - The MR60BHA2 and LD2410 parsers are ports of
     `firmware/esp32-csi-node/main/mmwave_sensor.c`: same SOF, `~XOR` header
     and data checksums, 30-byte payload cap, and F4F3F2F1…F8F7F6F5 framing.
   - ESP32 packets are parsed as the little-endian structs in `csi_collector`
     and `edge_processing.h`.
   - Host and node therefore agree on what a valid frame is.
2. **Serial without dependencies.** A fixed Python script (the pyserial
   "pump") reads the port for a bounded time and number of bytes.
   - Port, baud, duration and command bytes are passed as argv and validated
     first: the port regex, a fixed baud set, and command bytes of at most 64
     hex bytes.
   - Output is lowercase hex, so the secret-redaction layer cannot corrupt
     binary data.
   - DTR control is best-effort and reported when a port does not support it.
3. **Receive-only network.**
   - The ESP32 capture binds a local UDP port (bind address from a fixed set,
     port ≥ 1024) and never sends to a node.
   - `EADDRINUSE` is reported as "sensing server already bound".
4. **Minimal actuation.** An RPLIDAR scan sends `A5 20` (start) and `A5 25`
   (stop), and drives DTR low (the A1 motor). It spins the motor but writes no
   configuration. Nothing else is written to any device.
5. **No raw sensor data in results.**
   - iPhone depth frames are validated with the same rules as
     `integrations/iphone-lidar/web/codec.mjs` and reduced to statistics.
   - The relay token comes only from `RUVIEW_LIDAR_TOKEN`. It is rejected in
     the URL and never echoed.
6. **Authority.**
   - All four tools require the new `device-access` MCP grant.
   - The CLI and SDK run them directly, as trusted local code.
   - Serial port names are pattern-checked in the schema and again in the
     driver. `/dev/pts/*` is deliberately excluded: it could read another
     terminal's input.
7. **Evidence.**
   - A successful read is tagged `MEASURED` as a *reading* on this host.
   - mmWave vitals are labelled as the radar's own estimates.
   - None of these outputs is an accuracy claim.

## Evidence

Development container, Linux x64, Node 22. No physical sensors were attached.

- **ESP32 UDP**: a simulated node sent 60 ADR-018 CSI frames plus 3 vitals
  packets to `127.0.0.1:5005` through `ruview esp32`.
  - Result: 1 node, CSI rate 15 Hz, zero sequence loss, vitals decoded
    (15.2 bpm / 69 bpm, SYNTHETIC).
  - Unit test on real loopback: 2 dropped sequence numbers out of 7 produce a
    loss fraction of 0.2857.
- **MR60BHA2**, emulated on a pseudo-terminal and read through real pyserial:
  - auto-detected `mr60bha2`;
  - 200 frames, 0 checksum errors;
  - presence 1.0, distance 87 cm, 14.8 / 68.5 bpm (the emulator's values,
    SYNTHETIC).
- **RPLIDAR**, emulated on a pseudo-terminal that answers `A5 20`:
  - 3564 points, 99 revolutions, full angular coverage;
  - the missing DTR support was reported.
- **iPhone LiDAR**, the real relay (`integrations/iphone-lidar/web/relay.mjs`)
  with a simulated phone:
  - 30 frames at 10 fps, median depth 1.824 m (SYNTHETIC);
  - a wrong token gives `connect_failed` with a remedy.
- **Not yet done**: real ESP32-S3/C6, MR60BHA2, LD2410, RPLIDAR and iPhone
  captures. Each modality needs one real capture before its parser is called
  hardware-validated.

## Amendment 1 (2026-10-01): e2e suite and latency

- `harness/ruview/test/e2e/devices.e2e.mjs` (`npm run test:e2e:devices`, CI
  workflow `ruview-device-e2e.yml`) automates the emulated-device runs above
  through the real CLI:
  - MR60BHA2 and RPLIDAR on pseudo-terminals, read via real pyserial;
  - an ESP32 UDP node;
  - the real iPhone relay.
- The harness still refuses `/dev/pts/*` port names. The suite links each pty
  to `/dev/ttyRUVIEWE2E<n>` with root or `sudo -n`.
- `RUVIEW_E2E_REQUIRE=1` turns missing prerequisites into failures.
- mmWave auto-detect keeps its probe frames (≤ 2 s per model) and reads only
  the remainder of the window. A 5 s request previously cost about 9 s.
- `ruview_lidar_read` (iPhone) gains `max_frames` and returns immediately on
  a refused connection.
- MEASURED wall time for the device e2e suite on the development container:
  19.6 s before these changes, 5.6 s after (concurrent cases, no redundant
  re-reads).

## Amendment 2 (2026-10-01): Realtek nodes, honest capture, live analysis

Driven by a hardware run of this branch against an RTL8721Dx board (PL2303GC,
COM10) and an ESP32-C6 (CP210x, COM16) on Windows 11:

- **Realtek RAC1/RHB1 (ADR-323).** `ruview_esp32_capture` decodes the 49-byte
  RAC1 envelope (8- and 16-bit tones, CRC-32 verified, SYNTHETIC flag kept)
  and counts RHB1 heartbeats per sender. Before this, 3,439 live RAC1 frames
  in 15 s were all reported as unknown.
- **No false success.** The capture now fails with `no_decodable_packets`
  or `heartbeat_only` when packets arrive but none decode. It previously
  returned `ok: true` with a MEASURED label for zero decoded packets.
  `heartbeatOnlySenders` names boards that are alive but deliver no CSI — the
  stall observed on a Realtek board before a reset.
- **More ESP32 packet kinds.** ADR-110 sync (`0xC511A110`) and ADR-081 mesh
  envelopes (`0xC5118100`, counted by message type, never a phantom node).
  A node sending several CSI shapes (the C6 sends 1x256, 1x64 and 1x128)
  reports each in `csiShapes` instead of only the last one.
- **Live CSI → kernel.** `--analyze` runs the busiest (or `--node-id`) node's
  dominant single-antenna shape through `@ruvnet/ruview-kernel` at the
  measured arrival rate, with a bounded frame buffer (`analyze_max_frames`,
  default 6,000). Results are signal-processing estimates with no reference.
- **Serial monitor no longer reboots nodes.** The monitor opens the port with
  DTR/RTS deasserted. MEASURED on the C6: uptime kept rising across a monitor
  run (1,612 s → 1,625 s); before, opening the port reset it (uptime ~15 s).
  It takes `--baud` (Realtek logs at 1,500,000) and explains the C6/S3
  USB-Serial/JTAG console when the UART is silent.
- **Device scan.** Prolific `067B:23A3` (PL2303GC) is classified `realtek`.
- **Windows.** The hosts file gets an owner-only ACL (`icacls`), failing
  closed; POSIX `0o600` is ignored on Windows. The symlink-escape test skips
  only where Windows refuses to create symlinks.
- **Parser speed.** Int8Array view and `sqrt` instead of per-byte reads and
  `Math.hypot`: MEASURED 0.42–0.52 M → 1.33–1.67 M packets/s (about 3.2x;
  three interleaved runs, median of seven each, Windows 11 x64, Node 24).

Hardware evidence (MEASURED, this host): C6 node 42, 45 s, 1,160 packets,
1,157 decoded, 0 sequence loss, 541 1x256 frames at 12.1 Hz analyzed by the
napi kernel (integrity verified). The RAC1 decoder is cross-checked against
the Rust `realtek-csi-sim` encoder (200/200 frames at 8- and 16-bit tones);
a live RAC1 re-run is pending reconnection of the Realtek board.

Real flash (MEASURED, same C6, COM16): `ruview flash --variant c6 --confirm`
with the checksum-verified `release_bins/c6-adr110` bundle wrote 4 images
(hash verified by esptool, NVS kept: node 42 rejoined). The boot log showed
13 CSI callbacks, the MGMT+DATA upgrade and no panic. After boot, `monitor`
saw CSI without a reset (uptime 36 s → 62 s) and the UDP capture decoded
183/183 packets with zero loss. The run exposed one more Windows bug, now
fixed: the boot-log capture wrote decoded text to a cp1252 stdout (the
scrubbed child env) and aborted on the firmware's U+2192 log character, so
`bootLog.captured` was false and an earlier abort would have hidden the CSI
evidence. It now writes raw bytes; a regression test forces cp1252.

Live RTL8721Dx re-run (MEASURED, board 4 on COM10, node 3, alongside the C6):
`devices` classified COM10 as `realtek`; a 20 s capture decoded 7,404/7,404
packets (6,959 RAC1 frames at 348 Hz, RSSI −38.9 dBm, 200 RHB1 heartbeats);
`monitor --baud 1500000` saw 62 CSI log lines and did not reset the board
(sequence kept rising, 80,856 → 88,257). `--analyze --node-id 3` ran 12,549
live frames at a measured 313.7 Hz through the WASM kernel (integrity
verified). The board also exposed a loss-accounting bug carried over from the
original code: one stray sequence value (+52,835, then straight back) was
counted as 52,834 lost frames, giving 97% "loss". The tracker now treats a
single out-of-window value as a stray, needs two agreeing frames to accept a
counter reset, and lets a late packet fill its own gap. Against ground truth
(unique sequence numbers over the span, 15 s): 4,783/4,783 present, true loss
0, 9 strays — the tracker reported loss 0 and 9 strays.

## Amendment 3 (2026-10-01): ESPHome radar kits and CSI buffer starvation

- **ESPHome source for radar.** The Seeed MR60BHA2 kit (a XIAO ESP32-C6 with
  the 60 GHz radar) runs ESPHome, which owns the radar UART. Raw frames never
  reach USB, so `mmwave --model auto` correctly returned `no_valid_frames`.
  `ruview_mmwave_read` now takes `source: "esphome"` with a `host`.
  - **Client:** a dependency-free, read-only client for the plaintext ESPHome
    native API (TCP 6053) runs hello → connect → device info → list entities
    → subscribe states for a bounded window. It never sends a command.
  - **Roles:** presence, heart, breathing, distance and target count are
    found by entity name. The result uses the same summary fields as the
    serial path.
  - **Decoding:** ESPHome's `missing_state` flag and proto3 zero defaults are
    honoured, so absent readings are excluded rather than averaged as 0.
  - **Address policy:** hosts must resolve to private, link-local, CGNAT
    (Tailscale) or loopback addresses.
  - **Unsupported cases:** encrypted (Noise) and password-protected APIs are
    reported as unsupported.
- **Live kit (MEASURED, 192.168.1.102, ESPHome 2026.9.0, project
  `seeedstudio.mr60bha2_kit` `spaces-1.0`).**
  - The kit had been configured for an out-of-range SSID. It was moved to
    the operator's network through its own captive portal, without
    reflashing.
  - A 15 s read returned 154 state updates:
    - presence: detected, 1 target;
    - mean distance: 40.0 cm;
    - device-reported heart rate: mean 75.3 bpm;
    - device-reported breathing rate: mean 10.7 bpm.
  - These are firmware-computed values, not validated against a reference.
- **Realtek heartbeat-only stall: root cause.** Reading the stalled board's
  console without a reset showed continuous `[WLAN-W] lack of csi buf!` /
  `csi buf not enough`.
  - The radio produces CSI reports, but the firmware does not return report
    buffers, so CSI stops while heartbeats continue.
  - `ruview monitor` now counts these lines and fails with
    `csi_buffer_starvation`. Observed live: 399 lines in 8 s, 0 CSI.
  - The lasting fix belongs in the RTL8721Dx firmware (outside this
    repository).
- **Views.** Radar results render in the terminal UI and in the console
  widget (`ruview_mmwave_read` is now a UI tool). Both label vitals
  "device-reported, not validated".

## Consequences

- One command per modality works across CLI, MCP and SDK, with actionable
  failures (`pyserial_missing`, `port_open_failed`, `no_valid_frames`,
  `port_in_use`, `connect_failed`).
- Python and pyserial become the serial dependency. `doctor` reports both.
- The package's unpacked-size ceiling rises to 320 KiB (from 224 KiB) in
  both npm workflows, with the published package still free of runtime
  dependencies.
- Follow-up: feed these readers into `ruview-hal` observations (ADR-320)
  for fusion.
