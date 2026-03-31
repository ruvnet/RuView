# ADR-059: Live ESP32 CSI Pipeline Integration

## Status

Accepted

## Date

2026-03-12

## Context

ADR-058 established a dual-modal browser demo combining webcam video and WiFi CSI for pose estimation. However, it used simulated CSI data. To demonstrate real-world capability, we need an end-to-end pipeline from physical ESP32 hardware through to the browser visualization.

The ESP32-S3 firmware (`firmware/esp32-csi-node/`) already supports CSI collection and UDP streaming (ADR-018). The sensing server (`wifi-densepose-sensing-server`) already supports UDP ingestion and WebSocket bridging. The missing piece was connecting these components and enabling the browser demo to consume live data.

## Decision

Implement a complete live CSI pipeline:

```
ESP32-S3 (CSI capture) → UDP:5005 → sensing-server (Rust/Axum) → WS:8765 → browser demo
```

### Components

1. **ESP32 Firmware** - Rebuilt with native Windows ESP-IDF v5.4.0 toolchain (no Docker). Configured for target network and PC IP via `sdkconfig`. Helper scripts added:
   - `build_firmware.ps1` - Sets up IDF environment, cleans, builds, and flashes
   - `read_serial.ps1` - Serial monitor with DTR/RTS reset capability

2. **Sensing Server** - `wifi-densepose-sensing-server` started with:
   - `--source esp32` - Expect real ESP32 UDP frames
   - `--bind-addr 0.0.0.0` - Accept connections from any interface
   - `--ui-path <path>` - Serve the demo UI via HTTP

3. **Browser Demo** - `main.js` updated to auto-connect to `ws://localhost:8765/ws/sensing` on page load. Falls back to simulated CSI if the WebSocket is unavailable (GitHub Pages).

### Network Configuration

The ESP32 sends UDP packets to a configured target IP. If the PC's IP doesn't match the firmware's compiled target, a secondary IP alias can be added:

```powershell
# PowerShell (Admin)
New-NetIPAddress -IPAddress 192.168.1.100 -PrefixLength 24 -InterfaceAlias "Wi-Fi"
```

### Data Flow

| Stage | Protocol | Format | Rate |
|-------|----------|--------|------|
| ESP32 → Server | UDP | ADR-018 binary frame (magic `0xC5110001`, I/Q pairs) | ~100 Hz |
| Server → Browser | WebSocket | ADR-018 binary frame (forwarded) | ~10 Hz (tick-ms=100) |
| Browser decode | JavaScript | Float32 amplitude/phase arrays | Per frame |

### Build Environment (Windows)

ESP-IDF v5.4.0 on Windows requires:
- IDF_PATH pointing to the ESP-IDF framework
- IDF_TOOLS_PATH pointing to toolchain binaries
- MSYS/MinGW environment variables removed (ESP-IDF rejects them)
- Python venv from ESP-IDF tools for `idf.py` execution

The `build_firmware.ps1` script handles all of this automatically.

## Consequences

### Positive
- First end-to-end demonstration of real WiFi CSI → pose estimation in a browser
- No Docker required for firmware builds on Windows
- Demo gracefully degrades to simulated CSI when no server is available
- Same demo works on GitHub Pages (simulated) and locally (live ESP32)

### Negative
- ESP32 target IP is compiled into firmware; changing it requires a rebuild or NVS override
- Windows firewall may block UDP:5005; user must allow it
- Mixed content restrictions prevent HTTPS pages from connecting to ws:// (local only)

## Related

- [ADR-018](ADR-018-esp32-dev-implementation.md) - ESP32 CSI frame format and UDP streaming
- [ADR-058](ADR-058-ruvector-wasm-browser-pose-example.md) - Dual-modal WASM browser pose demo
- [ADR-039](ADR-039-edge-intelligence-framework.md) - Edge intelligence on ESP32
- Issue [#245](https://github.com/ruvnet/RuView/issues/245) - Tracking issue
