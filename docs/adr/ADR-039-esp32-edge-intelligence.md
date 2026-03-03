# ADR-039: ESP32-S3 Edge Intelligence Pipeline

**Status**: Accepted (hardware-validated on RuView ESP32-S3)
**Date**: 2026-03-02
**Deciders**: @ruvnet

## Context

WiFi-DensePose captures Channel State Information (CSI) from ESP32-S3 nodes and streams raw I/Q data to a host server for processing. This architecture has limitations:

1. **Bandwidth**: Raw CSI at 20 Hz × 128 subcarriers × 2 bytes = ~5 KB/frame = ~100 KB/s per node. Multi-node deployments saturate low-bandwidth links.
2. **Latency**: Server-side processing adds network round-trip delay for time-critical signals like fall detection.
3. **Power**: Continuous raw streaming prevents duty-cycling for battery-powered deployments.
4. **Scalability**: Server CPU scales linearly with node count for basic signal processing that could run on the ESP32-S3's dual cores.

## Decision

Implement a tiered edge processing pipeline on the ESP32-S3 that performs signal processing locally and sends compact results:

### Tier 0 — Raw Passthrough (default, backward compatible)
No on-device processing. CSI frames streamed as-is (magic `0xC5110001`).

### Tier 1 — Basic Signal Processing
- Phase extraction and unwrapping from I/Q pairs
- Welford running variance per subcarrier
- Top-K subcarrier selection by variance
- Delta compression (XOR + RLE) for 30-50% bandwidth reduction (magic `0xC5110003`)

### Tier 2 — Full Edge Intelligence
All of Tier 1, plus:
- Biquad IIR bandpass filters: breathing (0.1-0.5 Hz), heart rate (0.8-2.0 Hz)
- Zero-crossing BPM estimation
- Presence detection with adaptive threshold calibration (1200 frames, 3-sigma)
- Fall detection (phase acceleration exceeding configurable threshold)
- Multi-person vitals via subcarrier group clustering (up to 4 persons)
- 32-byte vitals packet at configurable interval (magic `0xC5110002`)

### Architecture

```
Core 0 (WiFi)                    Core 1 (DSP)
┌─────────────────┐              ┌──────────────────────────┐
│ CSI callback     │──SPSC ring──▶│ Phase extract + unwrap   │
│ (wifi_csi_cb)    │   buffer     │ Welford variance         │
│                  │              │ Top-K selection           │
│ UDP raw stream   │              │ Biquad bandpass filters   │
│ (0xC5110001)     │              │ Zero-crossing BPM         │
└─────────────────┘              │ Presence detection        │
                                 │ Fall detection             │
                                 │ Multi-person clustering    │
                                 │ Delta compression          │
                                 │ ──▶ UDP vitals (0xC5110002)│
                                 │ ──▶ UDP compressed (0x03)  │
                                 └──────────────────────────┘
```

### Wire Protocols

**Vitals Packet (32 bytes, magic `0xC5110002`)**:

| Offset | Type | Field |
|--------|------|-------|
| 0-3 | u32 LE | Magic `0xC5110002` |
| 4 | u8 | Node ID |
| 5 | u8 | Flags (bit0=presence, bit1=fall, bit2=motion) |
| 6-7 | u16 LE | Breathing rate (BPM × 100) |
| 8-11 | u32 LE | Heart rate (BPM × 10000) |
| 12 | i8 | RSSI |
| 13 | u8 | Number of detected persons |
| 14-15 | u8[2] | Reserved |
| 16-19 | f32 LE | Motion energy |
| 20-23 | f32 LE | Presence score |
| 24-27 | u32 LE | Timestamp (ms since boot) |
| 28-31 | u32 LE | Reserved |

**Compressed Frame (magic `0xC5110003`)**:

| Offset | Type | Field |
|--------|------|-------|
| 0-3 | u32 LE | Magic `0xC5110003` |
| 4 | u8 | Node ID |
| 5 | u8 | WiFi channel |
| 6-7 | u16 LE | Original I/Q length |
| 8-9 | u16 LE | Compressed length |
| 10+ | bytes | RLE-encoded XOR delta |

### Configuration

Six NVS keys in the `csi_cfg` namespace:

| NVS Key | Type | Default | Description |
|---------|------|---------|-------------|
| `edge_tier` | u8 | 2 | Processing tier (0/1/2) |
| `pres_thresh` | u16 | 0 | Presence threshold × 1000 (0 = auto) |
| `fall_thresh` | u16 | 2000 | Fall threshold × 1000 (rad/s²) |
| `vital_win` | u16 | 256 | Phase history window |
| `vital_int` | u16 | 1000 | Vitals interval (ms) |
| `subk_count` | u8 | 8 | Top-K subcarrier count |

All configurable via `provision.py --edge-tier 2 --pres-thresh 0.05 ...`

### Additional Features

- **OTA Updates**: HTTP server on port 8032 (`POST /ota`, `GET /ota/status`) with rollback support
- **Power Management**: WiFi modem sleep + automatic light sleep with configurable duty cycle

## Consequences

### Positive
- Fall detection latency reduced from ~500 ms (network RTT) to <50 ms (on-device)
- Bandwidth reduced 30-50% with delta compression, or 95%+ with vitals-only mode
- Battery-powered deployments possible with duty-cycled light sleep
- Server can handle 10x more nodes (only parses 32-byte vitals instead of ~5 KB CSI)

### Negative
- Firmware complexity increases (edge_processing.c is ~750 lines)
- ESP32-S3 RAM usage increases ~12 KB for ring buffer + filter state
- Binary size increases from ~550 KB to ~925 KB with full WASM3 Tier 3 (10% free in 1 MB partition — see ADR-040)

### Risks
- BPM accuracy depends on subject distance and movement; needs real-world validation
- Fall detection heuristic may false-positive on environmental motion (doors, pets)
- Multi-person separation via subcarrier clustering is approximate without calibration

## Implementation

- `firmware/esp32-csi-node/main/edge_processing.c` — DSP pipeline (~750 lines)
- `firmware/esp32-csi-node/main/edge_processing.h` — Types and API
- `firmware/esp32-csi-node/main/ota_update.c/h` — HTTP OTA endpoint
- `firmware/esp32-csi-node/main/power_mgmt.c/h` — Power management
- `rust-port/.../wifi-densepose-sensing-server/src/main.rs` — Vitals parser + REST endpoint
- `scripts/provision.py` — Edge config CLI arguments
- `.github/workflows/firmware-ci.yml` — CI build + size gate (updated to 950 KB for Tier 3)

### Tier 3 — WASM Programmable Sensing (ADR-040, ADR-041)

See [ADR-040](ADR-040-wasm-programmable-sensing.md) for hot-loadable WASM modules
compiled from Rust, executed via WASM3 interpreter on-device. Core modules:
gesture recognition, coherence monitoring, adversarial detection.

[ADR-041](ADR-041-wasm-module-collection.md) defines the curated module collection
(37 modules across 6 categories). Phase 1 implemented modules:
- `vital_trend.rs` — Clinical vital sign trend analysis (bradypnea, tachypnea, apnea)
- `intrusion.rs` — State-machine intrusion detection (calibrate-monitor-arm-alert)
- `occupancy.rs` — Spatial occupancy zone detection with per-zone variance analysis

## Hardware Benchmark (RuView ESP32-S3)

Measured on ESP32-S3 (QFN56 rev v0.2, 8 MB flash, 160 MHz, ESP-IDF v5.2).

### Boot Timing

| Milestone | Time (ms) |
|-----------|-----------|
| `app_main()` | 412 |
| WiFi STA init | 627 |
| WiFi connected + IP | 3,732 |
| CSI collection init | 3,754 |
| Edge DSP task started | 3,773 |
| WASM runtime initialized | 3,857 |
| **Total boot → ready** | **~3.9 s** |

### CSI Performance

| Metric | Value |
|--------|-------|
| Frame rate | **28.5 Hz** (measured, ch 5 BW20) |
| Frame sizes | 128 / 256 bytes |
| RSSI range | -83 to -32 dBm (mean -62 dBm) |
| Per-frame interval | 30.6 ms avg |

### Memory

| Region | Size |
|--------|------|
| RAM (main heap) | 256 KiB |
| RAM (secondary) | 21 KiB |
| DRAM | 32 KiB |
| RTC RAM | 7 KiB |
| **Total available** | **316 KiB** |
| PSRAM | Not populated on test board |
| WASM arena fallback | Internal heap (160 KB/slot × 4) |

### Firmware Binary

| Metric | Value |
|--------|-------|
| Binary size | **925 KB** (0xE7440 bytes) |
| Partition size | 1 MB (factory) |
| Free space | 10% (99 KB) |
| CI size gate | 950 KB (PASS) |
| WASM3 interpreter | Included (full, ~100 KB) |
| WASM binary (7 modules) | 13.8 KB (wasm32-unknown-unknown release) |

### WASM Runtime

| Metric | Value |
|--------|-------|
| Init time | **106 ms** |
| Module slots | 4 |
| Arena per slot | 160 KB |
| Frame budget | 10,000 µs (10 ms) |
| Timer interval | 1,000 ms (1 Hz) |

### Findings

1. **Fall detection threshold too low** — default `fall_thresh=2000` (2.0 rad/s²) triggers 6.7 false positives/s in static indoor environment. Recommend increasing to 5000-8000 for typical deployments.
2. **No PSRAM on test board** — WASM arena falls back to internal heap. Boards with PSRAM would support larger modules.
3. **CSI rate exceeds spec** — measured 28.5 Hz vs. expected ~20 Hz. Performance headroom is better than estimated.
4. **WiFi-to-Ethernet isolation** — some routers block UDP between WiFi and wired clients. Recommend same-subnet verification in deployment guide.
5. **sendto ENOMEM crash (Issue #127)** — CSI callbacks in promiscuous mode fire 100-500+ times/sec, exhausting the lwIP pbuf pool and causing a guru meditation crash. Fixed with a dual approach: 50 Hz rate limiter in `csi_collector.c` (20 ms minimum send interval) and a 100 ms ENOMEM backoff in `stream_sender.c`. Binary size with fix: 947 KB. Hardware-verified stable for 200+ CSI callbacks with zero ENOMEM errors.
