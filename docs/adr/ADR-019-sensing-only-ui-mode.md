# ADR-019: Sensing-Only UI Mode with Gaussian Splat Visualization

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-02-28 |
| **Deciders** | ruv |
| **Relates to** | ADR-013 (Feature-Level Sensing), ADR-018 (ESP32 Dev Implementation) |

## Context

The WiFi-DensePose UI was originally built to require the full FastAPI DensePose backend (`localhost:8000`) for all functionality. This backend depends on heavy Python packages (PyTorch ~2GB, torchvision, OpenCV, SQLAlchemy, Redis) making it impractical for lightweight sensing-only deployments where the user simply wants to visualize live WiFi signal data from ESP32 CSI or Windows RSSI collectors.

A Rust port exists (`rust-port/wifi-densepose-rs`) using Axum with lighter runtime footprint (~10MB binary, ~5MB RAM), but it still requires libtorch C++ bindings and OpenBLAS for compilation—a non-trivial build.

Users need a way to run the UI with **only the sensing pipeline** active, without installing the full DensePose backend stack.

## Decision

Implement a **sensing-only UI mode** that:

1. **Decouples the sensing pipeline** from the DensePose API backend. The sensing WebSocket server (`ws_server.py` on port 8765) operates independently of the FastAPI backend (port 8000).

2. **Auto-detects sensing-only mode** at startup. When the DensePose backend is unreachable, the UI sets `backendDetector.sensingOnlyMode = true` and:
   - Suppresses all API requests to `localhost:8000` at the `ApiService.request()` level
   - Skips initialization of DensePose-dependent tabs (Dashboard, Hardware, Live Demo)
   - Shows a green "Sensing mode" status toast instead of error banners
   - Silences health monitoring polls

3. **Adds a new "Sensing" tab** with Three.js Gaussian splat visualization:
   - Custom GLSL `ShaderMaterial` rendering point-cloud splats on a 20×20 floor grid
   - Signal field splats colored by intensity (blue → green → red)
   - Body disruption blob at estimated motion position
   - Breathing ring modulation when breathing-band power detected
   - Side panel with RSSI sparkline, feature meters, and classification badge

4. **Python WebSocket bridge** (`v1/src/sensing/ws_server.py`) that:
   - Auto-detects ESP32 UDP CSI stream on port 5005 (ADR-018 binary frames)
   - Falls back to `WindowsWifiCollector` → `SimulatedCollector`
   - Runs `RssiFeatureExtractor` → `PresenceClassifier` pipeline
   - Broadcasts JSON sensing updates every 500ms on `ws://localhost:8765`

5. **Client-side fallback**: `sensing.service.js` generates simulated data when the WebSocket server is unreachable, so the visualization always works.

## Architecture

```
ESP32 (UDP :5005)  ──┐
                     ├──▶  ws_server.py (:8765)  ──▶  sensing.service.js  ──▶  SensingTab.js
Windows WiFi RSSI ───┘         │                          │                      │
                          Feature extraction          WebSocket client      gaussian-splats.js
                          + Classification            + Reconnect            (Three.js ShaderMaterial)
                                                      + Sim fallback
```

### Data flow

| Source | Collector | Feature Extraction | Output |
|--------|-----------|-------------------|--------|
| ESP32 CSI (ADR-018) | `Esp32UdpCollector` (UDP :5005) | Amplitude mean → pseudo-RSSI → `RssiFeatureExtractor` | `sensing_update` JSON |
| Windows WiFi | `WindowsWifiCollector` (netsh) | RSSI + signal% → `RssiFeatureExtractor` | `sensing_update` JSON |
| Simulated | `SimulatedCollector` | Synthetic RSSI patterns | `sensing_update` JSON |

### Sensing update JSON schema

```json
{
  "type": "sensing_update",
  "timestamp": 1234567890.123,
  "source": "esp32",
  "nodes": [{ "node_id": 1, "rssi_dbm": -39, "position": [2,0,1.5], "amplitude": [...], "subcarrier_count": 56 }],
  "features": { "mean_rssi": -39.0, "variance": 2.34, "motion_band_power": 0.45, ... },
  "classification": { "motion_level": "active", "presence": true, "confidence": 0.87 },
  "signal_field": { "grid_size": [20,1,20], "values": [...] }
}
```

## Files

### Created
| File | Purpose |
|------|---------|
| `v1/src/sensing/ws_server.py` | Python asyncio WebSocket server with auto-detect collectors |
| `ui/components/SensingTab.js` | Sensing tab UI with Three.js integration |
| `ui/components/gaussian-splats.js` | Custom GLSL Gaussian splat renderer |
| `ui/services/sensing.service.js` | WebSocket client with reconnect + simulation fallback |

### Modified
| File | Change |
|------|--------|
| `ui/index.html` | Added Sensing nav tab button and content section |
| `ui/app.js` | Sensing-only mode detection, conditional tab init |
| `ui/style.css` | Sensing tab layout and component styles |
| `ui/config/api.config.js` | `AUTO_DETECT: false` (sensing uses own WS) |
| `ui/services/api.service.js` | Short-circuit requests in sensing-only mode |
| `ui/services/health.service.js` | Skip polling when backend unreachable |
| `ui/components/DashboardTab.js` | Graceful failure in sensing-only mode |

## Consequences

### Positive
- UI works with zero heavy dependencies—only `pip install websockets` (+ numpy/scipy already installed)
- ESP32 CSI data flows end-to-end without PyTorch, OpenCV, or database
- Existing DensePose tabs still work when the full backend is running
- Clean console output—no `ERR_CONNECTION_REFUSED` spam in sensing-only mode

### Negative
- Two separate WebSocket endpoints: `:8765` (sensing) and `:8000/api/v1/stream/pose` (DensePose)
- Pose estimation, zone occupancy, and historical data features unavailable in sensing-only mode
- Client-side simulation fallback may mislead users if they don't notice the "Simulated" badge

### Neutral
- Rust Axum backend remains a future option for a unified lightweight server
- The sensing pipeline reuses the existing `RssiFeatureExtractor` and `PresenceClassifier` classes unchanged

## Alternatives Considered

1. **Install minimal FastAPI** (`pip install fastapi uvicorn pydantic`): Starts the server but pose endpoints return errors without PyTorch.
2. **Build Rust backend**: Single binary, but requires libtorch + OpenBLAS build toolchain.
3. **Merge sensing into FastAPI**: Would require FastAPI installed even for sensing-only use.

Option 1 was rejected because it still shows broken tabs. The chosen approach cleanly separates concerns.
