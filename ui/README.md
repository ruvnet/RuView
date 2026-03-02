# WiFi DensePose UI

A modular, modern web interface for the WiFi DensePose human tracking system. Provides real-time monitoring, WiFi sensing visualization, and pose estimation from CSI (Channel State Information).

## Architecture

The UI follows a modular architecture with clear separation of concerns:

```
ui/
├── app.js                    # Main application entry point
├── index.html                # HTML shell with tab structure
├── style.css                 # Complete CSS design system
├── config/
│   └── api.config.js         # API endpoints and configuration
├── services/
│   ├── api.service.js        # HTTP API client
│   ├── websocket.service.js  # WebSocket connection manager
│   ├── websocket-client.js   # Low-level WebSocket client
│   ├── pose.service.js       # Pose estimation API wrapper
│   ├── sensing.service.js    # WiFi sensing data service (live + simulation fallback)
│   ├── health.service.js     # Health monitoring API wrapper
│   ├── stream.service.js     # Streaming API wrapper
│   └── data-processor.js     # Signal data processing utilities
├── components/
│   ├── TabManager.js         # Tab navigation component
│   ├── DashboardTab.js       # Dashboard with live system metrics
│   ├── SensingTab.js         # WiFi sensing visualization (3D signal field, metrics)
│   ├── LiveDemoTab.js        # Live pose detection with setup guide
│   ├── HardwareTab.js        # Hardware configuration
│   ├── SettingsPanel.js      # Settings panel
│   ├── PoseDetectionCanvas.js # Canvas-based pose skeleton renderer
│   ├── gaussian-splats.js    # 3D Gaussian splat signal field renderer (Three.js)
│   ├── body-model.js         # 3D body model
│   ├── scene.js              # Three.js scene management
│   ├── signal-viz.js         # Signal visualization utilities
│   ├── environment.js        # Environment/room visualization
│   └── dashboard-hud.js      # Dashboard heads-up display
├── utils/
│   ├── backend-detector.js   # Auto-detect backend availability
│   ├── mock-server.js        # Mock server for testing
│   └── pose-renderer.js      # Pose rendering utilities
└── tests/
    ├── test-runner.html       # Test runner UI
    ├── test-runner.js         # Test framework and cases
    └── integration-test.html  # Integration testing page
```

## Features

### WiFi Sensing Tab
- 3D Gaussian-splat signal field visualization (Three.js)
- Real-time RSSI, variance, motion band, breathing band metrics
- Presence/motion classification with confidence scores
- **Data source banner**: green "LIVE - ESP32", yellow "RECONNECTING...", or red "SIMULATED DATA"
- Sparkline RSSI history graph
- "About This Data" card explaining CSI capabilities per sensor count

### Live Demo Tab
- WebSocket-based real-time pose skeleton rendering
- **Estimation Mode badge**: green "Signal-Derived" or blue "Model Inference"
- **Setup Guide panel** showing what each ESP32 count provides:
  - 1 ESP32: presence, breathing, gross motion
  - 2-3 ESP32s: body localization, motion direction
  - 4+ ESP32s + trained model: individual limb tracking, full pose
- Debug mode with log export
- Zone selection and force-reconnect controls
- Performance metrics sidebar (frames, uptime, errors)

### Dashboard
- Live system health monitoring
- Real-time pose detection statistics
- Zone occupancy tracking
- System metrics (CPU, memory, disk)
- API status indicators

### Hardware Configuration
- Interactive antenna array visualization
- Real-time CSI data display
- Configuration panels
- Hardware status monitoring

## Data Sources

The sensing service (`sensing.service.js`) supports three connection states:

| State | Banner Color | Description |
|-------|-------------|-------------|
| **LIVE - ESP32** | Green | Connected to the Rust sensing server receiving real CSI data |
| **RECONNECTING** | Yellow (pulsing) | WebSocket disconnected, retrying (up to 20 attempts) |
| **SIMULATED DATA** | Red | Fallback to client-side simulation after 5+ failed reconnects |

Simulated frames include a `_simulated: true` marker so code can detect synthetic data.

## Backends

### Rust Sensing Server (primary)
The Rust-based `wifi-densepose-sensing-server` serves the UI and provides:
- `GET /health` — server health
- `GET /api/v1/sensing/latest` — latest sensing features
- `GET /api/v1/vital-signs` — vital sign estimates (HR/RR)
- `GET /api/v1/model/info` — RVF model container info
- `WS /ws/sensing` — real-time sensing data stream
- `WS /api/v1/stream/pose` — real-time pose keypoint stream

### Python FastAPI (legacy)
The original Python backend on port 8000 is still supported. The UI auto-detects which backend is available via `backend-detector.js`.

## Quick Start

### With Docker (recommended)
```bash
cd docker/

# Default: auto-detects ESP32 on UDP 5005, falls back to simulation
docker-compose up

# Force real ESP32 data
CSI_SOURCE=esp32 docker-compose up

# Force simulation (no hardware needed)
CSI_SOURCE=simulated docker-compose up
```
Open http://localhost:3000/ui/index.html

### With local Rust binary
```bash
cd rust-port/wifi-densepose-rs
cargo build -p wifi-densepose-sensing-server --no-default-features

# Run with simulated data
../../target/debug/sensing-server --source simulated --tick-ms 100 --ui-path ../../ui --http-port 3000

# Run with real ESP32
../../target/debug/sensing-server --source esp32 --tick-ms 100 --ui-path ../../ui --http-port 3000
```
Open http://localhost:3000/ui/index.html

### With Python HTTP server (legacy)
```bash
# Start FastAPI backend on port 8000
wifi-densepose start

# Serve the UI on port 3000
cd ui/
python -m http.server 3000
```
Open http://localhost:3000

## Pose Estimation Modes

| Mode | Badge | Requirements | Accuracy |
|------|-------|-------------|----------|
| **Signal-Derived** | Green | 1+ ESP32, no model needed | Presence, breathing, gross motion |
| **Model Inference** | Blue | 4+ ESP32s + trained `.rvf` model | Full 17-keypoint COCO pose |

To use model inference, start the server with a trained model:
```bash
sensing-server --source esp32 --model path/to/model.rvf --ui-path ./ui
```

## Configuration

### API Configuration
Edit `config/api.config.js`:

```javascript
export const API_CONFIG = {
  BASE_URL: window.location.origin,
  API_VERSION: '/api/v1',
  WS_CONFIG: {
    RECONNECT_DELAY: 5000,
    MAX_RECONNECT_ATTEMPTS: 20,
    PING_INTERVAL: 30000
  }
};
```

## Testing

Open `tests/test-runner.html` to run the test suite:

```bash
cd ui/
python -m http.server 3000
# Open http://localhost:3000/tests/test-runner.html
```

Test categories: API configuration, API service, WebSocket, pose service, health service, UI components, integration.

## Styling

Uses a CSS design system with custom properties, dark/light mode, responsive layout, and component-based styling. Key variables in `:root` of `style.css`.

## License

Part of the WiFi-DensePose system. See the main project LICENSE file.
