---
name: provision-node
description: Build, flash, and provision an ESP32-S3/C6 CSI node for RuView — firmware variant choice, cross-platform checksum-verified esptool flashing, NVS/WiFi/channel/MAC-filter overrides, boot-log evidence.
---

# provision-node

Bring an ESP32 sensing node online.

## 1. Pick a firmware variant

- **s3-8mb** (display build) — ESP32-S3 N16R8 / 16MB; AMOLED optional. The display-detect
  fix (#1000) means a *bare* board still captures CSI (MGMT+DATA).
- **s3-4mb** (no-display) — ESP32-S3 4MB; dual-OTA, display disabled.
- **c6** — ESP32-C6 + Seeed MR60BHA2 (60 GHz mmWave + WiFi CSI). The mmwave probe
  requires a validated MR60 header (#1107) so an empty UART never false-detects.

Prebuilt binaries: GitHub release `v0.8.1-esp32` (hardware-validated on S3 QFN56 rev v0.2).

## 2. Flash

Use a release flash bundle (or `release_bins/<variant>`, or an ESP-IDF `build/`).
The harness flashes on Windows, macOS, and Linux through `python -m esptool`:

```
npx @ruvnet/ruview doctor --group python,serial,firmware   # esptool, pyserial, ports, bundle checksums
npx @ruvnet/ruview flash-plan --port <PORT> --bundle <dir> --variant s3-8mb
npx @ruvnet/ruview flash --port <PORT> --bundle <dir> --variant s3-8mb --confirm
```

`flash` verifies SHA256SUMS, refuses a chip that does not match the variant
(`esptool chip_id`), writes 0x0 / 0x8000 / 0xf000 / 0x20000 (NVS preserved),
then captures a boot log. `hardwareValidated: true` requires CSI callbacks in
that log. Over MCP, `ruview_node_flash` needs the `hardware-write` grant and
`confirm: true`.

Building from source still uses ESP-IDF v5.4 (Docker or the Windows subprocess
flow; Git Bash/MSYS is unsupported, so strip `MSYSTEM*` env vars).

## 3. Provision

```
python firmware/esp32-csi-node/provision.py --port <PORT> \
  --ssid "<SSID>" --password "<secret>" --target-ip <server-ip> --target-port 5005
# optional ADR-060 overrides:
python firmware/esp32-csi-node/provision.py --port <PORT> --channel 6 --filter-mac AA:BB:CC:DD:EE:FF
```

Never echo or commit the WiFi password.

## 4. Confirm CSI is flowing

`ruview_node_monitor {port}` — PASS criteria: serial shows `CSI cb #...` callbacks and
(on a bare board) `CSI filter upgraded to MGMT+DATA`. No callbacks → the node isn't
capturing; do not proceed to calibration.
