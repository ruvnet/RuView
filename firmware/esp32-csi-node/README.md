# ESP32-S3 CSI Node Firmware

**Turn a $7 microcontroller into a privacy-first human sensing node.**

This firmware captures WiFi Channel State Information (CSI) from an ESP32-S3 and transforms it into real-time presence detection, vital sign monitoring, and programmable sensing -- all without cameras or wearables. Part of the [WiFi-DensePose](../../README.md) project.

[![ESP-IDF v5.2](https://img.shields.io/badge/ESP--IDF-v5.2-blue.svg)](https://docs.espressif.com/projects/esp-idf/en/v5.2/)
[![Target: ESP32-S3](https://img.shields.io/badge/target-ESP32--S3-purple.svg)](https://www.espressif.com/en/products/socs/esp32-s3)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-green.svg)](../../LICENSE)
[![Binary: ~943 KB](https://img.shields.io/badge/binary-~943%20KB-orange.svg)](#memory-budget)
[![CI: Docker Build](https://img.shields.io/badge/CI-Docker%20Build-brightgreen.svg)](../../.github/workflows/firmware-ci.yml)

> | Capability | Method | Performance |
> |------------|--------|-------------|
> | **CSI streaming** | Per-subcarrier I/Q capture over UDP | ~20 Hz, ADR-018 binary format |
> | **Breathing detection** | Bandpass 0.1-0.5 Hz, zero-crossing BPM | 6-30 BPM |
> | **Heart rate** | Bandpass 0.8-2.0 Hz, zero-crossing BPM | 40-120 BPM |
> | **Presence sensing** | Phase variance + adaptive calibration | < 1 ms latency |
> | **Fall detection** | Phase acceleration threshold | Configurable sensitivity |
> | **Programmable sensing** | WASM modules loaded over HTTP | Hot-swap, no reflash |

---

## Quick Start

For users who want to get running fast. Detailed explanations follow in later sections.

### 1. Build (Docker -- the only reliable method)

```bash
# From the repository root:
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32s3 && idf.py build"
```

### 2. Flash

```bash
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write_flash --flash_mode dio --flash_size 8MB \
  0x0 firmware/esp32-csi-node/build/bootloader/bootloader.bin \
  0x8000 firmware/esp32-csi-node/build/partition_table/partition-table.bin \
  0x10000 firmware/esp32-csi-node/build/esp32-csi-node.bin
```

### 3. Provision WiFi credentials (no reflash needed)

```bash
python scripts/provision.py --port COM7 \
  --ssid "YourSSID" --password "YourPass" --target-ip 192.168.1.20
```

### 4. Start the sensing server

```bash
cargo run -p wifi-densepose-sensing-server -- --http-port 3000 --source auto
```

### 5. Open the UI

Navigate to [http://localhost:3000](http://localhost:3000) in your browser.

### 6. (Optional) Upload a WASM sensing module

```bash
curl -X POST http://<ESP32_IP>:8032/wasm/upload --data-binary @gesture.rvf
curl http://<ESP32_IP>:8032/wasm/list
```

---

## Hardware Requirements

| Component | Specification | Notes |
|-----------|---------------|-------|
| **SoC** | ESP32-S3 (QFN56) | Dual-core Xtensa LX7, 240 MHz |
| **Flash** | 8 MB | ~943 KB used by firmware |
| **PSRAM** | 8 MB | 640 KB used for WASM arenas |
| **USB bridge** | Silicon Labs CP210x | Install the [CP210x driver](https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers) |
| **Recommended boards** | ESP32-S3-DevKitC-1, XIAO ESP32-S3 | Any ESP32-S3 with 8 MB flash works |
| **Deployment** | 3-6 nodes per room | Multistatic mesh for 360-degree coverage |

> **Tip:** A single node provides presence and vital signs along its line of sight. Multiple nodes (3-6) create a multistatic mesh that resolves 3D pose with <30 mm jitter and zero identity swaps.

---

## Firmware Architecture

The firmware implements a tiered processing pipeline. Each tier builds on the previous one. The active tier is selectable at compile time (Kconfig) or at runtime (NVS) without reflashing.

```
                        ESP32-S3 CSI Node
+--------------------------------------------------------------------------+
|  Core 0 (WiFi)              |  Core 1 (DSP)                             |
|                              |                                            |
|  WiFi STA + CSI callback     |  SPSC ring buffer consumer                |
|  Channel hopping (ADR-029)   |  Tier 0: Raw passthrough                  |
|  NDP injection               |  Tier 1: Phase unwrap, Welford, top-K     |
|  TDM slot management         |  Tier 2: Vitals, presence, fall detect    |
|                              |  Tier 3: WASM module dispatch             |
+--------------------------------------------------------------------------+
|  NVS config  |  OTA server (8032)  |  UDP sender  |  Power management    |
+--------------------------------------------------------------------------+
```

### Tier 0 -- Raw CSI Passthrough (Stable)

The default, production-stable baseline. Captures CSI frames from the WiFi driver and streams them over UDP in the ADR-018 binary format.

- **Magic:** `0xC5110001`
- **Rate:** ~20 Hz per channel
- **Payload:** 20-byte header + I/Q pairs (2 bytes per subcarrier per antenna)
- **Bandwidth:** ~5 KB/s per node (64 subcarriers, 1 antenna)

### Tier 1 -- Basic DSP (Stable)

Adds on-device signal conditioning to reduce bandwidth and improve signal quality.

- **Phase unwrapping** -- removes 2-pi discontinuities
- **Welford running statistics** -- incremental mean and variance per subcarrier
- **Top-K subcarrier selection** -- tracks only the K highest-variance subcarriers
- **Delta compression** -- XOR + RLE encoding reduces bandwidth by ~70%

### Tier 2 -- Full Pipeline (Stable)

Adds real-time health and safety monitoring.

- **Breathing rate** -- biquad IIR bandpass 0.1-0.5 Hz, zero-crossing BPM (6-30 BPM)
- **Heart rate** -- biquad IIR bandpass 0.8-2.0 Hz, zero-crossing BPM (40-120 BPM)
- **Presence detection** -- adaptive threshold calibration (60 s ambient learning)
- **Fall detection** -- phase acceleration exceeds configurable threshold
- **Multi-person estimation** -- subcarrier group clustering (up to 4 persons)
- **Vitals packet** -- 32-byte UDP packet at 1 Hz (magic `0xC5110002`)

### Tier 3 -- WASM Programmable Sensing (Alpha)

Turns the ESP32 from a fixed-function sensor into a programmable sensing computer. Instead of reflashing firmware to change algorithms, you upload new sensing logic as small WASM modules -- compiled from Rust, packaged in signed RVF containers.

See the [WASM Programmable Sensing](#wasm-programmable-sensing-tier-3) section for full details.

---

## Wire Protocols

All packets are sent over UDP to the configured aggregator. The magic number in the first 4 bytes identifies the packet type.

| Magic | Name | Rate | Size | Contents |
|-------|------|------|------|----------|
| `0xC5110001` | CSI Frame (ADR-018) | ~20 Hz | Variable | Raw I/Q per subcarrier per antenna |
| `0xC5110002` | Vitals Packet | 1 Hz | 32 bytes | Presence, breathing BPM, heart rate, fall flag, occupancy |
| `0xC5110004` | WASM Output | Event-driven | Variable | Custom events from WASM modules (u8 type + f32 value) |

### ADR-018 Binary Frame Format

```
Offset  Size  Field
0       4     Magic: 0xC5110001
4       1     Node ID
5       1     Number of antennas
6       2     Number of subcarriers (LE u16)
8       4     Frequency MHz (LE u32)
12      4     Sequence number (LE u32)
16      1     RSSI (i8)
17      1     Noise floor (i8)
18      2     Reserved
20      N*2   I/Q pairs (n_antennas * n_subcarriers * 2 bytes)
```

### Vitals Packet (32 bytes)

```
Offset  Size  Field
0       4     Magic: 0xC5110002
4       1     Node ID
5       1     Flags (bit0=presence, bit1=fall, bit2=motion)
6       2     Breathing rate (BPM * 100, fixed-point)
8       4     Heart rate (BPM * 10000, fixed-point)
12      1     RSSI (i8)
13      1     Number of detected persons
14      2     Reserved
16      4     Motion energy (f32)
20      4     Presence score (f32)
24      4     Timestamp (ms since boot)
28      4     Reserved
```

---

## Building

### Prerequisites

| Component | Version | Purpose |
|-----------|---------|---------|
| Docker Desktop | 28.x+ | Cross-compile firmware in ESP-IDF container |
| esptool | 5.x+ | Flash firmware to ESP32 (`pip install esptool`) |
| Python 3.10+ | 3.10+ | Provisioning script, serial monitor |
| ESP32-S3 board | -- | Target hardware |
| CP210x driver | -- | USB-UART bridge driver ([download](https://www.silabs.com/developers/usb-to-uart-bridge-vcp-drivers)) |

> **Why Docker?** ESP-IDF does NOT work from Git Bash/MSYS2 on Windows. The `idf.py` script detects the `MSYSTEM` environment variable and skips `main()`. Even removing `MSYSTEM`, the `cmd.exe` subprocess injects `doskey` aliases that break the ninja linker. Docker is the only reliable cross-platform build method.

### Build Command

```bash
# From the repository root:
MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "rm -rf build sdkconfig && idf.py set-target esp32s3 && idf.py build"
```

The `MSYS_NO_PATHCONV=1` prefix prevents Git Bash from mangling the `/project` path to `C:/Program Files/Git/project`.

**Build output:**
- `build/bootloader/bootloader.bin` -- second-stage bootloader
- `build/partition_table/partition-table.bin` -- flash partition layout
- `build/esp32-csi-node.bin` -- application firmware

### Custom Configuration

To change Kconfig settings before building:

```bash
MSYS_NO_PATHCONV=1 docker run --rm -it \
  -v "$(pwd)/firmware/esp32-csi-node:/project" -w /project \
  espressif/idf:v5.2 bash -c \
  "idf.py set-target esp32s3 && idf.py menuconfig"
```

Or create/edit `sdkconfig.defaults` before building:

```ini
CONFIG_IDF_TARGET="esp32s3"
CONFIG_ESP_WIFI_CSI_ENABLED=y
CONFIG_CSI_NODE_ID=1
CONFIG_CSI_WIFI_SSID="wifi-densepose"
CONFIG_CSI_WIFI_PASSWORD=""
CONFIG_CSI_TARGET_IP="192.168.1.100"
CONFIG_CSI_TARGET_PORT=5005
CONFIG_EDGE_TIER=2
CONFIG_WASM_MAX_MODULES=4
CONFIG_WASM_VERIFY_SIGNATURE=y
```

---

## Flashing

Find your serial port: `COM7` on Windows, `/dev/ttyUSB0` on Linux, `/dev/cu.SLAB_USBtoUART` on macOS.

```bash
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write_flash --flash_mode dio --flash_size 8MB \
  0x0 firmware/esp32-csi-node/build/bootloader/bootloader.bin \
  0x8000 firmware/esp32-csi-node/build/partition_table/partition-table.bin \
  0x10000 firmware/esp32-csi-node/build/esp32-csi-node.bin
```

### Serial Monitor

```bash
python -m serial.tools.miniterm COM7 115200
```

Expected output after boot:

```
I (321) main: ESP32-S3 CSI Node (ADR-018) -- Node ID: 1
I (345) main: WiFi STA initialized, connecting to SSID: wifi-densepose
I (1023) main: Connected to WiFi
I (1025) main: CSI streaming active -> 192.168.1.100:5005 (edge_tier=2, OTA=ready, WASM=ready)
```

---

## Runtime Configuration (NVS)

All settings can be changed at runtime via Non-Volatile Storage (NVS) without reflashing the firmware. NVS values override Kconfig defaults.

### Provisioning Script

The easiest way to write NVS settings:

```bash
python scripts/provision.py --port COM7 \
  --ssid "MyWiFi" \
  --password "MyPassword" \
  --target-ip 192.168.1.20
```

### NVS Key Reference

#### Network Settings

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `ssid` | string | `wifi-densepose` | WiFi SSID |
| `password` | string | *(empty)* | WiFi password |
| `target_ip` | string | `192.168.1.100` | Aggregator server IP address |
| `target_port` | u16 | `5005` | Aggregator UDP port |
| `node_id` | u8 | `1` | Unique node identifier (0-255) |

#### Channel Hopping and TDM (ADR-029)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `hop_count` | u8 | `1` | Number of channels to hop (1 = single-channel mode) |
| `chan_list` | blob | `[6]` | WiFi channel numbers for hopping |
| `dwell_ms` | u32 | `50` | Dwell time per channel in milliseconds |
| `tdm_slot` | u8 | `0` | This node's TDM slot index (0-based) |
| `tdm_nodes` | u8 | `1` | Total number of nodes in the TDM schedule |

#### Edge Intelligence (ADR-039)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `edge_tier` | u8 | `2` | Processing tier: 0=raw, 1=basic DSP, 2=full pipeline |
| `pres_thresh` | u16 | *auto* | Presence threshold (x1000). 0 = auto-calibrate from 60 s ambient |
| `fall_thresh` | u16 | `2000` | Fall detection threshold (x1000). 2000 = 2.0 rad/s^2 |
| `vital_win` | u16 | `256` | Phase history window depth (frames) |
| `vital_int` | u16 | `1000` | Vitals packet send interval (ms) |
| `subk_count` | u8 | `8` | Top-K subcarrier count for variance tracking |
| `power_duty` | u8 | `100` | Power duty cycle percentage (10-100). 100 = always on |

#### WASM Programmable Sensing (ADR-040)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `wasm_max` | u8 | `4` | Maximum concurrent WASM module slots (1-8) |
| `wasm_verify` | u8 | `1` | Require Ed25519 signature verification for uploads |

---

## Kconfig Menus

Three configuration menus are available via `idf.py menuconfig`:

### "CSI Node Configuration"

Basic WiFi and network settings: SSID, password, channel, node ID, aggregator IP/port.

### "Edge Intelligence (ADR-039)"

Processing tier selection, vitals interval, top-K subcarrier count, fall detection threshold, power duty cycle.

### "WASM Programmable Sensing (ADR-040)"

Maximum module slots, Ed25519 signature verification toggle, timer interval for `on_timer()` callbacks.

---

## WASM Programmable Sensing (Tier 3)

### Overview

Tier 3 turns the ESP32 from a fixed-function sensor into a programmable sensing computer. Instead of reflashing firmware to change algorithms, you upload new sensing logic as small WASM modules. These modules are:

- **Compiled from Rust** using the `wasm32-unknown-unknown` target
- **Packaged in signed RVF containers** with Ed25519 signatures
- **Uploaded over HTTP** to the running device (no physical access needed)
- **Executed per-frame** (~20 Hz) by the WASM3 interpreter after Tier 2 DSP completes

### RVF (RuVector Format)

RVF is a signed container that wraps a WASM binary with metadata for tamper detection and authenticity.

```
+------------------+-------------------+------------------+------------------+
| Header (32 B)    | Manifest (96 B)   | WASM payload     | Ed25519 sig (64B)|
+------------------+-------------------+------------------+------------------+
```

**Total overhead:** 192 bytes (32-byte header + 96-byte manifest + 64-byte signature).

| Field | Size | Contents |
|-------|------|----------|
| **Header** | 32 bytes | Magic (`RVF\x01`), format version, section sizes, flags |
| **Manifest** | 96 bytes | Module name, author, capabilities bitmask, budget request, SHA-256 build hash, event schema version |
| **WASM payload** | Variable | The compiled `.wasm` binary (max 128 KB) |
| **Signature** | 64 bytes | Ed25519 signature covering header + manifest + WASM |

### Host API

WASM modules import functions from the `"csi"` namespace to access sensor data:

| Function | Signature | Description |
|----------|-----------|-------------|
| `csi_get_phase` | `(i32) -> f32` | Phase (radians) for subcarrier index |
| `csi_get_amplitude` | `(i32) -> f32` | Amplitude for subcarrier index |
| `csi_get_variance` | `(i32) -> f32` | Running variance (Welford) for subcarrier |
| `csi_get_bpm_breathing` | `() -> f32` | Breathing rate BPM from Tier 2 |
| `csi_get_bpm_heartrate` | `() -> f32` | Heart rate BPM from Tier 2 |
| `csi_get_presence` | `() -> i32` | Presence flag (0 = empty, 1 = present) |
| `csi_get_motion_energy` | `() -> f32` | Motion energy scalar |
| `csi_get_n_persons` | `() -> i32` | Number of detected persons |
| `csi_get_timestamp` | `() -> i32` | Milliseconds since boot |
| `csi_emit_event` | `(i32, f32)` | Emit a typed event to the host (sent over UDP) |
| `csi_log` | `(i32, i32)` | Debug log from WASM (pointer + length) |
| `csi_get_phase_history` | `(i32, i32) -> i32` | Copy phase ring buffer into WASM memory |

### Module Lifecycle

Every WASM module must export these three functions:

| Export | Called | Purpose |
|--------|--------|---------|
| `on_init()` | Once, when started | Allocate state, initialize algorithms |
| `on_frame(n_subcarriers: i32)` | Per CSI frame (~20 Hz) | Process sensor data, emit events |
| `on_timer()` | At configurable interval (default 1 s) | Periodic housekeeping, aggregated output |

### HTTP Management Endpoints

All endpoints are served on **port 8032** (shared with the OTA update server).

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/wasm/upload` | Upload an RVF container or raw `.wasm` binary (max 128 KB) |
| `GET` | `/wasm/list` | List all module slots with state, telemetry, and RVF metadata |
| `POST` | `/wasm/start/:id` | Start a loaded module (calls `on_init`) |
| `POST` | `/wasm/stop/:id` | Stop a running module |
| `DELETE` | `/wasm/:id` | Unload a module and free its PSRAM arena |

### Included WASM Modules

The `wifi-densepose-wasm-edge` Rust crate provides three flagship modules:

| Module | File | Description |
|--------|------|-------------|
| **gesture** | `gesture.rs` | DTW template matching for wave, push, pull, and swipe gestures |
| **coherence** | `coherence.rs` | Phase phasor coherence monitoring with hysteresis gate |
| **adversarial** | `adversarial.rs` | Signal anomaly detection (phase jumps, flatlines, energy spikes) |

Build all modules:

```bash
cargo build -p wifi-densepose-wasm-edge --target wasm32-unknown-unknown --release
```

### Safety Features

| Protection | Detail |
|------------|--------|
| **Memory isolation** | Fixed 160 KB PSRAM arenas per slot (no heap fragmentation) |
| **Budget guard** | 10 ms per-frame default; auto-stop after 10 consecutive budget faults |
| **Signature verification** | Ed25519 enabled by default; disable with `wasm_verify=0` in NVS for development |
| **Hash verification** | SHA-256 of WASM payload checked against RVF manifest |
| **Slot limit** | Maximum 4 concurrent module slots (configurable to 8) |
| **Per-module telemetry** | Frame count, event count, mean/max execution time, budget faults |

---

## Memory Budget

| Component | SRAM | PSRAM | Flash |
|-----------|------|-------|-------|
| Base firmware (Tier 0) | ~12 KB | -- | ~820 KB |
| Tier 1-2 DSP pipeline | ~10 KB | -- | ~33 KB |
| WASM3 interpreter | ~10 KB | -- | ~100 KB |
| WASM arenas (x4 slots) | -- | 640 KB | -- |
| Host API + HTTP upload | ~3 KB | -- | ~23 KB |
| **Total** | **~35 KB** | **640 KB** | **~943 KB** |

- **PSRAM remaining:** 7.36 MB (available for future use)
- **Flash partition:** 1 MB OTA slot (6% headroom at current binary size)
- **SRAM remaining:** ~280 KB (FreeRTOS + WiFi stack uses the rest)

---

## Source Files

| File | Description |
|------|-------------|
| `main/main.c` | Application entry point: NVS init, WiFi STA, CSI collector, edge pipeline, OTA server, WASM runtime init |
| `main/csi_collector.c` / `.h` | WiFi CSI frame capture, ADR-018 binary serialization, channel hopping, NDP injection |
| `main/stream_sender.c` / `.h` | UDP socket management and packet transmission to aggregator |
| `main/nvs_config.c` / `.h` | Runtime configuration: loads Kconfig defaults, overrides from NVS |
| `main/edge_processing.c` / `.h` | Tier 0-2 DSP pipeline: SPSC ring buffer, biquad IIR filters, Welford stats, BPM extraction, presence, fall detection |
| `main/ota_update.c` / `.h` | HTTP OTA firmware update server on port 8032 |
| `main/power_mgmt.c` / `.h` | Battery-aware light sleep duty cycling |
| `main/wasm_runtime.c` / `.h` | WASM3 interpreter: module slots, host API bindings, budget guard, per-frame dispatch |
| `main/wasm_upload.c` / `.h` | HTTP endpoints for WASM module upload, list, start, stop, delete |
| `main/rvf_parser.c` / `.h` | RVF container parser: header validation, manifest extraction, SHA-256 hash verification |
| `components/wasm3/` | WASM3 interpreter library (MIT license, ~100 KB flash, ~10 KB RAM) |

---

## Architecture Diagram

```
ESP32-S3 Node                                 Host Machine
+------------------------------------------+  +---------------------------+
| Core 0 (WiFi)      Core 1 (DSP)         |  |                           |
|                                          |  |                           |
| WiFi STA --------> SPSC Ring Buffer      |  |                           |
| CSI Callback        |                    |  |                           |
| Channel Hop         v                    |  |                           |
| NDP Inject   +-- Tier 0: Raw ADR-018 ---------> UDP/5005               |
|              |   Tier 1: Phase + Welford |  |   Sensing Server          |
|              |   Tier 2: Vitals + Fall  ---------> (vitals)             |
|              |   Tier 3: WASM Dispatch  ---------> (events)             |
|              +                           |  |     |                     |
| NVS Config   OTA/WASM HTTP (port 8032)  |  |     v                     |
| Power Mgmt   POST /ota                  |  |   Web UI (:3000)          |
|              POST /wasm/upload           |  |   Pose + Vitals + Alerts  |
+------------------------------------------+  +---------------------------+
```

---

## CI/CD

The firmware is continuously verified by [`.github/workflows/firmware-ci.yml`](../../.github/workflows/firmware-ci.yml):

| Step | Check | Threshold |
|------|-------|-----------|
| **Docker build** | Full compile with ESP-IDF v5.4 container | Must succeed |
| **Binary size gate** | `esp32-csi-node.bin` file size | Must be < 950 KB |
| **Flash image integrity** | Partition table magic, bootloader presence, non-padding content | Warnings on failure |
| **Artifact upload** | Bootloader + partition table + app binary | 30-day retention |

---

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| No serial output | Wrong baud rate | Use `115200` in your serial monitor |
| WiFi won't connect | Wrong SSID/password | Re-run `provision.py` with correct credentials |
| No UDP frames received | Firewall blocking | Allow inbound UDP on port 5005 (see below) |
| `idf.py` fails on Windows | Git Bash/MSYS2 incompatibility | Use Docker -- this is the only supported build method on Windows |
| CSI callback not firing | Promiscuous mode issue | Verify `esp_wifi_set_promiscuous(true)` in `csi_collector.c` |
| WASM upload rejected | Signature verification | Disable with `wasm_verify=0` via NVS for development, or sign with Ed25519 |
| High frame drop rate | Ring buffer overflow | Reduce `edge_tier` or increase `dwell_ms` |
| Vitals readings unstable | Calibration period | Wait 60 seconds for adaptive threshold to settle |
| OTA update fails | Binary too large | Check binary is < 1 MB; current headroom is ~6% |
| Docker path error on Windows | MSYS path conversion | Prefix command with `MSYS_NO_PATHCONV=1` |

### Windows Firewall Rule

```powershell
netsh advfirewall firewall add rule name="ESP32 CSI" dir=in action=allow protocol=UDP localport=5005
```

---

## Architecture Decision Records

This firmware implements or references the following ADRs:

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-018](../../docs/adr/ADR-018-csi-binary-frame-format.md) | CSI binary frame format | Accepted |
| [ADR-029](../../docs/adr/ADR-029-ruvsense-multistatic-sensing-mode.md) | Channel hopping and TDM protocol | Accepted |
| [ADR-039](../../docs/adr/ADR-039-esp32-edge-intelligence.md) | Edge intelligence tiers 0-2 | Accepted |
| [ADR-040](../../docs/adr/) | WASM programmable sensing (Tier 3) with RVF container format | Alpha |

---

## License

This firmware is dual-licensed under [MIT](../../LICENSE-MIT) OR [Apache-2.0](../../LICENSE-APACHE), at your option.
