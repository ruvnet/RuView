# WiFi DensePose User Guide

WiFi DensePose turns commodity WiFi signals into real-time human pose estimation, vital sign monitoring, and presence detection. This guide walks you through installation, first run, API usage, hardware setup, and model training.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
   - [Docker (Recommended)](#docker-recommended)
   - [From Source (Rust)](#from-source-rust)
   - [From Source (Python)](#from-source-python)
   - [Guided Installer](#guided-installer)
3. [Quick Start](#quick-start)
   - [30-Second Demo (Docker)](#30-second-demo-docker)
   - [Verify the System Works](#verify-the-system-works)
4. [Data Sources](#data-sources)
   - [Simulated Mode (No Hardware)](#simulated-mode-no-hardware)
   - [Windows WiFi (RSSI Only)](#windows-wifi-rssi-only)
   - [ESP32-S3 (Full CSI)](#esp32-s3-full-csi)
5. [REST API Reference](#rest-api-reference)
6. [WebSocket Streaming](#websocket-streaming)
7. [Web UI](#web-ui)
8. [Vital Sign Detection](#vital-sign-detection)
9. [CLI Reference](#cli-reference)
10. [Training a Model](#training-a-model)
11. [RVF Model Containers](#rvf-model-containers)
12. [Hardware Setup](#hardware-setup)
    - [ESP32-S3 Mesh](#esp32-s3-mesh)
    - [Intel 5300 / Atheros NIC](#intel-5300--atheros-nic)
13. [Docker Compose (Multi-Service)](#docker-compose-multi-service)
14. [Troubleshooting](#troubleshooting)
15. [FAQ](#faq)

---

## Prerequisites

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| **OS** | Windows 10, macOS 10.15, Ubuntu 18.04 | Latest stable |
| **RAM** | 4 GB | 8 GB+ |
| **Disk** | 2 GB free | 5 GB free |
| **Docker** (for Docker path) | Docker 20+ | Docker 24+ |
| **Rust** (for source build) | 1.70+ | 1.85+ |
| **Python** (for legacy v1) | 3.8+ | 3.11+ |

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

Image size: ~132 MB. Contains the Rust sensing server, Three.js UI, and all signal processing.

### From Source (Rust)

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose/rust-port/wifi-densepose-rs

# Build
cargo build --release

# Verify (runs 542+ tests)
cargo test --workspace
```

The compiled binary is at `target/release/sensing-server`.

### From Source (Python)

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

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
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose
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
# (--source simulated is the default)

# From source
./target/release/sensing-server --source simulated --http-port 3000 --ws-port 3001
```

### Windows WiFi (RSSI Only)

Uses `netsh wlan` to capture RSSI from nearby access points. No special hardware needed, but capabilities are limited to coarse presence and motion detection (no pose estimation or vital signs).

```bash
# From source (Windows only)
./target/release/sensing-server --source windows --http-port 3000 --ws-port 3001 --tick-ms 500

# Docker (requires --network host on Windows)
docker run --network host ruvnet/wifi-densepose:latest --source windows --tick-ms 500
```

See [Tutorial #36](https://github.com/ruvnet/wifi-densepose/issues/36) for a walkthrough.

### ESP32-S3 (Full CSI)

Real Channel State Information at 20 Hz with 56-192 subcarriers. Required for pose estimation, vital signs, and through-wall sensing.

```bash
# From source
./target/release/sensing-server --source esp32 --udp-port 5005 --http-port 3000 --ws-port 3001

# Docker
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp ruvnet/wifi-densepose:latest --source esp32
```

The ESP32 nodes stream binary CSI frames over UDP to port 5005. See [Hardware Setup](#esp32-s3-mesh) for flashing instructions.

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

**URL:** `ws://localhost:3001/ws/sensing` (Docker) or `ws://localhost:8765/ws/sensing` (binary default).

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

The built-in Three.js UI is served at `http://localhost:3000/` (Docker) or the configured HTTP port.

**What you see:**

| Panel | Description |
|-------|-------------|
| 3D Body View | Rotatable wireframe skeleton with 17 COCO keypoints |
| Signal Heatmap | 56 subcarriers color-coded by amplitude |
| Phase Plot | Per-subcarrier phase values over time |
| Doppler Bars | Motion band power indicators |
| Vital Signs | Live breathing rate (BPM) and heart rate (BPM) |
| Dashboard | System stats, throughput, connected WebSocket clients |

The UI updates in real-time via the WebSocket connection.

---

## Vital Sign Detection

The system extracts breathing rate and heart rate from CSI signal fluctuations using FFT peak detection.

| Sign | Frequency Band | Range | Method |
|------|---------------|-------|--------|
| Breathing | 0.1-0.5 Hz | 6-30 BPM | Bandpass filter + FFT peak |
| Heart rate | 0.8-2.0 Hz | 40-120 BPM | Bandpass filter + FFT peak |

**Requirements:**
- CSI-capable hardware (ESP32-S3 or research NIC) for accurate readings
- Subject within ~3-5 meters of an access point
- Relatively stationary subject (large movements mask vital sign oscillations)

**Simulated mode** produces synthetic vital sign data for testing.

---

## CLI Reference

The Rust sensing server binary accepts the following flags:

| Flag | Default | Description |
|------|---------|-------------|
| `--source` | `auto` | Data source: `auto`, `simulated`, `windows`, `esp32` |
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
./target/release/sensing-server --source simulated --http-port 3000 --ws-port 3001 --ui-path ../../ui

# ESP32 hardware mode
./target/release/sensing-server --source esp32 --udp-port 5005

# Windows WiFi RSSI
./target/release/sensing-server --source windows --tick-ms 500

# Run benchmark
./target/release/sensing-server --benchmark

# Train and export model
./target/release/sensing-server --train --dataset data/ --epochs 100 --save-rvf model.rvf

# Load trained model with progressive loading
./target/release/sensing-server --model model.rvf --progressive
```

---

## Training a Model

The training pipeline is implemented in pure Rust (7,832 lines, zero external ML dependencies).

### Step 1: Obtain a Dataset

The system supports two public WiFi CSI datasets:

| Dataset | Source | Format | Subjects | Environments |
|---------|--------|--------|----------|-------------|
| [MM-Fi](https://mmfi.github.io/) | NeurIPS 2023 | `.npy` | 40 | 4 rooms |
| [Wi-Pose](https://github.com/aiot-lab/Wi-Pose) | AAAI 2024 | `.mat` | 8 | 3 rooms |

Download and place in a `data/` directory.

### Step 2: Train

```bash
# From source
./target/release/sensing-server --train --dataset data/ --dataset-type mmfi --epochs 100 --save-rvf model.rvf

# Via Docker (mount your data directory)
docker run --rm \
  -v $(pwd)/data:/data \
  -v $(pwd)/output:/output \
  ruvnet/wifi-densepose:latest \
  --train --dataset /data --epochs 100 --export-rvf /output/model.rvf
```

The pipeline runs 8 phases:
1. Dataset loading (MM-Fi `.npy` or Wi-Pose `.mat`)
2. Subcarrier resampling (114->56 or 30->56)
3. Graph transformer construction (17 COCO keypoints, 16 bone edges)
4. Cross-attention training (CSI features -> body pose)
5. Composite loss optimization (MSE + CE + UV + temporal + bone + symmetry)
6. SONA adaptation (micro-LoRA + EWC++)
7. Sparse inference optimization (hot/cold neuron partitioning)
8. RVF model packaging

### Step 3: Use the Trained Model

```bash
./target/release/sensing-server --model model.rvf --progressive --source esp32
```

Progressive loading enables instant startup (Layer A loads in <5ms with basic inference), with full model loading in the background.

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
- A computer running the sensing server

**Flashing firmware:**

Pre-built binaries are available at [Releases](https://github.com/ruvnet/wifi-densepose/releases/tag/v0.1.0-esp32).

```bash
# Flash an ESP32-S3 (requires esptool: pip install esptool)
python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write-flash --flash-mode dio --flash-size 4MB \
  0x0 bootloader.bin 0x8000 partition-table.bin 0x10000 esp32-csi-node.bin
```

**Provisioning:**

```bash
python scripts/provision.py --port COM7 \
  --ssid "YourWiFi" --password "YourPassword" --target-ip 192.168.1.20
```

Replace `192.168.1.20` with the IP of the machine running the sensing server.

**Start the aggregator:**

```bash
# From source
./target/release/sensing-server --source esp32 --udp-port 5005 --http-port 3000 --ws-port 3001

# Docker
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp ruvnet/wifi-densepose:latest --source esp32
```

See [ADR-018](../docs/adr/ADR-018-esp32-dev-implementation.md) and [Tutorial #34](https://github.com/ruvnet/wifi-densepose/issues/34).

### Intel 5300 / Atheros NIC

These research NICs provide full CSI on Linux with firmware/driver modifications.

| NIC | Driver | Platform | Setup |
|-----|--------|----------|-------|
| Intel 5300 | `iwl-csi` | Linux | Custom firmware, ~$15 used |
| Atheros AR9580 | `ath9k` patch | Linux | Kernel patch, ~$20 used |

These are advanced setups. See the respective driver documentation for installation.

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

## Troubleshooting

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

### ESP32: No data arriving

1. Verify the ESP32 is connected to the same WiFi network
2. Check the target IP matches the sensing server machine: `python scripts/provision.py --port COM7 --target-ip <YOUR_IP>`
3. Verify UDP port 5005 is not blocked by firewall
4. Test with: `nc -lu 5005` (Linux) or similar UDP listener

### Build: Rust compilation errors

Ensure Rust 1.70+ is installed:
```bash
rustup update stable
rustc --version
```

### Windows: RSSI mode shows no data

Run the terminal as Administrator (required for `netsh wlan` access).

### Vital signs show 0 BPM

- Vital sign detection requires CSI-capable hardware (ESP32 or research NIC)
- RSSI-only mode (Windows WiFi) does not have sufficient resolution for vital signs
- In simulated mode, synthetic vital signs are generated after a few seconds of warm-up

---

## FAQ

**Q: Do I need special hardware to try this?**
No. Run `docker run -p 3000:3000 ruvnet/wifi-densepose:latest` and open `http://localhost:3000`. Simulated mode exercises the full pipeline with synthetic data.

**Q: Can consumer WiFi laptops do pose estimation?**
No. Consumer WiFi exposes only RSSI (one number per access point), not CSI (56+ complex subcarrier values per frame). RSSI supports coarse presence and motion detection. Full pose estimation requires CSI-capable hardware like an ESP32-S3 ($8) or a research NIC.

**Q: How accurate is the pose estimation?**
Accuracy depends on hardware and environment. With a 3-node ESP32 mesh in a single room, the system tracks 17 COCO keypoints. The core algorithm follows the CMU "DensePose From WiFi" paper ([arXiv:2301.00250](https://arxiv.org/abs/2301.00250)). See the paper for quantitative evaluations.

**Q: Does it work through walls?**
Yes. WiFi signals penetrate non-metallic materials (drywall, wood, concrete up to ~30cm). Metal walls/doors significantly attenuate the signal. The effective through-wall range is approximately 5 meters.

**Q: How many people can it track?**
Each access point can distinguish ~3-5 people with 56 subcarriers. Multi-AP deployments multiply linearly (e.g., 4 APs cover ~15-20 people). There is no hard software limit; the practical ceiling is signal physics.

**Q: Is this privacy-preserving?**
The system uses WiFi radio signals, not cameras. No images or video are captured or stored. However, it does track human position, movement, and vital signs, which is personal data subject to applicable privacy regulations.

**Q: What's the Python vs Rust difference?**
The Rust implementation (v2) is 810x faster than Python (v1) for the full CSI pipeline. The Docker image is 132 MB vs 569 MB. Rust is the primary and recommended runtime. Python v1 remains available for legacy workflows.

---

## Further Reading

- [Architecture Decision Records](../docs/adr/) - 24 ADRs covering all design decisions
- [WiFi-Mat Disaster Response Guide](wifi-mat-user-guide.md) - Search & rescue module
- [Build Guide](build-guide.md) - Detailed build instructions
- [RuVector](https://github.com/ruvnet/ruvector) - Signal intelligence crate ecosystem
- [CMU DensePose From WiFi](https://arxiv.org/abs/2301.00250) - The foundational research paper
