# ADR-046: Android TV Box / Armbian Deployment Target

## Status

Proposed

## Context

Issue [#138](https://github.com/ruvnet/wifi-densepose/issues/138) requests ESP8266 and mobile device support. The ESP8266 lacks CSI capability and sufficient resources, but the discussion revealed a compelling deployment target: **Android TV boxes** (Amlogic/Allwinner/Rockchip SoCs) running **Armbian** (Debian for ARM).

These devices cost $15–35, are always-on mains-powered, include 802.11ac WiFi, 2–4 GB RAM, quad-core ARM Cortex-A53/A55 CPUs, and HDMI output. They are widely available as consumer "IPTV boxes" (T95, H96 Max, X96, MXQ Pro, etc.) and can boot Armbian from SD card without modifying the factory Android installation.

### Current deployment model

```
[ESP32-S3 nodes] --UDP CSI--> [Laptop/PC running sensing-server] --browser--> [UI]
```

This requires a general-purpose computer ($300+) to run the Rust sensing server, NN inference, and web dashboard. For permanent installations (elder care, smart home, security), dedicating a laptop is impractical.

### Proposed deployment model

```
[ESP32-S3 nodes] --UDP CSI--> [TV Box running Armbian + sensing-server] --HDMI--> [Display]
                                 $25, always-on, fanless
```

### Future: custom WiFi firmware for standalone operation

Many TV box WiFi chipsets (Realtek RTL8822CS, MediaTek MT7661, Broadcom BCM43455) can potentially be patched for CSI extraction when running under Linux with custom drivers. This would eliminate the ESP32 dependency entirely for basic sensing:

```
[TV Box with patched WiFi driver] --CSI extraction--> [sensing-server on same box] --HDMI--> [Display]
                                    $25 total, single device
```

This ADR covers Phase 1 (TV box as aggregator) and Phase 2 (custom WiFi firmware for CSI). Phase 2 is speculative and requires per-chipset R&D.

## Decision

### Phase 1: TV Box as Aggregator (Armbian)

1. **Cross-compile the sensing server** for `aarch64-unknown-linux-gnu` using `cross` or Docker-based cross-compilation.

2. **Create an Armbian deployment package** containing:
   - Pre-built `wifi-densepose-sensing-server` binary (aarch64)
   - systemd service file for auto-start on boot
   - Kiosk-mode Chromium configuration for HDMI dashboard display
   - Network configuration for ESP32 UDP reception (port 5005)
   - Optional: `hostapd` config to create a dedicated WiFi AP for the ESP32 mesh

3. **Define minimum hardware requirements:**

   | Component | Minimum | Recommended |
   |-----------|---------|-------------|
   | SoC | Amlogic S905W (A53 quad) | Amlogic S905X3 (A55 quad) |
   | RAM | 2 GB | 4 GB |
   | Storage | 8 GB eMMC + 8 GB SD | 16 GB eMMC + 16 GB SD |
   | WiFi | 802.11n 2.4 GHz | 802.11ac dual-band |
   | Ethernet | 100 Mbps | Gigabit |
   | USB | 1x USB 2.0 | 2x USB 3.0 |
   | HDMI | 1.4 | 2.0 |

4. **Tested reference devices** (initial target list):

   | Device | SoC | WiFi Chip | Price | Armbian Support |
   |--------|-----|-----------|-------|-----------------|
   | T95 Max+ | S905X3 | RTL8822CS | ~$30 | Good (meson-sm1) |
   | H96 Max X3 | S905X3 | RTL8822CS | ~$35 | Good (meson-sm1) |
   | X96 Max+ | S905X3 | RTL8822CS | ~$28 | Good (meson-sm1) |
   | Tanix TX6S | H616 | MT7668 | ~$25 | Moderate (sun50i-h616) |

5. **New Rust compilation target** in workspace CI:
   - Add `aarch64-unknown-linux-gnu` to cross-compilation matrix
   - Binary size target: <15 MB stripped (fits easily in SD card)
   - No GPU dependency — CPU-only inference using `candle` or ONNX Runtime for ARM

### Phase 2: Custom WiFi Firmware for CSI Extraction (Future)

1. **CSI extraction feasibility by chipset:**

   | Chipset | Driver | CSI Support | Monitor Mode | Effort |
   |---------|--------|-------------|--------------|--------|
   | Broadcom BCM43455 | brcmfmac | **Proven** (Nexmon CSI) | Yes | Low — patches exist |
   | Realtek RTL8822CS | rtw88 | **Moderate** — driver is open-source, CSI hooks need adding | Yes (patched) | Medium |
   | MediaTek MT7661 | mt76 | **Unknown** — MediaTek has released CSI tools for some chips | Yes | Medium-High |

2. **CSI extraction architecture** (Linux kernel driver modification):

   ```
   [WiFi chipset firmware] → [Modified kernel driver] → [Netlink/procfs CSI export]
                                                              ↓
                                                     [userspace CSI reader]
                                                              ↓
                                                     [sensing-server UDP input]
   ```

   The CSI data would be reformatted into the existing ESP32 binary protocol (ADR-018 header, magic `0xC5100001`) so the sensing server treats it identically to ESP32 frames. This means zero changes to the ingestion context.

3. **Hybrid mode**: When the TV box has both patched WiFi CSI and ESP32 UDP input, the sensing server's multi-node architecture (already supporting multiple `node_id` values) handles both sources transparently. The TV box's own WiFi becomes an additional viewpoint in the multistatic array.

### Phase 3: Android Companion App (Optional)

For users who want mobile monitoring without Armbian:

1. **PWA (Progressive Web App)**: The sensing server already serves a web UI. Adding a PWA manifest with offline caching makes it installable on any Android device. No native app needed.

2. **Native Android app** (future): Only if PWA proves insufficient. Would use Kotlin + Jetpack Compose, consuming the existing REST API and WebSocket endpoints.

## Deployment Architecture

### Single-Room Deployment (Phase 1)

```
┌──────────────────────────────────────────────────────────────┐
│                        Room                                  │
│                                                              │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                  │
│  │ ESP32-S3 │  │ ESP32-S3 │  │ ESP32-S3 │  CSI sensor mesh │
│  │ Node 1   │  │ Node 2   │  │ Node 3   │  ($10 each)      │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                  │
│       │              │              │                         │
│       └──────────────┼──────────────┘                        │
│                      │ UDP port 5005                         │
│                      ▼                                       │
│  ┌──────────────────────────────────────┐                   │
│  │      Android TV Box (Armbian)        │                   │
│  │                                      │                   │
│  │  ┌──────────────────────────────┐   │                   │
│  │  │   wifi-densepose-sensing-    │   │                   │
│  │  │   server (aarch64 binary)    │   │                   │
│  │  │                              │   │                   │
│  │  │  • CSI ingestion (UDP)       │   │                   │
│  │  │  • Feature extraction        │   │                   │
│  │  │  • NN inference (CPU)        │   │                   │
│  │  │  • WebSocket streaming       │   │                   │
│  │  │  • REST API                  │   │                   │
│  │  │  • Web UI (:3000)            │   │                   │
│  │  └──────────────────────────────┘   │                   │
│  │                                      │                   │
│  │  ┌──────────────────────────────┐   │                   │
│  │  │   Chromium Kiosk Mode        │───│──→ HDMI out       │
│  │  │   (localhost:3000)           │   │    to display      │
│  │  └──────────────────────────────┘   │                   │
│  │                                      │                   │
│  │  Cost: $25-35                        │                   │
│  │  Power: 5-10W (USB-C or barrel)      │                   │
│  │  Form: fits behind TV/monitor        │                   │
│  └──────────────────────────────────────┘                   │
│                                                              │
└──────────────────────────────────────────────────────────────┘

Total system cost: $55-65 (3 ESP32 nodes + 1 TV box)
```

### Multi-Room Deployment

```
                    ┌──────────────┐
                    │   Router     │
                    │  (WiFi AP)   │
                    └──────┬───────┘
                           │ LAN
            ┌──────────────┼──────────────┐
            │              │              │
    ┌───────▼───────┐ ┌───▼────────┐ ┌──▼──────────┐
    │  Room A       │ │  Room B    │ │  Room C     │
    │  TV Box +     │ │  TV Box +  │ │  TV Box +   │
    │  3x ESP32     │ │  3x ESP32  │ │  3x ESP32   │
    │  HDMI display │ │  HDMI      │ │  HDMI       │
    └───────────────┘ └────────────┘ └─────────────┘

    Each room: self-contained sensing + display
    Central dashboard: aggregate all rooms via REST API
```

### Standalone Mode (Phase 2 — Custom WiFi FW)

```
┌──────────────────────────────────────┐
│      Android TV Box (Armbian)        │
│                                      │
│  ┌────────────────────┐              │
│  │  Patched WiFi      │              │
│  │  Driver             │              │
│  │  (CSI extraction)  │              │
│  └─────────┬──────────┘              │
│            │ CSI frames              │
│            ▼                         │
│  ┌────────────────────┐              │
│  │  sensing-server    │──→ HDMI out  │
│  │  (inference +      │              │
│  │   dashboard)       │              │
│  └────────────────────┘              │
│                                      │
│  Single device: $25                  │
│  No ESP32 nodes needed               │
└──────────────────────────────────────┘
```

## Consequences

### Positive

- **10x cost reduction** for aggregator: $25 TV box vs $300+ laptop/PC
- **Always-on deployment**: Mains-powered, fanless, designed for 24/7 operation
- **HDMI output**: Direct connection to TV/monitor for wall-mounted dashboards
- **Familiar hardware**: Available globally, no specialized ordering required
- **Armbian ecosystem**: Mature Debian-based distro with package management, systemd, SSH
- **Path to standalone**: Custom WiFi firmware could eliminate ESP32 dependency entirely
- **PWA for mobile**: No native app development needed for mobile monitoring
- **Multi-room scaling**: One TV box per room, each self-contained

### Negative

- **ARM cross-compilation**: Adds CI complexity; `candle`/ONNX Runtime ARM builds need testing
- **Armbian compatibility**: Not all TV boxes are well-supported; need a tested device list
- **Performance uncertainty**: ARM A53 cores are ~3-5x slower than x86 for NN inference; may need model quantization (INT8) for real-time operation
- **Phase 2 risk**: Custom WiFi firmware is chipset-specific, may require kernel patches per driver version, and CSI quality varies by chipset
- **Support burden**: Different hardware = more configurations to support
- **No GPU**: TV boxes lack discrete GPU; inference is CPU-only (but our models are small enough)

### Neutral

- **No changes to existing ESP32 firmware** — TV box receives the same UDP frames
- **No changes to sensing server protocol** — Phase 2 CSI output uses same binary format
- **Existing web UI works as-is** — Chromium kiosk mode or any browser on the LAN

## Implementation Plan

### Phase 1 (2-3 weeks)

1. Add `aarch64-unknown-linux-gnu` cross-compilation target using `cross`
2. Build and test sensing-server binary on reference TV box (T95 Max+ / S905X3)
3. Create systemd service + Armbian deployment script
4. Benchmark: measure inference latency, memory usage, thermal throttling
5. Create `docs/deployment/armbian-tv-box.md` setup guide
6. Add HDMI kiosk mode configuration (Chromium autostart)

### Phase 2 (4-8 weeks, R&D)

1. Acquire TV box with BCM43455 (proven Nexmon CSI support)
2. Build Armbian with Nexmon CSI patches for BCM43455
3. Write userspace CSI reader → ESP32 binary protocol converter
4. Test CSI quality comparison: ESP32 vs BCM43455
5. If viable: add RTL8822CS CSI extraction via rtw88 driver modification

### Phase 3 (1 week)

1. Add PWA manifest to sensing server web UI
2. Test on Android Chrome, iOS Safari
3. Add service worker for offline dashboard caching

## References

- [Nexmon CSI](https://github.com/seemoo-lab/nexmon_csi) — Broadcom WiFi CSI extraction (BCM43455, BCM4339, BCM4358)
- [Armbian](https://www.armbian.com/) — Debian/Ubuntu for ARM SBCs and TV boxes
- [rtw88 driver](https://github.com/torvalds/linux/tree/master/drivers/net/wireless/realtek/rtw88) — Mainline Linux driver for Realtek 802.11ac chips
- [mt76 driver](https://github.com/torvalds/linux/tree/master/drivers/net/wireless/mediatek/mt76) — Mainline Linux driver for MediaTek WiFi chips
- [cross](https://github.com/cross-rs/cross) — Zero-setup Rust cross-compilation
- [ADR-018: ESP32 CSI Binary Protocol](ADR-018-dev-implementation.md) — Binary frame format reused for Phase 2 CSI extraction
- [ADR-039: Edge Intelligence](ADR-039-esp32-edge-intelligence.md) — On-device processing tiers
- [ADR-043: Sensing Server](ADR-043-sensing-server-ui-api-completion.md) — Single-binary deployment target
