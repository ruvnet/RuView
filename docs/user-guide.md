# WiFi DensePose User Guide

WiFi DensePose turns commodity WiFi signals into real-time human pose estimation, vital sign monitoring, and presence detection. This guide walks you through installation, first run, API usage, hardware setup, and model training.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
   - [Docker (Recommended)](#docker-recommended)
   - [From Source (Rust)](#from-source-rust)
   - [From crates.io](#from-cratesio-individual-crates)
   - [From Source (Python)](#from-source-python)
   - [Guided Installer](#guided-installer)
3. [Quick Start](#quick-start)
   - [30-Second Demo (Docker)](#30-second-demo-docker)
   - [Verify the System Works](#verify-the-system-works)
4. [Data Sources](#data-sources)
   - [Simulated Mode (No Hardware)](#simulated-mode-no-hardware)
   - [Windows WiFi (RSSI Only)](#windows-wifi-rssi-only)
   - [ESP32-S3 (Full CSI)](#esp32-s3-full-csi)
   - [ESP32 Multistatic Mesh (Advanced)](#esp32-multistatic-mesh-advanced)
   - [Cognitum Seed Integration (ADR-069)](#cognitum-seed-integration-adr-069)
5. [REST API Reference](#rest-api-reference)
6. [WebSocket Streaming](#websocket-streaming)
7. [Web UI](#web-ui)
8. [Vital Sign Detection](#vital-sign-detection)
9. [CLI Reference](#cli-reference)
10. [Observatory Visualization](#observatory-visualization)
11. [Adaptive Classifier](#adaptive-classifier)
    - [Recording Training Data](#recording-training-data)
    - [Training the Model](#training-the-model)
    - [Using the Trained Model](#using-the-trained-model)
12. [Training a Model](#training-a-model)
    - [CRV Signal-Line Protocol](#crv-signal-line-protocol)
13. [RVF Model Containers](#rvf-model-containers)
14. [Hardware Setup](#hardware-setup)
    - [ESP32-S3 Mesh](#esp32-s3-mesh)
    - [Intel 5300 / Atheros NIC](#intel-5300--atheros-nic)
15. [Camera-Free Pose Training](#camera-free-pose-training)
16. [ruvllm Training Pipeline](#ruvllm-training-pipeline)
17. [Docker Compose (Multi-Service)](#docker-compose-multi-service)
16. [Testing Firmware Without Hardware (QEMU)](#testing-firmware-without-hardware-qemu)
    - [What You Need](#what-you-need)
    - [Your First Test Run](#your-first-test-run)
    - [Understanding the Test Output](#understanding-the-test-output)
    - [Testing Multiple Nodes at Once (Swarm)](#testing-multiple-nodes-at-once-swarm)
    - [Swarm Presets](#swarm-presets)
    - [Writing Your Own Swarm Config](#writing-your-own-swarm-config)
    - [Debugging Firmware in QEMU](#debugging-firmware-in-qemu)
    - [Running the Full Test Suite](#running-the-full-test-suite)
17. [Troubleshooting](#troubleshooting)
18. [FAQ](#faq)

---

## Prerequisites

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| **OS** | Windows 10/11, macOS 10.15, Ubuntu 18.04 | Latest stable |
| **RAM** | 4 GB | 8 GB+ |
| **Disk** | 2 GB free | 5 GB free |
| **Docker** (for Docker path) | Docker 20+ | Docker 24+ |
| **Rust** (for source build) | 1.70+ | 1.85+ |
| **Python** (for legacy v1) | 3.10+ | 3.13+ |

**Hardware for live sensing (optional):**

| Option | Cost | Capabilities |
|--------|------|-------------|
| ESP32-S3 mesh (3-6 boards) | ~$54 | Full CSI: pose, breathing, heartbeat, presence |
| Intel 5300 / Atheros AR9580 | $50-100 | Full CSI with 3x3 MIMO (Linux only) |
| Any WiFi laptop | $0 | RSSI-only: coarse presence and motion detection |

No hardware? The system runs in **simulated mode** with synthetic CSI data.

---

## Installation

### Docker (Recommended)

The fastest path. No toolchain installation needed.

```bash
docker pull ruvnet/wifi-densepose:latest
```

Multi-architecture image (amd64 + arm64). Works on Intel/AMD and Apple Silicon Macs. Contains the Rust sensing server, Three.js UI, and all signal processing.

**Data source selection:** Use the `CSI_SOURCE` environment variable to select the sensing mode:

| Value | Description |
|-------|-------------|
| `auto` | (default) Probe for ESP32 on UDP 5005, fall back to simulation |
| `esp32` | Receive real CSI frames from ESP32 devices over UDP |
| `simulated` | Generate synthetic CSI frames (no hardware required) |
| `wifi` | Host Wi-Fi RSSI (not available inside containers) |

Example: `docker run -e CSI_SOURCE=esp32 -p 3000:3000 -p 5005:5005/udp ruvnet/wifi-densepose:latest`

### From Source (Rust)

On Debian/Ubuntu-based Linux systems, install the native desktop prerequisites before the first Rust release build:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config \
  libglib2.0-dev libgtk-3-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libwebkit2gtk-4.1-dev
```

This prepares the native GTK/WebKit dependencies used by the desktop/Tauri crates in this workspace.

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView/rust-port/wifi-densepose-rs

# Build
cargo build --release

# Verify (runs 1,400+ tests)
cargo test --workspace --no-default-features
```

The compiled binary is at `target/release/sensing-server`.

### From crates.io (Individual Crates)

All 16 crates are published to crates.io at v0.3.0. Add individual crates to your own Rust project:

```bash
# Core types and traits
cargo add wifi-densepose-core

# Signal processing (includes RuvSense multistatic sensing)
cargo add wifi-densepose-signal

# Neural network inference
cargo add wifi-densepose-nn

# Mass Casualty Assessment Tool
cargo add wifi-densepose-mat

# ESP32 hardware + TDM protocol + QUIC transport
cargo add wifi-densepose-hardware

# RuVector integration (add --features crv for CRV signal-line protocol)
cargo add wifi-densepose-ruvector --features crv

# WebAssembly bindings
cargo add wifi-densepose-wasm

# WASM edge runtime (lightweight, for embedded/IoT)
cargo add wifi-densepose-wasm-edge
```

See the full crate list and dependency order in [CLAUDE.md](../CLAUDE.md#crate-publishing-order).

### From Source (Python)

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView

pip install -r requirements.txt
pip install -e .

# Or via PyPI
pip install wifi-densepose
pip install wifi-densepose[gpu]   # GPU acceleration
pip install wifi-densepose[all]   # All optional deps
```

### Guided Installer

An interactive installer that detects your hardware and recommends a profile:

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView
./install.sh
```

Available profiles: `verify`, `python`, `rust`, `browser`, `iot`, `docker`, `field`, `full`.

Non-interactive:
```bash
./install.sh --profile rust --yes
```

---

## Quick Start

### 30-Second Demo (Docker)

```bash
# Pull and run
docker run -p 3000:3000 -p 3001:3001 ruvnet/wifi-densepose:latest

# Open the UI in your browser
# http://localhost:3000
```

You will see a Three.js visualization with:
- 3D body skeleton (17 COCO keypoints)
- Signal amplitude heatmap
- Phase plot
- Vital signs panel (breathing + heartbeat)

### Verify the System Works

Open a second terminal and test the API:

```bash
# Health check
curl http://localhost:3000/health
# Expected: {"status":"ok","source":"simulated","clients":0}

# Latest sensing frame
curl http://localhost:3000/api/v1/sensing/latest

# Vital signs
curl http://localhost:3000/api/v1/vital-signs

# Pose estimation (17 COCO keypoints)
curl http://localhost:3000/api/v1/pose/current

# Server build info
curl http://localhost:3000/api/v1/info
```

All endpoints return JSON. In simulated mode, data is generated from a deterministic reference signal.

---

## Data Sources

The `--source` flag controls where CSI data comes from.

### Simulated Mode (No Hardware)

Default in Docker. Generates synthetic CSI data exercising the full pipeline.

```bash
# Docker
docker run -p 3000:3000 ruvnet/wifi-densepose:latest
# (--source auto is the default; falls back to simulate when no hardware detected)

# From source
./target/release/sensing-server --source simulate --http-port 3000 --ws-port 3001
```

### Windows WiFi (RSSI Only)

Uses `netsh wlan` to capture RSSI from nearby access points. No special hardware needed. Supports presence detection, motion classification, and coarse breathing rate estimation. No pose estimation (requires CSI).

```bash
# From source (Windows only)
./target/release/sensing-server --source wifi --http-port 3000 --ws-port 3001 --tick-ms 500

# Docker (requires --network host on Windows)
docker run --network host ruvnet/wifi-densepose:latest --source wifi --tick-ms 500
```

> **Community verified:** Tested on Windows 10 (10.0.26200) with Intel Wi-Fi 6 AX201 160MHz, Python 3.14, StormFiber 5 GHz network. All 7 tutorial steps passed with stable RSSI readings at -48 dBm. See [Tutorial #36](https://github.com/ruvnet/RuView/issues/36) for the full walkthrough and test results.

**Vital signs from RSSI:** The sensing server now supports breathing rate estimation from RSSI variance patterns (requires stationary subject near AP) and motion classification with confidence scoring. RSSI-based vital sign detection has lower fidelity than ESP32 CSI — it is best for presence detection and coarse motion classification.

### macOS WiFi (RSSI Only)

Uses CoreWLAN via a Swift helper binary. macOS Sonoma 14.4+ redacts real BSSIDs; the adapter generates deterministic synthetic MACs so the multi-BSSID pipeline still works.

```bash
# Compile the Swift helper (once)
swiftc -O v1/src/sensing/mac_wifi.swift -o mac_wifi

# Run natively
./target/release/sensing-server --source macos --http-port 3000 --ws-port 3001 --tick-ms 500
```

See [ADR-025](adr/ADR-025-macos-corewlan-wifi-sensing.md) for details.

### Linux WiFi (RSSI Only)

Uses `iw dev <iface> scan` to capture RSSI. Requires `CAP_NET_ADMIN` (root) for active scans; use `scan dump` for cached results without root.

```bash
# Run natively (requires root for active scanning)
sudo ./target/release/sensing-server --source linux --http-port 3000 --ws-port 3001 --tick-ms 500
```

### ESP32-S3 (Full CSI)

Real Channel State Information at 20 Hz with 56-192 subcarriers. Required for pose estimation, vital signs, and through-wall sensing.

```bash
# From source
./target/release/sensing-server --source esp32 --udp-port 5005 --http-port 3000 --ws-port 3001

# Docker (use CSI_SOURCE environment variable)
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp -e CSI_SOURCE=esp32 ruvnet/wifi-densepose:latest
```

The ESP32 nodes stream binary CSI frames over UDP to port 5005. See [Hardware Setup](#esp32-s3-mesh) for flashing instructions.

### ESP32 Multistatic Mesh (Advanced)

For higher accuracy with through-wall tracking, deploy 3-6 ESP32-S3 nodes in a **multistatic mesh** configuration. Each node acts as both transmitter and receiver, creating multiple sensing paths through the environment.

```bash
# Start the aggregator with multistatic mode
./target/release/sensing-server --source esp32 --udp-port 5005 --http-port 3000 --ws-port 3001
```

The mesh uses a **Time-Division Multiplexing (TDM)** protocol so nodes take turns transmitting, avoiding self-interference. Key features:

| Feature | Description |
|---------|-------------|
| TDM coordination | Nodes cycle through TX/RX slots (configurable guard intervals) |
| Channel hopping | Automatic 2.4/5 GHz band cycling for multiband fusion |
| QUIC transport | TLS 1.3-encrypted streams on aggregator nodes (ADR-032a) |
| Manual crypto fallback | HMAC-SHA256 beacon auth on constrained ESP32-S3 nodes |
| Attention-weighted fusion | Cross-viewpoint attention with geometric diversity bias |

See [ADR-029](adr/ADR-029-ruvsense-multistatic-sensing-mode.md) and [ADR-032](adr/ADR-032-multistatic-mesh-security-hardening.md) for the full design.

### Cognitum Seed Integration (ADR-069)

Connect an ESP32-S3 to a [Cognitum Seed](https://cognitum.one) (Pi Zero 2 W, ~$15) for persistent vector storage, kNN similarity search, cryptographic witness chain, and AI-accessible sensing via MCP proxy.

**What the Seed adds:**
- **RVF vector store** — Persistent 8-dim feature vectors with content-addressed IDs and kNN search (cosine, L2, dot product)
- **Witness chain** — SHA-256 tamper-evident audit trail for every ingest operation
- **Ed25519 custody** — Device-bound keypair for cryptographic attestation of sensing data
- **Sensor fusion** — BME280 (temp/humidity/pressure), PIR motion, reed switch, 4-ch ADC provide environmental ground truth
- **MCP proxy** — 114 tools via JSON-RPC 2.0 so AI assistants (Claude, GPT) can query sensing state directly
- **Reflex rules** — Automatic alarm triggers based on fragility, drift, and anomaly thresholds

**Setup:**

```bash
# 1. Plug in the Cognitum Seed via USB — appears as a network adapter at 169.254.42.1

# 2. Pair your client (opens a 30-second window, USB-only for security)
curl -sk -X POST https://169.254.42.1:8443/api/v1/pair/window
curl -sk -X POST https://169.254.42.1:8443/api/v1/pair \
  -H 'Content-Type: application/json' -d '{"client_name":"my-laptop"}'
# Save the returned token — it is shown only once

# 3. Provision ESP32 to send features to your laptop (where the bridge runs)
python firmware/esp32-csi-node/provision.py --port COM9 \
  --ssid "YourWiFi" --password "secret" \
  --target-ip 192.168.1.20 --target-port 5006 --node-id 1

# 4. Run the bridge (receives ESP32 UDP, ingests into Seed via HTTPS)
export SEED_TOKEN="your-pairing-token"
python scripts/seed_csi_bridge.py \
  --seed-url https://169.254.42.1:8443 --token "$SEED_TOKEN" \
  --udp-port 5006 --batch-size 10 --validate

# 5. Check Seed status
python scripts/seed_csi_bridge.py --token "$SEED_TOKEN" --stats

# 6. Trigger compaction (reclaim disk space from deleted vectors)
python scripts/seed_csi_bridge.py --token "$SEED_TOKEN" --compact
```

**Feature vector dimensions (magic `0xC5110003`, 48 bytes, 1 Hz):**

| Dim | Feature | Range | Source |
|-----|---------|-------|--------|
| 0 | Presence score | 0.0–1.0 | `s_presence_score / 10.0` |
| 1 | Motion energy | 0.0–1.0 | `s_motion_energy / 10.0` |
| 2 | Breathing rate | 0.0–1.0 | `s_breathing_bpm / 30.0` |
| 3 | Heart rate | 0.0–1.0 | `s_heartrate_bpm / 120.0` |
| 4 | Phase variance | 0.0–1.0 | Mean Welford variance of top-K subcarriers |
| 5 | Person count | 0.0–1.0 | Active persons / 4 |
| 6 | Fall detected | 0.0 or 1.0 | Binary fall flag |
| 7 | RSSI | 0.0–1.0 | `(rssi + 100) / 100` |

**Architecture:**

```
ESP32-S3 ($9)  ──UDP:5006──>  Host (bridge)  ──HTTPS──>  Cognitum Seed ($15)
  CSI @ 100 Hz                seed_csi_bridge.py           RVF vector store
  Features @ 1 Hz            Batches, validates            kNN graph + boundary
  Vitals @ 1 Hz              NaN rejection                 Witness chain
                              Source IP filtering           114-tool MCP proxy
```

See [ADR-069](adr/ADR-069-cognitum-seed-csi-pipeline.md) for the complete design, validation results, and security analysis.

---

## REST API Reference

Base URL: `http://localhost:3000` (Docker) or `http://localhost:8080` (binary default).

| Method | Endpoint | Description | Example Response |
|--------|----------|-------------|-----------------|
| `GET` | `/health` | Server health check | `{"status":"ok","source":"simulated","clients":0}` |
| `GET` | `/api/v1/sensing/latest` | Latest CSI sensing frame (amplitude, phase, motion) | JSON with subcarrier arrays |
| `GET` | `/api/v1/vital-signs` | Breathing rate + heart rate + confidence | `{"breathing_bpm":16.2,"heart_bpm":72.1,"confidence":0.87}` |
| `GET` | `/api/v1/pose/current` | 17 COCO keypoints (x, y, z, confidence) | Array of 17 joint positions |
| `GET` | `/api/v1/info` | Server version, build info, uptime | JSON metadata |
| `GET` | `/api/v1/bssid` | Multi-BSSID WiFi registry | List of detected access points |
| `GET` | `/api/v1/model/layers` | Progressive model loading status | Layer A/B/C load state |
| `GET` | `/api/v1/model/sona/profiles` | SONA adaptation profiles | List of environment profiles |
| `POST` | `/api/v1/model/sona/activate` | Activate a SONA profile for a specific room | `{"profile":"kitchen"}` |
| `GET` | `/api/v1/models` | List available RVF model files | `{"models":[...],"count":0}` |
| `GET` | `/api/v1/models/active` | Currently loaded model (or null) | `{"model":null}` |
| `POST` | `/api/v1/models/load` | Load a model by ID | `{"status":"loaded","model_id":"..."}` |
| `POST` | `/api/v1/models/unload` | Unload the active model | `{"status":"unloaded"}` |
| `DELETE` | `/api/v1/models/:id` | Delete a model file from disk | `{"status":"deleted"}` |
| `GET` | `/api/v1/models/lora/profiles` | List LoRA adapter profiles | `{"profiles":[]}` |
| `POST` | `/api/v1/models/lora/activate` | Activate a LoRA profile | `{"status":"activated"}` |
| `GET` | `/api/v1/recording/list` | List CSI recording sessions | `{"recordings":[...],"count":0}` |
| `POST` | `/api/v1/recording/start` | Start recording CSI frames to JSONL | `{"status":"recording","session_id":"..."}` |
| `POST` | `/api/v1/recording/stop` | Stop the active recording | `{"status":"stopped","duration_secs":...}` |
| `DELETE` | `/api/v1/recording/:id` | Delete a recording file | `{"status":"deleted"}` |
| `GET` | `/api/v1/train/status` | Training run status | `{"phase":"idle"}` |
| `POST` | `/api/v1/train/start` | Start a training run | `{"status":"started"}` |
| `POST` | `/api/v1/train/stop` | Stop the active training run | `{"status":"stopped"}` |
| `POST` | `/api/v1/adaptive/train` | Train adaptive classifier from recordings | `{"success":true,"accuracy":0.85}` |
| `GET` | `/api/v1/adaptive/status` | Adaptive model status and accuracy | `{"loaded":true,"accuracy":0.85}` |
| `POST` | `/api/v1/adaptive/unload` | Unload adaptive model | `{"success":true}` |

### Example: Get Vital Signs

```bash
curl -s http://localhost:3000/api/v1/vital-signs | python -m json.tool
```

```json
{
    "breathing_bpm": 16.2,
    "heart_bpm": 72.1,
    "breathing_confidence": 0.87,
    "heart_confidence": 0.63,
    "motion_level": 0.12,
    "timestamp_ms": 1709312400000
}
```

### Example: Get Pose

```bash
curl -s http://localhost:3000/api/v1/pose/current | python -m json.tool
```

```json
{
    "persons": [
        {
            "id": 0,
            "keypoints": [
                {"name": "nose", "x": 0.52, "y": 0.31, "z": 0.0, "confidence": 0.91},
                {"name": "left_eye", "x": 0.54, "y": 0.29, "z": 0.0, "confidence": 0.88}
            ]
        }
    ],
    "frame_id": 1024,
    "timestamp_ms": 1709312400000
}
```

---

## WebSocket Streaming

Real-time sensing data is available via WebSocket.

**URL:** `ws://localhost:3000/ws/sensing` (same port as HTTP — recommended) or `ws://localhost:3001/ws/sensing` (dedicated WS port).

> **Note:** The `/ws/sensing` WebSocket endpoint is available on both the HTTP port (3000) and the dedicated WebSocket port (3001/8765). The web UI uses the HTTP port so only one port needs to be exposed. The dedicated WS port remains available for backward compatibility.

### Python Example

```python
import asyncio
import websockets
import json

async def stream():
    uri = "ws://localhost:3001/ws/sensing"
    async with websockets.connect(uri) as ws:
        async for message in ws:
            data = json.loads(message)
            persons = data.get("persons", [])
            vitals = data.get("vital_signs", {})
            print(f"Persons: {len(persons)}, "
                  f"Breathing: {vitals.get('breathing_bpm', 'N/A')} BPM")

asyncio.run(stream())
```

### JavaScript Example

```javascript
const ws = new WebSocket("ws://localhost:3001/ws/sensing");

ws.onmessage = (event) => {
    const data = JSON.parse(event.data);
    console.log("Persons:", data.persons?.length ?? 0);
    console.log("Breathing:", data.vital_signs?.breathing_bpm, "BPM");
};

ws.onerror = (err) => console.error("WebSocket error:", err);
```

### curl (single frame)

```bash
# Requires wscat (npm install -g wscat)
wscat -c ws://localhost:3001/ws/sensing
```

---

## Web UI

The built-in Three.js UI is served at `http://localhost:3000/ui/` (Docker) or the configured HTTP port.

**Two visualization modes:**

| Page | URL | Purpose |
|------|-----|---------|
| **Dashboard** | `/ui/index.html` | Tabbed monitoring dashboard with body model, signal heatmap, phase plot, vital signs |
| **Observatory** | `/ui/observatory.html` | Immersive 3D room visualization with cinematic lighting and wireframe figures |

**Dashboard panels:**

| Panel | Description |
|-------|-------------|
| 3D Body View | Rotatable wireframe skeleton with 17 COCO keypoints |
| Signal Heatmap | 56 subcarriers color-coded by amplitude |
| Phase Plot | Per-subcarrier phase values over time |
| Doppler Bars | Motion band power indicators |
| Vital Signs | Live breathing rate (BPM) and heart rate (BPM) |
| Dashboard | System stats, throughput, connected WebSocket clients |

Both UIs update in real-time via WebSocket and auto-detect the sensing server on the same origin.

---

## Dense Point Cloud (Camera + WiFi CSI Fusion)

RuView can generate real-time 3D point clouds by fusing camera depth estimation with WiFi CSI spatial sensing. This creates a spatial model of the environment that updates in real-time.

### Setup

```bash
# Build the pointcloud binary
cd rust-port/wifi-densepose-rs
cargo build --release -p wifi-densepose-pointcloud

# Start the server (auto-detects camera + CSI). Loopback-only by default.
./target/release/ruview-pointcloud serve --bind 127.0.0.1:9880
```

Open `http://localhost:9880` for the interactive Three.js 3D viewer.

> **Security note.** The server exposes live camera, skeleton, vitals, and occupancy over HTTP. The `--bind` flag defaults to `127.0.0.1:9880` (loopback-only). Exposing on `0.0.0.0` or a LAN IP is opt-in — the server logs a warning when it does, but there is no auth/TLS layer. Put a reverse proxy in front if you need remote access.

> **Brain URL.** Observations are POSTed to `http://127.0.0.1:9876` by default. Override via the `RUVIEW_BRAIN_URL` environment variable or the `--brain <url>` flag on `serve` / `train`.

### Sensors

| Sensor | Auto-detected | Data |
|--------|--------------|------|
| Camera (`/dev/video0`) | Yes (Linux UVC) | RGB frames → MiDaS depth → 3D points |
| ESP32 CSI (UDP:3333) | Yes (if provisioned) | ADR-018 binary → occupancy + pose + vitals |
| MiDaS depth server (port 9885) | Optional | GPU-accelerated neural depth estimation |

### Commands

| Command | Description |
|---------|-------------|
| `ruview-pointcloud serve --bind 127.0.0.1:9880` | Start HTTP server + Three.js viewer (loopback-only by default) |
| `ruview-pointcloud demo` | Generate synthetic point cloud (no hardware needed) |
| `ruview-pointcloud capture --output room.ply` | Capture single frame to PLY file |
| `ruview-pointcloud cameras` | List available cameras |
| `ruview-pointcloud train --data-dir ./data [--brain URL]` | Depth calibration + occupancy training (writes under canonicalized `data-dir`; refuses `..` traversal) |
| `ruview-pointcloud csi-test --count 100` | Send test CSI frames (no ESP32 needed) |
| `ruview-pointcloud fingerprint <name> [--seconds 5]` | Record a named CSI room fingerprint for later matching |

### Pipeline Components

1. **ADR-018 Parser** — Decodes ESP32 CSI binary frames from UDP (magic `0xC5110001` raw CSI and `0xC5110006` feature state), extracts I/Q subcarrier amplitudes and phases. Lives in `parser.rs`; unit-tested against hand-rolled test vectors.
2. **Pose (stub)** — 17 COCO keypoint *layout* generated by `heuristic_pose_from_amplitude` from CSI amplitude energy. This is **not** the trained WiFlow model — it is a placeholder so the viewer has a skeleton to render. Wiring to real Candle/ONNX inference from the `wifi-densepose-nn` crate is a planned follow-up.
3. **Vital Signs** — Breathing rate from CSI phase analysis (peak counting on stable subcarrier)
4. **Motion Detection** — CSI amplitude variance over 20 frames, triggers adaptive capture
5. **RF Tomography** — Backprojection from per-node RSSI to 8×8×4 occupancy grid
6. **Camera Depth** — MiDaS monocular depth (GPU) with luminance+edge fallback
7. **Sensor Fusion** — Voxel-grid merging of camera depth + CSI occupancy
8. **Brain Bridge** — Stores spatial observations in the ruOS brain every 60 seconds

### API Endpoints

| Endpoint | Method | Returns |
|----------|--------|---------|
| `/health` | GET | `{"status": "ok"}` |
| `/api/status` | GET | Camera, CSI, pipeline state, vitals, motion |
| `/api/cloud` | GET | Point cloud (up to 1000 points) + pipeline data |
| `/api/splats` | GET | Gaussian splats for Three.js rendering |
| `/` | GET | Interactive Three.js 3D viewer |

### Training

The training pipeline calibrates depth estimation and occupancy detection:

```bash
ruview-pointcloud train --data-dir ~/.local/share/ruview/training --brain http://127.0.0.1:9876
```

This captures frames, runs depth calibration (grid search over scale/offset/gamma), trains occupancy thresholds, exports DPO preference pairs, and submits results to the ruOS brain.

### Output Formats

- **PLY** — Standard 3D point cloud (ASCII, with RGB color)
- **Gaussian Splats** — JSON format for Three.js rendering
- **Brain Memories** — Spatial observations stored as `spatial-observation`, `spatial-motion`, `spatial-vitals`

### Deep Room Scan

Capture a high-quality 3D model of the room:

```bash
# Stop the live server first (frees the camera)
# Then capture 20 frames and process with MiDaS
ruview-pointcloud capture --frames 20 --output room_model.ply
```

Result: 40,000+ voxels at 5cm resolution, 12,000+ Gaussian splats.

### ESP32 Provisioning for CSI

To send CSI data to the pointcloud server:

```bash
python3 firmware/esp32-csi-node/provision.py \
    --port /dev/ttyACM0 \
    --ssid "YourWiFi" --password "YourPassword" \
    --target-ip 192.168.1.123 --target-port 3333 \
    --node-id 1
```

---

## Vital Sign Detection

The system extracts breathing rate and heart rate from CSI signal fluctuations using FFT peak detection.

| Sign | Frequency Band | Range | Method |
|------|---------------|-------|--------|
| Breathing | 0.1-0.5 Hz | 6-30 BPM | Bandpass filter + FFT peak |
| Heart rate | 0.8-2.0 Hz | 40-120 BPM | Bandpass filter + FFT peak |

**Requirements:**
- CSI-capable hardware (ESP32-S3 or research NIC) for accurate readings
- Subject within ~3-5 meters of an access point (up to ~8 m with multistatic mesh)
- Relatively stationary subject (large movements mask vital sign oscillations)

**Signal smoothing:** Vital sign estimates pass through a three-stage smoothing pipeline (ADR-048): outlier rejection (±8 BPM HR, ±2 BPM BR per frame), 21-frame trimmed mean, and EMA with α=0.02. This produces stable readings that hold steady for 5-10+ seconds instead of jumping every frame. See [Adaptive Classifier](#adaptive-classifier) for details.

**Simulated mode** produces synthetic vital sign data for testing.

---

## CLI Reference

The Rust sensing server binary accepts the following flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--source` | `auto` | Data source: `auto`, `simulate`, `wifi`, `esp32` |
| `--http-port` | `8080` | HTTP port for REST API and UI |
| `--ws-port` | `8765` | WebSocket port |
| `--udp-port` | `5005` | UDP port for ESP32 CSI frames |
| `--ui-path` | (none) | Path to UI static files directory |
| `--tick-ms` | `50` | Simulated frame interval (milliseconds) |
| `--benchmark` | off | Run vital sign benchmark (1000 frames) and exit |
| `--train` | off | Train a model from dataset |
| `--dataset` | (none) | Path to dataset directory (MM-Fi or Wi-Pose) |
| `--dataset-type` | `mmfi` | Dataset format: `mmfi` or `wipose` |
| `--epochs` | `100` | Training epochs |
| `--export-rvf` | (none) | Export RVF model container and exit |
| `--save-rvf` | (none) | Save model state to RVF on shutdown |
| `--model` | (none) | Load a trained `.rvf` model for inference |
| `--load-rvf` | (none) | Load model config from RVF container |
| `--progressive` | off | Enable progressive 3-layer model loading |

### Common Invocations

```bash
# Simulated mode with UI (development)
./target/release/sensing-server --source simulate --http-port 3000 --ws-port 3001 --ui-path ../../ui

# ESP32 hardware mode
./target/release/sensing-server --source esp32 --udp-port 5005

# Windows WiFi RSSI
./target/release/sensing-server --source wifi --tick-ms 500

# Run benchmark
./target/release/sensing-server --benchmark

# Train and export model
./target/release/sensing-server --train --dataset data/ --epochs 100 --save-rvf model.rvf

# Load trained model with progressive loading
./target/release/sensing-server --model model.rvf --progressive
```

---

## Observatory Visualization

The Observatory is an immersive Three.js visualization that renders WiFi sensing data as a cinematic 3D experience. It features room-scale props, wireframe human figures, WiFi signal animations, and a live data HUD.

**URL:** `http://localhost:3000/ui/observatory.html`

**Features:**

| Feature | Description |
|---------|-------------|
| Room scene | Furniture, walls, floor with emissive materials and 6-point lighting |
| Wireframe figures | Up to 4 human skeletons with joint pulsation synced to breathing |
| Signal field | Volumetric WiFi wave visualization |
| Live HUD | Heart rate, breathing rate, confidence, RSSI, motion level |
| Auto-detect | Automatically connects to live ESP32 data when sensing server is running |
| Scenario cycling | 6 preset scenarios with smooth transitions (demo mode) |

**Keyboard shortcuts:**

| Key | Action |
|-----|--------|
| `1-6` | Switch scenario |
| `A` | Toggle auto-cycle |
| `P` | Pause/resume |
| `S` | Open settings |
| `R` | Reset camera |

**Live data auto-detect:** When served by the sensing server, the Observatory probes `/health` on the same origin and automatically connects via WebSocket. The HUD badge switches from `DEMO` to `LIVE`. No configuration needed.

---

## Adaptive Classifier

The adaptive classifier (ADR-048) learns your environment's specific WiFi signal patterns from labeled recordings. It replaces static threshold-based classification with a trained logistic regression model that uses 15 features (7 server-computed + 8 subcarrier-derived statistics).

### Signal Smoothing Pipeline

All CSI-derived metrics pass through a three-stage pipeline before reaching the UI:

| Stage | What It Does | Key Parameters |
|-------|-------------|----------------|
| **Adaptive baseline** | Learns quiet-room noise floor, subtracts drift | α=0.003, 50-frame warm-up |
| **EMA + median filter** | Smooths motion score and vital signs | Motion α=0.15; Vitals: 21-frame trimmed mean, α=0.02 |
| **Hysteresis debounce** | Prevents rapid state flickering | 4 frames (~0.4s) required for state transition |

Vital signs use additional stabilization:

| Parameter | Value | Effect |
|-----------|-------|--------|
| HR dead-band | ±2 BPM | Prevents micro-drift |
| BR dead-band | ±0.5 BPM | Prevents micro-drift |
| HR max jump | 8 BPM/frame | Rejects noise spikes |
| BR max jump | 2 BPM/frame | Rejects noise spikes |

### Recording Training Data

Record labeled CSI sessions while performing distinct activities. Each recording captures full sensing frames (features + raw subcarrier amplitudes) at ~10-25 FPS.

```bash
# 1. Record empty room (leave the room for 30 seconds)
curl -X POST http://localhost:3000/api/v1/recording/start \
  -H "Content-Type: application/json" -d '{"id":"train_empty_room"}'
# ... wait 30 seconds ...
curl -X POST http://localhost:3000/api/v1/recording/stop

# 2. Record sitting still (sit near ESP32 for 30 seconds)
curl -X POST http://localhost:3000/api/v1/recording/start \
  -H "Content-Type: application/json" -d '{"id":"train_sitting_still"}'
# ... wait 30 seconds ...
curl -X POST http://localhost:3000/api/v1/recording/stop

# 3. Record walking (walk around the room for 30 seconds)
curl -X POST http://localhost:3000/api/v1/recording/start \
  -H "Content-Type: application/json" -d '{"id":"train_walking"}'
# ... wait 30 seconds ...
curl -X POST http://localhost:3000/api/v1/recording/stop

# 4. Record active movement (jumping jacks, arm waving for 30 seconds)
curl -X POST http://localhost:3000/api/v1/recording/start \
  -H "Content-Type: application/json" -d '{"id":"train_active"}'
# ... wait 30 seconds ...
curl -X POST http://localhost:3000/api/v1/recording/stop
```

Recordings are saved as JSONL files in `data/recordings/`. Filenames must start with `train_` and contain a class keyword:

| Filename pattern | Class |
|-----------------|-------|
| `*empty*` or `*absent*` | absent |
| `*still*` or `*sitting*` | present_still |
| `*walking*` or `*moving*` | present_moving |
| `*active*` or `*exercise*` | active |

### Training the Model

Train the adaptive classifier from your labeled recordings:

```bash
curl -X POST http://localhost:3000/api/v1/adaptive/train
```

The server trains a multiclass logistic regression on 15 features using mini-batch SGD (200 epochs). Training completes in under 1 second for typical recording sets. The trained model is saved to `data/adaptive_model.json` and automatically loaded on server restart.

**Check model status:**

```bash
curl http://localhost:3000/api/v1/adaptive/status
```

**Unload the model (revert to threshold-based classification):**

```bash
curl -X POST http://localhost:3000/api/v1/adaptive/unload
```

### Using the Trained Model

Once trained, the adaptive model runs automatically:

1. Each CSI frame is classified using the learned weights instead of static thresholds
2. Model confidence is blended with smoothed threshold confidence (70/30 split)
3. The model persists across server restarts (loaded from `data/adaptive_model.json`)

**Tips for better accuracy:**

- Record with clearly distinct activities (actually leave the room for "empty")
- Record 30-60 seconds per activity (more data = better model)
- Re-record and retrain if you move the ESP32 or rearrange the room
- The model is environment-specific — retrain when the physical setup changes

### Adaptive Classifier API

| Method | Endpoint | Description |
|--------|----------|-------------|
| `POST` | `/api/v1/adaptive/train` | Train from `train_*` recordings |
| `GET` | `/api/v1/adaptive/status` | Model status, accuracy, class stats |
| `POST` | `/api/v1/adaptive/unload` | Unload model, revert to thresholds |
| `POST` | `/api/v1/recording/start` | Start recording CSI frames |
| `POST` | `/api/v1/recording/stop` | Stop recording |
| `GET` | `/api/v1/recording/list` | List recordings |

---

## Training a Model

The training pipeline is implemented in pure Rust (7,832 lines, zero external ML dependencies).

### Step 1: Obtain a Dataset

The system supports two public WiFi CSI datasets:

| Dataset | Source | Format | Subjects | Environments | Download |
|---------|--------|--------|----------|-------------|----------|
| [MM-Fi](https://ntu-aiot-lab.github.io/mm-fi) | NeurIPS 2023 | `.npy` | 40 | 4 rooms | [GitHub repo](https://github.com/ybhbingo/MMFi_dataset) (Google Drive / Baidu links inside) |
| [Wi-Pose](https://github.com/NjtechCVLab/Wi-PoseDataset) | Entropy 2023 | `.mat` | 12 | 1 room | [GitHub repo](https://github.com/NjtechCVLab/Wi-PoseDataset) (Google Drive / Baidu links inside) |

Download the dataset files and place them in a `data/` directory.

### Step 2: Train

```bash
# From source
./target/release/sensing-server --train --dataset data/ --dataset-type mmfi --epochs 100 --save-rvf model.rvf

# Via Docker (mount your data directory)
# Note: Training mode requires overriding the default entrypoint
docker run --rm \
  -v $(pwd)/data:/data \
  -v $(pwd)/output:/output \
  --entrypoint /app/sensing-server \
  ruvnet/wifi-densepose:latest \
  --train --dataset /data --epochs 100 --export-rvf /output/model.rvf
```

The pipeline runs 10 phases:
1. Dataset loading (MM-Fi `.npy` or Wi-Pose `.mat`)
2. Hardware normalization (Intel 5300 / Atheros / ESP32 -> canonical 56 subcarriers)
3. Subcarrier resampling (114->56 or 30->56 via Catmull-Rom interpolation)
4. Graph transformer construction (17 COCO keypoints, 16 bone edges)
5. Cross-attention training (CSI features -> body pose)
6. **Domain-adversarial training** (MERIDIAN: gradient reversal + virtual domain augmentation)
7. Composite loss optimization (MSE + CE + UV + temporal + bone + symmetry)
8. SONA adaptation (micro-LoRA + EWC++)
9. Sparse inference optimization (hot/cold neuron partitioning)
10. RVF model packaging

### Step 3: Use the Trained Model

```bash
./target/release/sensing-server --model model.rvf --progressive --source esp32
```

Progressive loading enables instant startup (Layer A loads in <5ms with basic inference), with full model loading in the background.

### Cross-Environment Adaptation (MERIDIAN)

Models trained in one room typically lose 40-70% accuracy in a new room due to different WiFi multipath patterns. The MERIDIAN system (ADR-027) solves this with a 10-second automatic calibration:

1. **Deploy** the trained model in a new room
2. **Collect** ~200 unlabeled CSI frames (10 seconds at 20 Hz)
3. The system automatically generates environment-specific LoRA weights via contrastive test-time training
4. No labels, no retraining, no user intervention

MERIDIAN components (all pure Rust, +12K parameters):

| Component | What it does |
|-----------|-------------|
| Hardware Normalizer | Resamples any WiFi chipset to canonical 56 subcarriers |
| Domain Factorizer | Separates pose-relevant from room-specific features |
| Geometry Encoder | Encodes AP positions (FiLM conditioning with DeepSets) |
| Virtual Augmentor | Generates synthetic environments for robust training |
| Rapid Adaptation | 10-second unsupervised calibration via contrastive TTT |

See [ADR-027](adr/ADR-027-cross-environment-domain-generalization.md) for the full design.

### CRV Signal-Line Protocol

The CRV (Coordinate Remote Viewing) signal-line protocol (ADR-033) maps a 6-stage cognitive sensing methodology onto WiFi CSI processing. This enables structured anomaly classification and multi-person disambiguation.

| Stage | CRV Term | WiFi Mapping |
|-------|----------|-------------|
| I | Gestalt | Detrended autocorrelation → periodicity / chaos / transient classification |
| II | Sensory | 6-modality CSI feature encoding (texture, temperature, luminosity, etc.) |
| III | Topology | AP mesh topology graph with link quality weights |
| IV | Coherence | Phase phasor coherence gate (Accept/PredictOnly/Reject/Recalibrate) |
| V | Interrogation | Person-specific signal extraction with targeted subcarrier selection |
| VI | Partition | Multi-person partition with cross-room convergence scoring |

```bash
# Enable CRV in your Cargo.toml
cargo add wifi-densepose-ruvector --features crv
```

See [ADR-033](adr/ADR-033-crv-signal-line-sensing-integration.md) for the full design.

---

## RVF Model Containers

The RuVector Format (RVF) packages a trained model into a single self-contained binary file.

### Export

```bash
./target/release/sensing-server --export-rvf model.rvf
```

### Load

```bash
./target/release/sensing-server --model model.rvf --progressive
```

### Contents

An RVF file contains: model weights, HNSW vector index, quantization codebooks, SONA adaptation profiles, Ed25519 training proof, and vital sign filter parameters.

### Deployment Targets

| Target | Quantization | Size | Load Time |
|--------|-------------|------|-----------|
| ESP32 / IoT | int4 | ~0.7 MB | <5ms |
| Mobile / WASM | int8 | ~6-10 MB | ~200-500ms |
| Field (WiFi-Mat) | fp16 | ~62 MB | ~2s |
| Server / Cloud | f32 | ~50+ MB | ~3s |

---

## Hardware Setup

### ESP32-S3 Mesh

A 3-6 node ESP32-S3 mesh provides full CSI at 20 Hz. Total cost: ~$54 for a 3-node setup.

**What you need:**
- 3-6x ESP32-S3 development boards (~$8 each)
- A WiFi router (the CSI source)
- A computer running the sensing server (aggregator)

**Flashing firmware:**

Pre-built binaries are available at [Releases](https://github.com/ruvnet/RuView/releases):

| Release | What It Includes | Tag |
|---------|-----------------|-----|
| [v0.5.0](https://github.com/ruvnet/RuView/releases/tag/v0.5.0-esp32) | **Stable (recommended)** — mmWave sensor fusion (MR60BHA2/LD2410 auto-detect), 48-byte fused vitals, all v0.4.3.1 fixes | `v0.5.0-esp32` |
| [v0.4.3.1](https://github.com/ruvnet/RuView/releases/tag/v0.4.3.1-esp32) | Fall detection fix ([#263](https://github.com/ruvnet/RuView/issues/263)), 4MB flash ([#265](https://github.com/ruvnet/RuView/issues/265)), watchdog fix ([#266](https://github.com/ruvnet/RuView/issues/266)) | `v0.4.3.1-esp32` |
| [v0.4.1](https://github.com/ruvnet/RuView/releases/tag/v0.4.1-esp32) | CSI build fix, compile guard, AMOLED display, edge intelligence ([ADR-057](../docs/adr/ADR-057-firmware-csi-build-guard.md)) | `v0.4.1-esp32` |
| [v0.3.0-alpha](https://github.com/ruvnet/RuView/releases/tag/v0.3.0-alpha-esp32) | Alpha — adds on-device edge intelligence (ADR-039) | `v0.3.0-alpha-esp32` |
| [v0.2.0](https://github.com/ruvnet/RuView/releases/tag/v0.2.0-esp32) | Raw CSI streaming, TDM, channel hopping, QUIC mesh | `v0.2.0-esp32` |

> **Important:** Always use **v0.4.3.1 or later**. Earlier versions have false fall detection alerts (v0.4.2 and below) and CSI disabled in the build config (pre-v0.4.1).

```bash
# Flash an ESP32-S3 with 8MB flash (most boards)
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write-flash --flash-mode dio --flash-size 8MB --flash-freq 80m \
  0x0 bootloader.bin 0x8000 partition-table.bin \
  0xf000 ota_data_initial.bin 0x20000 esp32-csi-node.bin
```

**4MB flash boards** (e.g. ESP32-S3 SuperMini 4MB): download the 4MB binaries from the [v0.4.3 release](https://github.com/ruvnet/RuView/releases/tag/v0.4.3-esp32) and use `--flash-size 4MB`:

```bash
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write-flash --flash-mode dio --flash-size 4MB --flash-freq 80m \
  0x0 bootloader.bin 0x8000 partition-table-4mb.bin \
  0xF000 ota_data_initial.bin 0x20000 esp32-csi-node-4mb.bin
```

**Provisioning:**

```bash
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFi" --password "YourPassword" --target-ip 192.168.1.20
```

Replace `192.168.1.20` with the IP of the machine running the sensing server.

**Mesh key provisioning (secure mode):**

For multistatic mesh deployments with authenticated beacons (ADR-032), provision a shared mesh key:

```bash
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFi" --password "YourPassword" --target-ip 192.168.1.20 \
  --mesh-key "$(openssl rand -hex 32)"
```

All nodes in a mesh must share the same 256-bit mesh key for HMAC-SHA256 beacon authentication. The key is stored in ESP32 NVS flash and zeroed on firmware erase.

**TDM slot assignment:**

Each node in a multistatic mesh needs a unique TDM slot ID (0-based):

```bash
# Node 0 (slot 0) — first transmitter
python firmware/esp32-csi-node/provision.py --port COM7 --tdm-slot 0 --tdm-total 3

# Node 1 (slot 1)
python firmware/esp32-csi-node/provision.py --port COM8 --tdm-slot 1 --tdm-total 3

# Node 2 (slot 2)
python firmware/esp32-csi-node/provision.py --port COM9 --tdm-slot 2 --tdm-total 3
```

**Edge Intelligence (v0.3.0-alpha, [ADR-039](../docs/adr/ADR-039-esp32-edge-intelligence.md)):**

The v0.3.0-alpha firmware adds on-device signal processing that runs directly on the ESP32-S3 — no host PC needed for basic presence and vital signs. Edge processing is disabled by default for full backward compatibility.

| Tier | What It Does | Extra RAM |
|------|-------------|-----------|
| **0** | Disabled (default) — streams raw CSI to the aggregator | 0 KB |
| **1** | Phase unwrapping, running statistics, top-K subcarrier selection, delta compression | ~30 KB |
| **2** | Everything in Tier 1, plus presence detection, breathing/heart rate, motion scoring, fall detection | ~33 KB |

Enable via NVS (no reflash needed):

```bash
# Enable Tier 2 (full vitals) on an already-flashed node
python firmware/esp32-csi-node/provision.py --port COM7 \
  --ssid "YourWiFi" --password "YourPassword" --target-ip 192.168.1.20 \
  --edge-tier 2
```

Key NVS settings for edge processing:

| NVS Key | Default | What It Controls |
|---------|---------|-----------------|
| `edge_tier` | 0 | Processing tier (0=off, 1=stats, 2=vitals) |
| `pres_thresh` | 50 | Sensitivity for presence detection (lower = more sensitive) |
| `fall_thresh` | 15000 | Fall detection threshold in milli-units (15000 = 15.0 rad/s²). Normal walking is 2-5, real falls are 20+. Raise to reduce false positives. |
| `vital_win` | 300 | How many frames of phase history to keep for breathing/HR extraction |
| `vital_int` | 1000 | How often to send a vitals packet, in milliseconds |
| `subk_count` | 32 | Number of best subcarriers to keep (out of 56) |

When Tier 2 is active, the node sends a 32-byte vitals packet at 1 Hz (configurable) containing presence state, motion score, breathing BPM, heart rate BPM, confidence values, fall flag, and occupancy estimate. The packet uses magic `0xC5110002` and is sent to the same aggregator IP and port as raw CSI frames.

Binary size: 990 KB (8MB flash, 52% free) or 773 KB (4MB flash). v0.5.0 adds mmWave sensor fusion (~12 KB larger).

> **Alpha notice**: Vital sign estimation uses heuristic BPM extraction. Accuracy is best with stationary subjects in controlled environments. Not for medical use.

**Start the aggregator:**

```bash
# From source
./target/release/sensing-server --source esp32 --udp-port 5005 --http-port 3000 --ws-port 3001

# Docker (use CSI_SOURCE environment variable)
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp -e CSI_SOURCE=esp32 ruvnet/wifi-densepose:latest
```

See [ADR-018](../docs/adr/ADR-018-esp32-dev-implementation.md), [ADR-029](../docs/adr/ADR-029-ruvsense-multistatic-sensing-mode.md), and [Tutorial #34](https://github.com/ruvnet/RuView/issues/34).

### Intel 5300 / Atheros NIC

These research NICs provide full CSI on Linux with firmware/driver modifications.

| NIC | Driver | Platform | Setup |
|-----|--------|----------|-------|
| Intel 5300 | `iwl-csi` | Linux | Custom firmware, ~$15 used |
| Atheros AR9580 | `ath9k` patch | Linux | Kernel patch, ~$20 used |

These are advanced setups. See the respective driver documentation for installation.

---

## Camera-Free Pose Training

RuView can train a 17-keypoint COCO pose model **without any camera** by fusing 10 sensor signals from the ESP32 nodes and Cognitum Seed:

| Signal | Source | What it provides |
|--------|--------|-----------------|
| PIR sensor | Seed GPIO 6 | Binary presence ground truth |
| BME280 temperature | Seed I2C | Occupancy proxy (temp rises with people) |
| BME280 humidity | Seed I2C | Breathing confirmation |
| Cross-node RSSI | 2x ESP32 | Rough XY position (triangulation) |
| Vitals stability | ESP32 DSP | Activity level (stable HR = stationary) |
| Temporal CSI patterns | ESP32 DSP | Walk (periodic), sit (stable), empty (flat) |
| kNN clusters | Seed vector store | Natural state groupings |
| Boundary fragility | Seed graph analysis | Regime changes (enter/exit) |
| Reed switch | Seed GPIO 5 | Door open/close events |
| Vibration sensor | Seed GPIO 13 | Footstep detection |

### How It Works

The pipeline generates weak labels from sensor fusion, then trains in 5 phases:

1. **Multi-modal collection** — Syncs CSI frames with Seed sensor events
2. **Weak label generation** — RSSI triangulation for head position, subcarrier asymmetry for hands, vibration for feet
3. **5-keypoint pose proxy** — Trains head/hands/feet positions from fused signals
4. **17-keypoint interpolation** — Derives full COCO skeleton using bone length constraints
5. **Self-refinement** — Bootstraps from confident predictions (3 rounds)

```bash
# With Cognitum Seed connected (all 10 signals):
node scripts/train-camera-free.js \
  --data data/recordings/pretrain-*.csi.jsonl \
  --seed-url https://169.254.42.1:8443 \
  --seed-token "$SEED_TOKEN"

# Without Seed (CSI-only, 3 signals — still works):
node scripts/train-camera-free.js \
  --data data/recordings/pretrain-*.csi.jsonl --no-seed
```

**Output:** 82.8 KB model (8 KB at 4-bit) with 17-keypoint predictions, 0 skeleton violations, LoRA per-node adapters, and EWC protection against forgetting.

See [ADR-071](adr/ADR-071-ruvllm-training-pipeline.md) and the [pretraining tutorial](tutorials/cognitum-seed-pretraining.md) for the full walkthrough.

---

## Camera-Supervised Pose Training (v0.7.0)

For significantly higher accuracy, use a webcam as a **temporary teacher** during training. The camera captures real 17-keypoint poses via MediaPipe, paired with simultaneous ESP32 CSI data. After training, the camera is no longer needed — the model runs on CSI only.

**Result: 92.9% PCK@20** from a 5-minute collection session.

### Requirements

- Python 3.9+ with `mediapipe` and `opencv-python` (`pip install mediapipe opencv-python`)
- ESP32-S3 node streaming CSI over UDP (port 5005)
- A webcam (laptop, USB, or Mac camera via Tailscale)

### Step 1: Capture Camera + CSI Simultaneously

Run both scripts at the same time (in separate terminals):

```bash
# Terminal 1: Record ESP32 CSI
python scripts/record-csi-udp.py --duration 300

# Terminal 2: Capture camera keypoints
python scripts/collect-ground-truth.py --duration 300 --preview
```

Move around naturally in front of the camera for 5 minutes. The `--preview` flag shows a live skeleton overlay.

### Step 2: Align and Train

```bash
# Align camera keypoints with CSI windows
node scripts/align-ground-truth.js \
  --gt data/ground-truth/*.jsonl \
  --csi data/recordings/csi-*.csi.jsonl

# Train (start with lite, scale up as you collect more data)
node scripts/train-wiflow-supervised.js \
  --data data/paired/*.jsonl \
  --scale lite \
  --epochs 50

# Evaluate
node scripts/eval-wiflow.js \
  --model models/wiflow-supervised/wiflow-v1.json \
  --data data/paired/*.jsonl
```

### Scale Presets

| Preset | Params | Training Time | Best For |
|--------|--------|---------------|----------|
| `--scale lite` | 189K | ~19 min | < 1,000 samples (5 min capture) |
| `--scale small` | 474K | ~1 hr | 1K-10K samples |
| `--scale medium` | 800K | ~2 hrs | 10K-50K samples |
| `--scale full` | 7.7M | ~8 hrs | 50K+ samples (GPU recommended) |

See [ADR-079](adr/ADR-079-camera-ground-truth-training.md) for the full design and optimization details.

---

## Pre-Trained Models (No Training Required)

Pre-trained models are available on HuggingFace: **https://huggingface.co/ruvnet/wifi-densepose-pretrained**

Download and start sensing immediately — no datasets, no GPU, no training needed.

### Quick Start with Pre-Trained Models

```bash
# Install huggingface CLI
pip install huggingface_hub

# Download all models
huggingface-cli download ruvnet/wifi-densepose-pretrained --local-dir models/pretrained

# The models include:
#   model.safetensors    — 48 KB contrastive encoder
#   model-q4.bin         — 8 KB quantized (recommended)
#   model-q2.bin         — 4 KB ultra-compact (ESP32 edge)
#   presence-head.json   — presence detection head (100% accuracy)
#   node-1.json          — LoRA adapter for room 1
#   node-2.json          — LoRA adapter for room 2
```

### What the Models Do

The pre-trained encoder converts 8-dim CSI feature vectors into 128-dim embeddings. These embeddings power all 17 sensing applications:

- **Presence detection** — 100% accuracy, never misses, never false alarms
- **Environment fingerprinting** — kNN search finds "states like this one"
- **Anomaly detection** — embeddings that don't match known clusters = anomaly
- **Activity classification** — different activities cluster in embedding space
- **Room adaptation** — swap LoRA adapters for different rooms without retraining

### Retraining on Your Own Data

If you want to improve accuracy for your specific environment:

```bash
# Collect 2+ minutes of CSI from your ESP32
python scripts/collect-training-data.py --port 5006 --duration 120

# Retrain (uses ruvllm, no PyTorch needed)
node scripts/train-ruvllm.js --data data/recordings/*.csi.jsonl

# Benchmark your retrained model
node scripts/benchmark-ruvllm.js --model models/csi-ruvllm
```

---

## Health & Wellness Applications

WiFi sensing can monitor health metrics without any wearable or camera:

```bash
# Sleep quality monitoring (run overnight)
node scripts/sleep-monitor.js --port 5006 --bind 192.168.1.20

# Breathing disorder pre-screening
node scripts/apnea-detector.js --port 5006 --bind 192.168.1.20

# Stress detection via heart rate variability
node scripts/stress-monitor.js --port 5006 --bind 192.168.1.20

# Walking analysis + tremor detection
node scripts/gait-analyzer.js --port 5006 --bind 192.168.1.20

# Replay on recorded data (no live hardware needed)
node scripts/sleep-monitor.js --replay data/recordings/*.csi.jsonl
```

> **Note:** These are pre-screening tools, not medical devices. Consult a healthcare professional for diagnosis.

---

## ruvllm Training Pipeline

All training uses **ruvllm** — a Rust-native ML runtime. No Python, no PyTorch, no GPU drivers required. Runs on any machine with Node.js.

### 5-Phase Training

| Phase | What | Duration (M4 Pro) |
|-------|------|--------------------|
| Contrastive pretraining | Triplet + InfoNCE loss on CSI embeddings | ~5s |
| Task head training | Presence, activity, vitals classifiers | ~10s |
| LoRA refinement | Per-node room adaptation (rank-4) | ~4s |
| TurboQuant quantization | 2/4/8-bit with <0.5% quality loss | <1s |
| EWC consolidation | Prevent catastrophic forgetting | <1s |

```bash
# Basic training
node scripts/train-ruvllm.js --data data/recordings/pretrain-*.csi.jsonl

# Benchmark
node scripts/benchmark-ruvllm.js --model models/csi-ruvllm
```

### Quantization Options

| Bits | Size | Compression | Quality Loss | Use Case |
|------|------|-------------|-------------|----------|
| fp32 | 48 KB | 1x | 0% | Development |
| 8-bit | 16 KB | 4x | <0.01% | Cognitum Seed inference |
| 4-bit | 8 KB | 8x | <0.1% | Recommended for deployment |
| 2-bit | 4 KB | 16x | <1% | ESP32-S3 SRAM (edge inference) |

### Key Features

- **SONA adaptation** — Adapts to new rooms in <1ms without retraining
- **LoRA adapters** — 2,048 parameters per room, hot-swappable
- **EWC protection** — Learns new rooms without forgetting previous ones
- **Deterministic** — Same seed always produces same model (reproducible)
- **10x data augmentation** — Temporal interpolation, noise injection, cross-node blending

---

## Docker Compose (Multi-Service)

For production deployments with both Rust and Python services:

```bash
cd docker
docker compose up
```

This starts:
- Rust sensing server on ports 3000 (HTTP), 3001 (WS), 5005 (UDP)
- Python legacy server on ports 8080 (HTTP), 8765 (WS)

---

## Testing Firmware Without Hardware (QEMU)

You can test the ESP32-S3 firmware on your computer without any physical hardware. The project uses **QEMU** — an emulator that pretends to be an ESP32-S3 chip, running the real firmware code inside a virtual machine on your PC.

This is useful when:
- You don't have an ESP32-S3 board yet
- You want to test firmware changes before flashing to real hardware
- You're running automated tests in CI/CD
- You want to simulate multiple ESP32 nodes talking to each other

### What You Need

**Required:**
- Python 3.8+ (you probably already have this)
- QEMU with ESP32-S3 support (Espressif's fork)

**Install QEMU (one-time setup):**

```bash
# Easiest: use the automated installer (installs QEMU + Python tools)
bash scripts/install-qemu.sh

# Or check what's already installed:
bash scripts/install-qemu.sh --check
```

The installer detects your OS (Ubuntu, Fedora, macOS, etc.), installs build dependencies, clones Espressif's QEMU fork, builds it, and adds it to your PATH. It also installs the Python tools (`esptool`, `pyyaml`, `esp-idf-nvs-partition-gen`).

<details>
<summary>Manual installation (if you prefer)</summary>

```bash
# Build from source
git clone https://github.com/espressif/qemu.git
cd qemu
./configure --target-list=xtensa-softmmu --enable-slirp
make -j$(nproc)
export QEMU_PATH=$(pwd)/build/qemu-system-xtensa

# Install Python tools
pip install esptool pyyaml esp-idf-nvs-partition-gen
```

</details>

**For multi-node testing (optional):**

```bash
# Linux only — needed for virtual network bridges
sudo apt install socat bridge-utils iproute2
```

### The `qemu-cli.sh` Command

All QEMU testing is available through a single command:

```bash
bash scripts/qemu-cli.sh <command>
```

| Command | What it does |
|---------|-------------|
| `install` | Install QEMU (runs the installer above) |
| `test` | Run single-node firmware test |
| `swarm --preset smoke` | Quick 2-node swarm test |
| `swarm --preset standard` | Standard 3-node test |
| `mesh 3` | Multi-node mesh test |
| `chaos` | Fault injection resilience test |
| `fuzz --duration 60` | Run fuzz testing |
| `status` | Show what's installed and ready |
| `help` | Show all commands |

### Your First Test Run

The simplest way to test the firmware:

```bash
# Using the CLI:
bash scripts/qemu-cli.sh test

# Or directly:
bash scripts/qemu-esp32s3-test.sh
```

**What happens behind the scenes:**
1. The firmware is compiled with a "mock CSI" mode — instead of reading real WiFi signals, it generates synthetic test data that mimics real people walking, falling, or breathing
2. The compiled firmware is loaded into QEMU, which boots it like a real ESP32-S3
3. The emulator's serial output (what you'd see on a USB cable) is captured
4. A validation script checks the output for expected behavior and errors

If you already built the firmware and want to skip rebuilding:

```bash
SKIP_BUILD=1 bash scripts/qemu-esp32s3-test.sh
```

To give it more time (useful on slower machines):

```bash
QEMU_TIMEOUT=120 bash scripts/qemu-esp32s3-test.sh
```

### Understanding the Test Output

The test runs 16 checks on the firmware's output. Here's what a successful run looks like:

```
=== QEMU ESP32-S3 Firmware Test (ADR-061) ===

[PASS] Boot: Firmware booted successfully
[PASS] NVS config: Configuration loaded from flash
[PASS] Mock CSI: Synthetic WiFi data generator started
[PASS] Edge processing: Signal analysis pipeline running
[PASS] Frame serialization: Data packets formatted correctly
[PASS] No crashes: No error conditions detected
...

16/16 checks passed
=== Test Complete (exit code: 0) ===
```

**Exit codes explained:**

| Code | Meaning | What to do |
|------|---------|-----------|
| 0 | **PASS** — everything works | Nothing, you're good! |
| 1 | **WARN** — minor issues | Review the output; usually safe to continue |
| 2 | **FAIL** — something broke | Check the `[FAIL]` lines for what went wrong |
| 3 | **FATAL** — can't even start | Usually a missing tool or build failure; check error messages |

### Testing Multiple Nodes at Once (Swarm)

Real deployments use 3-8 ESP32 nodes. The **swarm configurator** lets you simulate multiple nodes on your computer, each with a different role:

- **Sensor nodes** — generate WiFi signal data (like ESP32s placed around a room)
- **Coordinator node** — collects data from all sensors and runs analysis
- **Gateway node** — bridges data to your computer

```bash
# Quick 2-node smoke test (15 seconds)
python3 scripts/qemu_swarm.py --preset smoke

# Standard 3-node test: 2 sensors + 1 coordinator (60 seconds)
python3 scripts/qemu_swarm.py --preset standard

# See what's available
python3 scripts/qemu_swarm.py --list-presets

# Preview what would run (without actually running)
python3 scripts/qemu_swarm.py --preset standard --dry-run
```

**Note:** Multi-node testing with virtual bridges requires Linux and `sudo`. On other systems, nodes use a simpler networking mode where each node can reach the coordinator but not each other.

### Swarm Presets

| Preset | Nodes | Duration | Best for |
|--------|-------|----------|----------|
| `smoke` | 2 | 15s | Quick check that things work |
| `standard` | 3 | 60s | Normal development testing |
| `ci_matrix` | 3 | 30s | CI/CD pipelines |
| `large_mesh` | 6 | 90s | Testing at scale |
| `line_relay` | 4 | 60s | Multi-hop relay testing |
| `ring_fault` | 4 | 75s | Fault tolerance testing |
| `heterogeneous` | 5 | 90s | Mixed scenario testing |

### Writing Your Own Swarm Config

Create a YAML file describing your test scenario:

```yaml
# my_test.yaml
swarm:
  name: my-custom-test
  duration_s: 45
  topology: star       # star, mesh, line, or ring
  aggregator_port: 5005

nodes:
  - role: coordinator
    node_id: 0
    scenario: 0        # 0=empty room (baseline)
    channel: 6
    edge_tier: 2

  - role: sensor
    node_id: 1
    scenario: 2        # 2=walking person
    channel: 6
    tdm_slot: 1

  - role: sensor
    node_id: 2
    scenario: 3        # 3=fall event
    channel: 6
    tdm_slot: 2

assertions:
  - all_nodes_boot           # Did every node start up?
  - no_crashes               # Any error/panic?
  - all_nodes_produce_frames # Is each sensor generating data?
  - fall_detected_by_node_2  # Did node 2 detect the fall?
```

**Available scenarios** (what kind of fake WiFi data to generate):

| # | Scenario | Description |
|---|----------|-------------|
| 0 | Empty room | Baseline with just noise |
| 1 | Static person | Someone standing still |
| 2 | Walking | Someone walking across the room |
| 3 | Fall | Someone falling down |
| 4 | Multiple people | Two people in the room |
| 5 | Channel sweep | Cycling through WiFi channels |
| 6 | MAC filter | Testing device filtering |
| 7 | Ring overflow | Stress test with burst of data |
| 8 | RSSI sweep | Signal strength from weak to strong |
| 9 | Zero-length | Edge case: empty data packet |

**Topology options:**

| Topology | Shape | When to use |
|----------|-------|-------------|
| `star` | All sensors connect to one coordinator | Most common setup |
| `mesh` | Every node can talk to every other | Testing fully connected networks |
| `line` | Nodes in a chain (A → B → C → D) | Testing relay/forwarding |
| `ring` | Chain with ends connected | Testing circular routing |

Run your custom config:

```bash
python3 scripts/qemu_swarm.py --config my_test.yaml
```

### Debugging Firmware in QEMU

If something goes wrong, you can attach a debugger to the emulated ESP32:

```bash
# Terminal 1: Start QEMU with debug support (paused at boot)
qemu-system-xtensa -machine esp32s3 -nographic \
  -drive file=firmware/esp32-csi-node/build/qemu_flash.bin,if=mtd,format=raw \
  -s -S

# Terminal 2: Connect the debugger
xtensa-esp-elf-gdb firmware/esp32-csi-node/build/esp32-csi-node.elf \
  -ex "target remote :1234" \
  -ex "break app_main" \
  -ex "continue"
```

Or use VS Code: open the project, press **F5**, and select **"QEMU ESP32-S3 Debug"**.

### Running the Full Test Suite

For thorough validation before submitting a pull request:

```bash
# 1. Single-node test (2 minutes)
bash scripts/qemu-esp32s3-test.sh

# 2. Multi-node swarm test (1 minute)
python3 scripts/qemu_swarm.py --preset standard

# 3. Fuzz testing — finds edge-case crashes (1-5 minutes)
cd firmware/esp32-csi-node/test
make all CC=clang
make run_serialize FUZZ_DURATION=60
make run_edge FUZZ_DURATION=60
make run_nvs FUZZ_DURATION=60

# 4. NVS configuration matrix — tests 14 config combinations
python3 scripts/generate_nvs_matrix.py --output-dir build/nvs_matrix

# 5. Chaos testing — injects faults to test resilience (2 minutes)
bash scripts/qemu-chaos-test.sh
```

All of these also run automatically in CI when you push changes to `firmware/`.

---

## Troubleshooting

### Docker: "no matching manifest for linux/arm64" on macOS

The `latest` tag supports both amd64 and arm64. Pull the latest image:

```bash
docker pull ruvnet/wifi-densepose:latest
```

If you still see this error, your local Docker may have a stale cached manifest. Try:

```bash
docker pull --platform linux/arm64 ruvnet/wifi-densepose:latest
```

### Docker: "Connection refused" on localhost:3000

Make sure you're mapping the ports correctly:

```bash
docker run -p 3000:3000 -p 3001:3001 ruvnet/wifi-densepose:latest
```

The `-p 3000:3000` maps host port 3000 to container port 3000.

### Docker: No WebSocket data in UI

Add the WebSocket port mapping:

```bash
docker run -p 3000:3000 -p 3001:3001 ruvnet/wifi-densepose:latest
```

### ESP32: "CSI not enabled in menuconfig"

Firmware versions prior to v0.4.1 had `CONFIG_ESP_WIFI_CSI_ENABLED` disabled in the build config. Upgrade to [v0.4.1](https://github.com/ruvnet/RuView/releases/tag/v0.4.1-esp32) or later. If building from source, ensure `sdkconfig.defaults` exists (not just `sdkconfig.defaults.template`). See [ADR-057](../docs/adr/ADR-057-firmware-csi-build-guard.md).

### ESP32: No data arriving

1. Verify firmware is v0.4.1+ (older versions had CSI disabled — see above)
2. Verify the ESP32 is connected to the same WiFi network
3. Check the target IP matches the sensing server machine: `python firmware/esp32-csi-node/provision.py --port COM7 --target-ip <YOUR_IP>`
4. Verify UDP port 5005 is not blocked by firewall
5. Test with: `nc -lu 5005` (Linux) or similar UDP listener

### Build: Rust compilation errors

Ensure Rust 1.75+ is installed (1.85+ recommended):
```bash
rustup update stable
rustc --version
```

### Build: Linux native desktop prerequisites

If you are compiling the Rust workspace on a Debian/Ubuntu-based Linux system, install the native desktop development packages first:

```bash
sudo apt update
sudo apt install -y \
  build-essential pkg-config \
  libglib2.0-dev libgtk-3-dev \
  libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev \
  libwebkit2gtk-4.1-dev
```

Then rerun:

```bash
cargo build --release
```

This is the same Linux pre-step referenced in the Rust source build section and covers the common GTK/WebKit `pkg-config` requirements used by the desktop build.

### Windows: RSSI mode shows no data

Run the terminal as Administrator (required for `netsh wlan` access). Verified working on Windows 10 and 11 with Intel AX201 and Intel BE201 adapters.

### Vital signs show 0 BPM

- Vital sign detection requires CSI-capable hardware (ESP32 or research NIC)
- RSSI-only mode (Windows WiFi) does not have sufficient resolution for vital signs
- In simulated mode, synthetic vital signs are generated after a few seconds of warm-up
- With real ESP32 data, vital signs take ~5 seconds to stabilize (smoothing pipeline warm-up)

### Vital signs jumping around

The server applies a 3-stage smoothing pipeline (ADR-048). If readings are still unstable:
- Ensure the subject is relatively still (large movements mask vital sign oscillations)
- Train the adaptive classifier for your specific environment: `curl -X POST http://localhost:3000/api/v1/adaptive/train`
- Check signal quality: `curl http://localhost:3000/api/v1/sensing/latest` — look for `signal_quality > 0.4`

### Observatory shows DEMO instead of LIVE

- Verify the sensing server is running: `curl http://localhost:3000/health`
- Access Observatory via the server URL: `http://localhost:3000/ui/observatory.html` (not a file:// URL)
- Hard refresh with Ctrl+Shift+R to clear cached settings
- The auto-detect probes `/health` on the same origin — cross-origin won't work

### QEMU: "qemu-system-xtensa: command not found"

QEMU for ESP32-S3 must be built from Espressif's fork — it is not in standard package managers:

```bash
git clone https://github.com/espressif/qemu.git
cd qemu && ./configure --target-list=xtensa-softmmu && make -j$(nproc)
export QEMU_PATH=$(pwd)/build/qemu-system-xtensa
```

Or point to an existing build: `QEMU_PATH=/path/to/qemu-system-xtensa bash scripts/qemu-esp32s3-test.sh`

### QEMU: Test times out with no output

The emulator is slower than real hardware. Increase the timeout:

```bash
QEMU_TIMEOUT=120 bash scripts/qemu-esp32s3-test.sh
```

If there's truly no output at all, the firmware build may have failed. Rebuild without `SKIP_BUILD`:

```bash
bash scripts/qemu-esp32s3-test.sh   # without SKIP_BUILD
```

### QEMU: "esptool not found"

Install it with pip: `pip install esptool`

### QEMU Swarm: "Must be run as root"

Multi-node swarm tests with virtual network bridges require root on Linux. Two options:

1. Run with sudo: `sudo python3 scripts/qemu_swarm.py --preset standard`
2. Skip bridges (nodes use simpler networking): the tool automatically falls back on non-root systems, but nodes can't communicate with each other (only with the aggregator)

### QEMU Swarm: "yaml module not found"

Install PyYAML: `pip install pyyaml`

---

## FAQ

**Q: Do I need special hardware to try this?**
No. Run `docker run -p 3000:3000 ruvnet/wifi-densepose:latest` and open `http://localhost:3000`. Simulated mode exercises the full pipeline with synthetic data.

**Q: Can consumer WiFi laptops do pose estimation?**
No. Consumer WiFi exposes only RSSI (one number per access point), not CSI (56+ complex subcarrier values per frame). RSSI supports coarse presence and motion detection. Full pose estimation requires CSI-capable hardware like an ESP32-S3 ($8) or a research NIC.

**Q: How accurate is the pose estimation?**
Accuracy depends on hardware and environment. With a 3-node ESP32 mesh in a single room, the system tracks 17 COCO keypoints. The core algorithm follows the CMU "DensePose From WiFi" paper ([arXiv:2301.00250](https://arxiv.org/abs/2301.00250)). The MERIDIAN domain generalization system (ADR-027) reduces cross-environment accuracy loss from 40-70% to under 15% via 10-second automatic calibration.

**Q: Does it work through walls?**
Yes. WiFi signals penetrate non-metallic materials (drywall, wood, concrete up to ~30cm). Metal walls/doors significantly attenuate the signal. With a single AP the effective through-wall range is approximately 5 meters. With a 3-6 node multistatic mesh (ADR-029), attention-weighted cross-viewpoint fusion extends the effective range to ~8 meters through standard residential walls.

**Q: How many people can it track?**
Each access point can distinguish ~3-5 people with 56 subcarriers. Multi-AP deployments multiply linearly (e.g., 4 APs cover ~15-20 people). There is no hard software limit; the practical ceiling is signal physics.

**Q: Is this privacy-preserving?**
The system uses WiFi radio signals, not cameras. No images or video are captured or stored. However, it does track human position, movement, and vital signs, which is personal data subject to applicable privacy regulations.

**Q: What's the Python vs Rust difference?**
The Rust implementation (v2) is 810x faster than Python (v1) for the full CSI pipeline. The Docker image is 132 MB vs 569 MB. Rust is the primary and recommended runtime. Python v1 remains available for legacy workflows.

**Q: Can I use an ESP8266 instead of ESP32-S3?**
No. The ESP8266 does not expose WiFi Channel State Information (CSI) through its SDK, has insufficient RAM (~80 KB vs 512 KB), and runs a single-core 80 MHz CPU that cannot handle the signal processing pipeline. The ESP32-S3 is the minimum supported CSI capture device. See [Issue #138](https://github.com/ruvnet/RuView/issues/138) for alternatives including using cheap Android TV boxes as aggregation hubs.

**Q: Does the Windows WiFi tutorial work on Windows 10?**
Yes. Community-tested on Windows 10 (build 26200) with an Intel Wi-Fi 6 AX201 160MHz adapter on a 5 GHz network. All 7 tutorial steps passed with Python 3.14. See [Issue #36](https://github.com/ruvnet/RuView/issues/36) for full test results.

**Q: Can I run the sensing server on an ARM device (Raspberry Pi, TV box)?**
ARM64 deployment is planned ([ADR-046](adr/ADR-046-android-tv-box-armbian-deployment.md)) but not yet available as a pre-built binary. You can cross-compile from source using `cross build --release --target aarch64-unknown-linux-gnu -p wifi-densepose-sensing-server` if you have the Rust cross-compilation toolchain set up.

---

## Further Reading

- [Architecture Decision Records](../docs/adr/) - 48 ADRs covering all design decisions
- [WiFi-Mat Disaster Response Guide](wifi-mat-user-guide.md) - Search & rescue module
- [Build Guide](build-guide.md) - Detailed build instructions
- [RuVector](https://github.com/ruvnet/ruvector) - Signal intelligence crate ecosystem
- [CMU DensePose From WiFi](https://arxiv.org/abs/2301.00250) - The foundational research paper
