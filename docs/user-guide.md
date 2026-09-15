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
   - [Connect Mesh Data to the Dashboard and Observatory](#connect-mesh-data-to-the-dashboard-and-observatory)
   - [Cognitum Spaces activation](#cognitum-spaces-activation)
   - [Cognitum Seed Integration (ADR-069)](#cognitum-seed-integration-adr-069)
5. [REST API Reference](#rest-api-reference)
6. [WebSocket Streaming](#websocket-streaming)
7. [Web UI](#web-ui)
8. [Vital Sign Detection](#vital-sign-detection)
9. [CLI Reference](#cli-reference)
10. [Observatory Visualization](#observatory-visualization)
11. [Loading the Pretrained Model from Hugging Face](#loading-the-pretrained-model-from-hugging-face)
12. [Adaptive Classifier](#adaptive-classifier)
    - [Recording Training Data](#recording-training-data)
    - [Training the Model](#training-the-model)
    - [Using the Trained Model](#using-the-trained-model)
13. [World Model Prediction (OccWorld)](#world-model-prediction-occworld)
14. [Training a Model](#training-a-model)
    - [CRV Signal-Line Protocol](#crv-signal-line-protocol)
14. [RVF Model Containers](#rvf-model-containers)
14. [Perception Certificate Spine (Developer Preview, ADR-300)](#perception-certificate-spine-developer-preview-adr-297)
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
| ESP32-S3 mesh (3-6 boards) | ~$54 | CSI capture for presence, motion, and vital-sign heuristics; live 17-keypoint pose remains below-target and is not a validated capability |
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
cd RuView/v2

# Build
cargo build --release

# Verify (runs 1,400+ tests)
cargo test --workspace --no-default-features
```

The compiled binary is at `target/release/sensing-server`.

### From crates.io (Individual Crates)

The workspace's crates publish independently, so versions vary crate to crate (`wifi-densepose-core` is at 0.3.2, `wifi-densepose-signal` at 0.3.6, etc. as of this writing) — `cargo add` resolves each to its own latest by default, so you don't need to track exact numbers yourself. Add individual crates to your own Rust project:

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

# Coherent wideband RF tomography research crate (ADR-287) — synthetic
# stepped-frequency backprojection reconstruction. SYNTHETIC/L0 evidence
# only; not wired into any sensing pipeline above. See its own README.
cargo add wifi-densepose-sar
```

See the full crate list and dependency order in [CLAUDE.md](../CLAUDE.md#crate-publishing-order).

### Python wheel (pip) — ADR-117

The Python API ships as **two interchangeable PyPI packages** — same
compiled PyO3 wheel under both names; pick whichever import name
reads better in your code:

| PyPI | Install | Latest | Import |
|---|---|---|---|
| [`ruview`](https://pypi.org/project/ruview/) | `pip install ruview` | `2.0.0a1` | `from ruview import ...` |
| [`wifi-densepose`](https://pypi.org/project/wifi-densepose/) | `pip install wifi-densepose` | `2.0.0a1` | `from wifi_densepose import ...` |

```bash
pip install ruview                        # core DSP (~250 KB compiled wheel)
pip install "ruview[client]"              # + asyncio WebSocket + paho-mqtt
```

```python
# vitals
from ruview import BreathingExtractor, HeartRateExtractor
br = BreathingExtractor.esp32_default()   # 56 subcarriers @ 100 Hz, 30s window

# live sensing-server stream
from ruview.client import SensingClient, EdgeVitalsMessage
async with SensingClient("ws://localhost:8765/ws/sensing") as c:
    async for msg in c.stream():
        if isinstance(msg, EdgeVitalsMessage):
            print(msg.breathing_rate_bpm, msg.heartrate_bpm)

# Home Assistant semantic primitives (ADR-115 HA-MIND)
from ruview.client import (
    RuViewMqttClient, SemanticPrimitive, SemanticPrimitiveListener,
)
```

The wheels ship for Linux (x86_64, aarch64 via sdist), macOS (sdist),
and Windows (amd64 wheel). Stable ABI (`abi3-py310`) — one binary
covers Python 3.10+. Multi-arch native wheels are produced by the
[pip-release.yml](../.github/workflows/pip-release.yml) cibuildwheel
matrix on each `v*-pip` tag.

> **Migrating from v1.x?** The legacy `wifi-densepose==1.1.0` FastAPI
> server is end-of-life. `wifi-densepose==1.99.0` is a tombstone that
> raises `ImportError` with a migration URL; upgrade to `>=2.0.0a1`
> (or switch to `ruview`).

To build the wheel from source (e.g. for a local change):

```bash
git clone https://github.com/ruvnet/RuView.git
cd RuView/python
pip install maturin>=1.7
maturin develop --release
pytest tests/                              # 183 tests
pytest bench/ --benchmark-only             # 12 hot-path benchmarks
```

Full API + tests breakdown is on the PyPI front page:
[wifi-densepose on PyPI](https://pypi.org/project/wifi-densepose/) ·
[ruview on PyPI](https://pypi.org/project/ruview/).

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

# Docker
# Note: Docker Desktop on Windows does NOT support `--network host` to access native Windows WiFi adapters. 
# Therefore, `--source wifi` will fail inside a Docker container on Windows. 
# You MUST build from source to use Windows WiFi sensing.
```

> **Community verified:** Tested on Windows 10 (10.0.26200) with Intel Wi-Fi 6 AX201 160MHz, Python 3.14, StormFiber 5 GHz network. All 7 tutorial steps passed with stable RSSI readings at -48 dBm. See [Tutorial #36](https://github.com/ruvnet/RuView/issues/36) for the full walkthrough and test results.

**Vital signs from RSSI:** The sensing server now supports breathing rate estimation from RSSI variance patterns (requires stationary subject near AP) and motion classification with confidence scoring. RSSI-based vital sign detection has lower fidelity than ESP32 CSI — it is best for presence detection and coarse motion classification.

### macOS WiFi (RSSI Only)

Uses CoreWLAN via a Swift helper binary. macOS Sonoma 14.4+ redacts real BSSIDs; the adapter generates deterministic synthetic MACs so the multi-BSSID pipeline still works.

```bash
# Compile the Swift helper (once)
swiftc -O archive/v1/src/sensing/mac_wifi.swift -o mac_wifi

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

### Connect Mesh Data to the Dashboard and Observatory

If a standalone `aggregator` command prints live packets, the ESP32 fleet is already reaching that host. To visualize the same data, stop the standalone aggregator and run `sensing-server` on that same host and UDP port. The sensing server is the aggregator used by the REST API, WebSocket stream, dashboard, and Observatory.

```bash
# From a source build
cd v2
cargo run -p wifi-densepose-sensing-server -- \
  --source esp32 \
  --udp-port 5005 \
  --http-port 3000 \
  --ws-port 3001 \
  --ui-path ../../ui

# Docker
docker run --rm \
  -e CSI_SOURCE=esp32 \
  -p 3000:3000 \
  -p 3001:3001 \
  -p 5005:5005/udp \
  ruvnet/wifi-densepose:latest
```

Open the UI from the sensing server, not from a local file:

| View | URL |
|------|-----|
| Dashboard | `http://localhost:3000/ui/index.html` |
| Observatory | `http://localhost:3000/ui/observatory.html` |

Use these checks before debugging the browser:

```bash
curl http://localhost:3000/health
curl http://localhost:3000/api/v1/nodes
curl http://localhost:3000/api/v1/sensing/latest
```

If the ESP32 nodes are provisioned with `--target-ip <AGGREGATOR_HOST>`, that IP must be the machine running `sensing-server`. Only one process can receive UDP `:5005` at a time, so leave the standalone hardware `aggregator` off while the dashboard or Observatory is live.

### Cognitum Spaces activation

Cognitum Spaces gives RuView a tenant/workspace-scoped semantic world model
without uploading raw RF/CSI, recordings, pose frames, vital waveforms, or
identity observations. It represents sites, buildings, floors, bounded
rooms/spaces, zones, anonymous entities, semantic events, and alerts.

Activate the public RuView OAuth client with Authorization Code + PKCE:

```bash
wifi-densepose login --spaces
wifi-densepose whoami
wifi-densepose spaces --resource sites --limit 50
wifi-densepose spaces --resource events --limit 25
```

The login requests `sensing:read spaces:read`. That consent is read-only: it
does not grant publication, pairing, policy approval, command, or actuator
authority. Versioned collections are `sites`, `buildings`, `floors`,
`spaces`, `zones`, `entities`, `events`, and `alerts`. A returned
`nextCursor` is opaque and valid only for the same collection.

The dependency-free contributor harness exposes the same read path:

```bash
npx @ruvnet/ruview@0.5.0 spaces --resource alerts --limit 25
npx @ruvnet/ruview@0.5.0 mcp start
```

Its MCP tool is `ruview_spaces_list`. MCP reads are OAuth-only, use the fixed
Cognitum API origin, and require the explicit guarded-tool opt-in. The harness
does not accept an arbitrary credential path or API origin.

For service compatibility, `wifi-densepose spaces` can read
`COGNITUM_SPACES_API` at request time. API-key access to a versioned collection
also requires `--workspace <uuid>`; OAuth derives the workspace from the
signed token. Never print or commit either credential.

Every response is bounded and revalidated. Raw-sensing aliases, malformed
hierarchy, non-anonymous person/track entities, invalid timestamps, stale
confidence, and oversized structures fail closed. Empty data means no
authorized state is present; it does not prove that a physical site is empty.

RuVector spatial memory remains physically separated by tenant and workspace.
Agents observe or recommend by default. Any consequential execution requires a
separate policy/grant/approval decision and produces a signed, hash-chained
receipt; the Spaces read token can never satisfy that gate.

See ADR-325, ADR-326, and ADR-327 for the activation, memory, and governed-action
decisions.

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
| `GET` | `/api/v1/mesh` | ADR-110 fleet-wide mesh sync map ([iter 29](adr/ADR-110-esp32-c6-firmware-extension.md)) | `{"nodes":{"9":{...},"12":{...}},"total":2}` |
| `GET` | `/api/v1/nodes/:id/sync` | Single-node mesh sync snapshot (or 404) | `{"offset_us":1163565,"is_leader":false,...}` |
| `GET` | `/api/v1/mesh/metrics` | ADR-110 mesh state in Prometheus exposition format ([iter 36](adr/ADR-110-esp32-c6-firmware-extension.md)) | `wifi_densepose_mesh_offset_us{node="9"} 1163565\n…` |
| `GET` | `/api/field` | ADR-262 P3 — latest **signed RuField `FieldEvent`s** from the live sensing cycle, plus the signer pubkey + a `dev_signing_key` flag. Only egress-safe (P1/P2) events are surfaced; identity/biometric (P4/P5) and raw (P0) are held edge-local | `{"spec":"rufield","signer_pubkey_hex":"…","dev_signing_key":true,"events":[…]}` |

### RuField surface (ADR-262 P3)

RuView's live WiFi-CSI sensing now also speaks the standalone **RuField MFS** wire format. Each governed sensing cycle is converted (via the `wifi-densepose-rufield` anti-corruption bridge) into a **signed** `FieldEvent` (`Modality::WifiCsi`, ed25519 `ProvenanceRef`) and surfaced on two additive endpoints:

- `GET /api/field` — the most recent signed events (JSON).
- `GET /ws/field` — a WebSocket that streams each cycle's signed event (mirrors `/ws/sensing`).

```bash
curl -s http://localhost:3000/api/field | python -m json.tool          # latest signed FieldEvents
python -c "import asyncio,websockets; asyncio.run((lambda: websockets.connect('ws://localhost:8765/ws/field'))())"  # stream
```

Privacy is fail-closed: only egress-safe **P1/P2** events leave the box — raw (P0) and identity/biometric/aggregate (P3–P5) cycles are held **edge-local** and never appear on these endpoints; a no-presence cycle emits **no event**.

**Signing key:** the surface signs with a **dedicated dev/sensing key**, seeded from `WDP_RUFIELD_SIGNING_SEED` (a 64-char hex string or a ≥32-byte value); when unset it falls back to a deterministic dev default and logs a `WARN` (the `dev_signing_key` flag in `/api/field` reflects this). This is a standalone key pending the ADR-262 §8 Q1 key-ownership decision — set `WDP_RUFIELD_SIGNING_SEED` for any real deployment.

> **Honesty (ADR-262 §0/§6):** this is real plumbing on a live endpoint, **not an accuracy claim.** It is the single-link CSI sensing with its existing caveats (no validated room-coordinate accuracy — positions are the "strongest field peak", not calibrated triangulation).

### Example: Get fleet mesh state (ADR-110)

```bash
curl -s http://localhost:3000/api/v1/mesh | python -m json.tool
```

```json
{
    "nodes": {
        "9": {
            "offset_us":       1163565,
            "is_leader":       false,
            "is_valid":        true,
            "smoothed":        true,
            "sequence":        20,
            "csi_fps_ema":     10.0,
            "csi_fps_samples": 47
        },
        "12": {
            "offset_us":       -7,
            "is_leader":       true,
            "is_valid":        true,
            "smoothed":        false,
            "sequence":        20,
            "csi_fps_ema":     10.0,
            "csi_fps_samples": 51
        }
    },
    "total": 2
}
```

Empty `{"nodes": {}, "total": 0}` means no mesh peers reachable.
Nodes that haven't emitted a sync packet yet are omitted from the map.

### Example: Get one node's sync state

```bash
curl -s http://localhost:3000/api/v1/nodes/9/sync | python -m json.tool
```

200 → same `NodeSyncSnapshot` shape as inside `/api/v1/mesh` or the
WebSocket `sync` field. Field meanings are documented under
[Per-node mesh sync (ADR-110)](#per-node-mesh-sync-adr-110).

404 (unknown node):
```json
{"error": "unknown_node", "node_id": 99}
```

404 (node exists but hasn't synced yet):
```json
{
    "error":   "no_sync",
    "node_id": 9,
    "hint":    "node hasn't emitted a sync packet yet (no mesh peer or not v0.6.9+)"
}
```

Useful for Home Assistant REST sensors, Prometheus exporters,
automation rule probes, and curl debugging — anywhere you want
one-shot mesh state without holding a WebSocket connection.

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

### Per-node mesh sync (ADR-110)

Since firmware **v0.7.0-esp32** + sensing-server iter 23, every
`sensing_update` whose nodes participate in the [ADR-110](adr/ADR-110-esp32-c6-firmware-extension.md)
ESP-NOW mesh carries an optional `sync` object per node:

```json
{
  "type": "sensing_update",
  "nodes": [
    {
      "node_id": 9,
      "rssi_dbm": -38.0,
      "amplitude": [...],
      "subcarrier_count": 64,
      "sync": {
        "offset_us":       1163565,
        "is_leader":       false,
        "is_valid":        true,
        "smoothed":        true,
        "sequence":        20,
        "csi_fps_ema":     10.0,
        "csi_fps_samples": 47
      }
    }
  ]
}
```

Field meanings:

| Field | Type | Meaning |
|---|---|---|
| `offset_us` | i64 | Smoothed local-vs-mesh clock offset in microseconds. Negative when this node is behind the leader. §A0.10 on the bench measured ~1.16 s boot delta between two C6 boards. |
| `is_leader` | bool | True when this node is the elected mesh leader (lowest EUI-64 in the cohort). |
| `is_valid` | bool | True when this node has heard a fresh leader beacon within the firmware's `VALID_WINDOW_MS = 3 s` freshness gate. |
| `smoothed` | bool | True once the firmware-side EMA filter has seeded (after ~8 beacons ≈ 0.8 s of follower mode). |
| `sequence` | u32 | High-water CSI sequence number stamped when this sync packet was emitted. Pair with the per-frame `sequence` field on incoming CSI to interpolate a mesh-aligned timestamp for any frame. |
| `csi_fps_ema` | f64 | Per-node EMA of the observed CSI frame rate. Bench typical ≈ 10 Hz. |
| `csi_fps_samples` | u32 | How many inter-frame deltas the EMA has seen. Treat values < 5 as "not yet trustworthy" and fall back to 20 Hz. |
| `staleness_ms` | u64 (optional) | Milliseconds since the host last received a sync packet from this node ([iter 34](adr/ADR-110-esp32-c6-firmware-extension.md)). Fade UI badges after 5 000 ms; treat ≥ 9 000 ms as the same condition that the firmware's `c6_sync_espnow_is_valid()` reports as `false`. |

**When `sync` is omitted entirely**: the node isn't on the mesh (or
hasn't heard a peer yet). Non-ESP32 paths — multi-BSSID router scan,
synthetic-RSSI fallback, simulation — also omit `sync`. Existing
pre-iter-23 UI clients ignore the new field naturally because they
don't read it.

**How to render this in a UI**:
- `is_leader === true` → badge the node "Leader"
- `is_valid === false` → grey out / "Sync lost"
- `csi_fps_samples < 5` → label as "Calibrating" until ≥5 frames
- `|offset_us|` trend → render a jitter histogram to show the §A0.10
  EMA suppression working live

**How to recover a mesh-aligned timestamp for any CSI frame from this
node**: take the frame's own `sequence` u32, subtract `sync.sequence`,
divide by `sync.csi_fps_ema` (or 20.0 if `csi_fps_samples < 5`),
multiply by 1 000 000 µs — that's the mesh delta from the sync emit
time. Use it to align multistatic frames from sibling boards.

---

## Home Assistant + Matter integration

Full design + operator guide: [`docs/integrations/home-assistant.md`](integrations/home-assistant.md) (ADR-115).

### 30-second Mosquitto-add-on flow

1. Inside Home Assistant, install the **Mosquitto broker** add-on from the Add-on Store and start it.
2. In HA, **Settings → Devices & Services → Add Integration → MQTT**, point at the broker.
3. Start the sensing-server with MQTT:

   ```bash
   docker run --rm --net=host ruvnet/wifi-densepose:0.7.0 \
       --source esp32 --mqtt --mqtt-host <ha-host-ip>
   ```
4. Within ~5 seconds HA auto-creates one **device** per RuView node with 21 entities: 11 raw signals (presence, person count, HR, BR, motion, fall, RSSI, zones, pose, …) plus 10 semantic primitives (someone-sleeping, possible-distress, room-active, elderly-inactivity-anomaly, meeting, bathroom, fall-risk, bed-exit, no-movement, multi-room-transition).

### Privacy mode for healthcare / AAL

```bash
sensing-server --mqtt --mqtt-host <broker> --mqtt-tls --privacy-mode
```

`--privacy-mode` strips heart rate, breathing rate, and pose keypoints from MQTT **and** Matter — they never reach the wire. Semantic primitives stay published because they're inferred *states* server-side, not biometric *values*. This is the architectural win that makes ADR-115 healthcare- and enterprise-deployable.

### Matter Bridge (Apple Home / Google Home / Alexa / SmartThings)

```bash
sensing-server --matter --matter-setup-file /var/run/ruview-matter.txt
```

Open `/var/run/ruview-matter.txt` for the Matter pairing QR / 11-digit setup code. Scan it from Apple Home / Google Home / your HA Matter integration. RuView appears as a Bridged Device with one occupancy endpoint per node + per zone, plus a momentary switch for fall events.

Detailed entity reference, blueprint catalog, troubleshooting recipe matrix: see [`docs/integrations/home-assistant.md`](integrations/home-assistant.md).

### BFLD — privacy-gated WiFi BFI sensing layer (ADR-118)

The `wifi-densepose-bfld` crate adds an explicit privacy-gating layer on top of the sensing pipeline. It ingests 802.11ac/ax Beamforming Feedback Information (BFI) and emits bounded, classified sensing events that HA / Matter / MQTT consumers can read **without** leaking identity-discriminative data.

Three structural invariants enforced by the type system:

- **I1** — Raw BFI never exits the node (`Sink` marker-trait hierarchy)
- **I2** — Identity embedding is in-RAM-only (no `Serialize`/`Clone`/`Copy`; `Drop` zeroizes)
- **I3** — Cross-site identity correlation is cryptographically impossible (per-site BLAKE3-keyed hash + daily epoch rotation)

#### Minimal operator quickstart

Two runnable examples ship with the crate:

```bash
# In-process consumer: build pipeline, send one frame, print event JSON
cargo run -p wifi-densepose-bfld --example bfld_minimal

# Worker thread + HA-DISCO: full publish lifecycle (availability + discovery + state + LWT)
cargo run -p wifi-densepose-bfld --example bfld_handle
```

#### Production publish lifecycle (HA-DISCO + MQTT)

```rust
// Bootstrap (once at startup, retain=true messages):
publish_availability_online(&mut retained_pub, "seed-01")?;
publish_discovery(&mut retained_pub, "seed-01", PrivacyClass::Anonymous)?;

// Per-frame:
let handle = BfldPipelineHandle::spawn(pipeline, state_pub);
handle.send(PipelineInput { inputs, embedding })?;
```

Six HA entities are auto-created per node (`binary_sensor.*_bfld_presence`, `sensor.*_bfld_motion`/`person_count`/`zone_activity`/`confidence`/`identity_risk`). The `identity_risk` entity is **only present at `PrivacyClass::Anonymous`**; class `Restricted` deployments (care homes, regulated environments) drop it entirely from both discovery and state topics.

#### Three operator HA blueprints

Under `v2/crates/cog-ha-matter/blueprints/bfld/`:

- `presence-lighting.yaml` — `binary_sensor.*_bfld_presence` ⇒ `light.turn_on/off` with configurable hold time
- `motion-hvac.yaml` — `sensor.*_bfld_motion > threshold` ⇒ `climate.set_temperature` ΔT
- `identity-risk-anomaly.yaml` — rolling 7-day z-score notification (requires HA Statistics helper)

Import via HA UI: Settings → Automations & Scenes → Blueprints → Import.

#### Privacy class deployment matrix

| Class | Identity fields | Use case |
|-------|-----------------|----------|
| `Raw` | full BFI matrix | local-only research (never networked) |
| `Derived` | downsampled angles + risk score | operator-acknowledged LAN research mode |
| `Anonymous` (default) | aggregate sensing only + risk score + rotating hash | production HA / Matter deployments |
| `Restricted` | aggregate sensing only, identity fields stripped | care homes, GDPR/HIPAA-style regulated environments |

The `enable_privacy_mode()` runtime toggle on `BfldPipeline` engages `Restricted` from any baseline without restarting the pipeline — useful for security-incident response.

#### MQTT topic tree

```
ruview/<node_id>/bfld/availability         online / offline
ruview/<node_id>/bfld/presence/state       true / false
ruview/<node_id>/bfld/motion/state         0.000000..1.000000
ruview/<node_id>/bfld/person_count/state   integer
ruview/<node_id>/bfld/confidence/state     0.000000..1.000000
ruview/<node_id>/bfld/zone_activity/state  "<zone_name>"  (if configured)
ruview/<node_id>/bfld/identity_risk/state  0.000000..1.000000  (class 2 only)
```

The `rumqttc 0.24` (`use-rustls`) backend ships behind the `mqtt` feature; `RumqttPublisher::connect_with_lwt(node_id, opts, capacity)` pre-configures the Last Will and Testament so the broker auto-publishes `"offline"` on session drop.

Detailed surface: [`v2/crates/wifi-densepose-bfld/README.md`](../v2/crates/wifi-densepose-bfld/README.md), [`docs/research/BFLD/`](research/BFLD/) (11 files, 13,544 words), [ADR-118 through ADR-123](adr/ADR-118-bfld-beamforming-feedback-layer-for-detection.md).

### SENSE-BRIDGE — rvagent MCP server for AI agents (ADR-124)

`@ruvnet/rvagent` is a dual-transport MCP server that makes RuView sensing primitives callable by Claude Code, Cursor, and ruflo swarms without bespoke HTTP client code.

**Install (Claude Code)**:

```bash
claude mcp add rvagent -- npx @ruvnet/rvagent stdio
# With a remote sensing-server:
RUVIEW_SENSING_SERVER_URL=http://cognitum-v0:3000 claude mcp add rvagent -- npx @ruvnet/rvagent stdio
```

**Available tools (6 of 20 in v0.1.0)**:

| Tool | Returns |
|------|---------|
| `ruview.presence.now` | `present`, `n_persons`, `confidence`, `timestamp_ms` |
| `ruview.vitals.get_breathing` | `breathing_rate_bpm` (null if unavailable), `confidence` |
| `ruview.vitals.get_heart_rate` | `heartrate_bpm` (null if unavailable), `confidence` |
| `ruview.vitals.get_all` | Full `EdgeVitalsMessage` (all vitals in one call) |
| `ruview.bfld.last_scan` | `identity_risk_score`, `privacy_class`, `n_frames`, `timestamp_ms` |
| `ruview.bfld.subscribe` | `subscription_id`, `expires_at`, `topic` (MQTT wildcard) |

**Streamable HTTP** (for remote ruflo swarms):

```bash
RVAGENT_HTTP_TOKEN=secret npx @ruvnet/rvagent http --port 3001
# POST JSON-RPC to http://127.0.0.1:3001/mcp
# Cross-origin requests are rejected with 403; missing/wrong token → 401.
```

Source: [`tools/ruview-mcp/`](../tools/ruview-mcp/README.md). Tracking issue: [#787](https://github.com/ruvnet/RuView/issues/787). Full ADR: [ADR-124](adr/ADR-124-rvagent-mcp-ruvector-npm-integration.md).

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
cd v2
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
| `--export-rvf` | (none) | Export a **placeholder** RVF container-format demo and exit — **not a trained model**. For a real model use `--train` (+ `--save-rvf`) or download a pretrained encoder. |
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

## Loading the Pretrained Model from Hugging Face

A pretrained CSI encoder + presence-detection head is published on Hugging Face at [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained). It was trained on 60,630 frames / 610,615 contrastive triplets (12.2M steps, final loss 0.065) and reports **82.3% held-out temporal-triplet accuracy** (the older "100% presence" figure was measured on a single-class recording and has been retracted) and ~164k embeddings/sec on an Apple M4 Pro.

> **Results & proof.** The SOTA 17-keypoint pose model is published separately at [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) — **82.69% torso-PCK@20** on MM-Fi (83.59% ensemble + TTA), beating MultiFormer (72.25%) and CSI2Pose (68.41%). Browse the auditable [AetherArena leaderboard Space](https://huggingface.co/spaces/ruvnet/aether-arena), the full [MM-Fi study](benchmarks/mmfi-wifi-sensing-study.md), and the [efficiency frontier](benchmarks/wifi-pose-efficiency-frontier.md). Reproduce the deterministic pipeline proof with `python archive/v1/data/proof/verify.py` (must print `VERDICT: PASS`; see [ADR-168 benchmark proof](adr/ADR-168-benchmark-proof.md) and [WITNESS-LOG-028](WITNESS-LOG-028.md)).

What it ships (and what it does not):

| Capability | Status |
|------------|--------|
| Presence detection (occupied / empty) | ✅ Trained head — v2 encoder reports 82.3% held-out temporal-triplet acc (v1's "100% on validation" was a single-class recording — retracted, [#882](https://github.com/ruvnet/RuView/issues/882)) |
| 128-dim CSI embeddings (re-ID, similarity, downstream training) | ✅ Trained encoder |
| Single-person breathing / heart-rate | ⚠️ Server still uses heuristic DSP — model does not replace this yet |
| 17-keypoint full-body pose | 🔬 This HF bundle ships no keypoint head — but real pose weights exist elsewhere; see the tier table below |

### Model weights: what's real, what's not

"WiFi → pose" means three different things in this repo, at three different maturity
levels. Read the label, not the headline ([ADR-187](adr/ADR-187-archive-v1-deprecation-honest-labeling.md)):

| Tier | Checkpoint(s) | Honest status |
|------|---------------|---------------|
| **Real & validated** | [`ruvnet/wifi-densepose-pretrained`](https://huggingface.co/ruvnet/wifi-densepose-pretrained) (encoder + presence head) · [`ruvnet/wifi-densepose-mmfi-pose`](https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose) (17-keypoint pose) · `cog-person-count/count_v1` | **MEASURED / published.** Presence = 82.3% held-out temporal-triplet accuracy (the old "100% presence" figure was retracted); MM-Fi pose = 82.69% torso-PCK@20 on the `random_split` protocol. |
| **Real but weak (honestly labeled)** | committed `v2/crates/cog-pose-estimation/cog/artifacts/pose_v1.safetensors` | First-cut on-device model. **PCK@20 = 3.0% / PCK@50 = 18.5%** on a 217-sample holdout — **below the ADR-079 target of ≥ 35%.** Learns coarse structure (`r_hip` 77% PCK@50); distal/face joints near-random. Its runtime path in `cog-pose-estimation/src/inference.rs` is still a centred-skeleton **stub returning `confidence=0`**. Full disclosure in the [cog README](../v2/crates/cog-pose-estimation/cog/README.md). Do not advertise the live single-ESP32 17-keypoint feature without this caveat. |
| **Architecture only, no weights** | `archive/v1` `DensePoseHead` | Random `kaiming_normal_` init, **no checkpoint of any kind** (zero `.pth`/`.onnx`/`.safetensors` files under `archive/v1/`). Deprecated and superseded — see [`archive/v1/DEPRECATED.md`](../archive/v1/DEPRECATED.md). Do not expect real pose output from it. |

**Does it actually run, and can a single ESP32 do pose? ([#509](https://github.com/ruvnet/RuView/issues/509), [#1125](https://github.com/ruvnet/RuView/issues/1125))** Yes, it runs, and the results are reproducible: the deterministic signal-pipeline proof (`python archive/v1/data/proof/verify.py`, must print `VERDICT: PASS`), the committed pose training dump (`v2/crates/cog-pose-estimation/cog/artifacts/train_results.json`), and the auditable MM-Fi arena all back specific numbers. But a single-antenna, 56-subcarrier CSI stream at a 20-frame window does *not* carry the fine-grained spatial information the multi-antenna NIC research relies on — so the shippable pose accuracy the project stands behind today is the **MM-Fi benchmark number**, not a live single-ESP32 number. The path to a first reproducible on-device baseline (PCK@20 ≥ 35%) is tracked in [ADR-079](adr/ADR-079-camera-ground-truth-training.md) / [#645](https://github.com/ruvnet/RuView/issues/645).

### Model and capture compatibility

These artifacts share a pose objective, but not an input contract or adapter format. A checkpoint
is usable only when capture preprocessing, tensor shape, architecture, and runtime all match.

| Artifact/path | Required input | Measured status | Live ESP32 compatibility |
|---------------|----------------|-----------------|--------------------------|
| MM-Fi flagship and `micro` transformer | `[N,3,114,10]` amplitude | **MEASURED** on MM-Fi `random_split`; calibration reference is Python `.npz` LoRA | No direct S3/C6 path is validated; resampling a 56-tone SISO stream does not recreate three-antenna MM-Fi input |
| Cog `pose_v1.safetensors` | `[N,56,20]` amplitude | **MEASURED** PCK@20 = 3.0%, below the ≥35% target; cog-format LoRA is `.safetensors` | Shape matches the canonical window, but the documented live runtime remains below-target/stub and is not a reliable pose claim |
| Viewer `heuristic_pose_from_amplitude` | live canonical amplitude | Skeleton-layout placeholder, not a trained pose model | Renders a demonstrator skeleton only; it is not pose accuracy evidence |
| MERIDIAN automatic unlabeled calibration | proposed ~200-frame target-room capture | **PROPOSED** in ADR-027; no validated end-to-end command | Not available. The current calibration tools require paired CSI/keypoint labels and model-specific inputs |

Calibration files are not interchangeable: `calibrate.py` targets the MM-Fi transformer, while
`cog_calibrate.py` targets the cog conv+MLP. See the
[calibration reference](../aether-arena/calibration/README.md) for their exact schemas.

### Download

```bash
pip install huggingface_hub
huggingface-cli download ruvnet/wifi-densepose-pretrained \
    --local-dir models/wifi-densepose-pretrained
```

The download yields a small set of files (the `.rvf.jsonl` is the canonical container the sensing server reads):

```
models/wifi-densepose-pretrained/
  model.rvf.jsonl       # RVF container (encoder + presence head + lora)
  model.safetensors     # 48 KB — same encoder weights, safetensors format
  model-q4.bin          # 8 KB — recommended quantization for edge
  presence-head.json    # presence classifier head
  config.json           # sona-lora rank=8 alpha=16, target encoder + task_heads
```

### Using the weights

The HF artifact is in **JSONL RVF** format (one JSON object per line: `metadata`, `encoder`, `lora`). What you can do with it today:

| Consumer | Format it reads | Status |
|----------|-----------------|--------|
| Python / PyTorch training pipeline | `model.safetensors` | ✅ Works — load with `safetensors.torch.load_file` |
| RVF JSONL inspection / re-export | `model.rvf.jsonl` | ✅ Works — plain JSONL, parse line-by-line |
| Sensing-server `--model <PATH>` flag | binary RVF (`RVFS` magic) | ⚠️ Does **not** accept the JSONL file yet — see gap below |

**Known gap (tracked):** `v2/crates/wifi-densepose-sensing-server/src/rvf_container.rs` only parses the binary RVF segment format (magic `0x52564653`). Pointing `--model` at `model.rvf.jsonl` causes the progressive loader to error with `invalid magic at offset 0: expected 0x52564653, got 0x7974227B` (`0x7974227B` is the ASCII bytes `{"ty…` from the JSONL header), and the live pipeline degrades to null output rather than falling back to heuristic mode. Until a JSONL adapter lands (or the model is re-published as binary RVF), run the sensing-server **without** `--model` and consume the HF weights from Python or the training pipeline.

```bash
# Works today — Python side (training, evaluation, embedding extraction):
python -c "
from safetensors.torch import load_file
state = load_file('models/wifi-densepose-pretrained/model.safetensors')
print({k: tuple(v.shape) for k, v in state.items()})
"

# Sensing server — run heuristic for now:
cargo run -p wifi-densepose-sensing-server --release -- \
    --source esp32 --udp-port 5005 --http-port 3000
```

See [RVF Model Containers](#rvf-model-containers) for the binary format the loader expects, and [Training a Model](#training-a-model) for using the encoder as a starting point for environment-specific fine-tuning.

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

## World Model Prediction (OccWorld)

RuView integrates [OccWorld](https://github.com/wzzheng/OccWorld) (ECCV 2024) to predict
future 3D occupancy from WiFi CSI — extending the Kalman tracker's 5-frame horizon to
15 predicted frames (~7 s). See [ADR-147](adr/ADR-147-nvidia-cosmos-world-foundation-model-integration.md)
and the [benchmark proof](adr/ADR-168-benchmark-proof.md) for full details.

**Hardware requirement:** NVIDIA GPU with ≥4 GB VRAM (validated: RTX 5080 at 209 ms / 3.4 GB).

**Start the inference server:**
```bash
# Requires ml-env with PyTorch 2.7+ and mmcv/mmdet3d installed (see ADR-147 §3)
~/ml-env/bin/python3 scripts/occworld_server.py /tmp/occworld.sock
```

The Rust crate `wifi-densepose-worldmodel` connects over that Unix socket and injects
trajectory priors into the pose tracker automatically when the server is running.

**Accumulate training data and fine-tune for your space (improves prediction accuracy):**
```bash
# 1. Record WorldGraph snapshots while people move through the space (~1 hour minimum)
python3 scripts/occworld_retrain.py record \
    --server http://localhost:8080 \
    --out-dir /tmp/snapshots/scene_live \
    --duration 3600

# 2. Fine-tune VQVAE tokenizer on indoor occupancy
python3 scripts/occworld_retrain.py vqvae \
    --snapshots /tmp/snapshots/ \
    --work-dir out/ruview_vqvae

# 3. Fine-tune autoregressive transformer
python3 scripts/occworld_retrain.py transformer \
    --snapshots /tmp/snapshots/ \
    --vqvae-checkpoint out/ruview_vqvae/latest.pth \
    --work-dir out/ruview_occworld

# 4. Restart the server with your checkpoint
~/ml-env/bin/python3 scripts/occworld_server.py /tmp/occworld.sock out/ruview_occworld/latest.pth
```

`scripts/ruview_occ_dataset.py` is the domain adapter used internally by the retraining
pipeline — it converts WorldGraph JSON snapshots to OccWorld-format tensors with indoor
class remapping and zero ego-poses. See ADR-147 Phase 3 for details.

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
  --train --dataset /data --epochs 100 --save-rvf /output/model.rvf
```

The pipeline runs 10 phases:
1. Dataset loading (MM-Fi `.npy` or Wi-Pose `.mat`)
2. Hardware normalization (Intel 5300 / Atheros / ESP32 -> canonical 56 subcarriers)
3. Subcarrier resampling (114->56 or 30->56 via Catmull-Rom interpolation)
4. Graph transformer construction (17 COCO keypoints, 16 bone edges)
5. Cross-attention training (CSI features -> body pose)
6. Experimental domain-adversarial components (MERIDIAN research modules; not a validated automatic deployment path)
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

Models trained in one room can lose substantial accuracy in a new room because the multipath
distribution changes. ADR-027 proposes an automatic adaptation design, and the Rust tree contains
individual research components, but RuView does **not** currently provide a validated command that
turns ~200 unlabeled frames into a working room adapter.

Current, testable calibration is the separate **labeled** reference in
`aether-arena/calibration/`: collect paired CSI/keypoint samples, then fit a model-specific LoRA
adapter. The MM-Fi transformer expects `[N,3,114,10]`; the cog expects `[N,56,20]`. Neither adapter
loads into the other model, and neither result establishes live ESP32 pose accuracy without a
leakage-free held-out capture and mean-pose baseline.

ADR-027 research components:

| Component | What it does |
|-----------|-------------|
| Hardware Normalizer | Resamples any WiFi chipset to canonical 56 subcarriers |
| Domain Factorizer | Separates pose-relevant from room-specific features |
| Geometry Encoder | Encodes AP positions (FiLM conditioning with DeepSets) |
| Virtual Augmentor | Generates synthetic environments for robust training |
| Rapid Adaptation | Proposed unlabeled contrastive TTT; not wired as a validated deployment workflow |

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

## Perception Certificate Spine (Developer Preview, ADR-300)

RuView's perception substrate program (ADR-300) is building a `signal → observation →
calibration → inference → uncertainty → evidence → certificate → policy → governed
action` pipeline, where a downstream consumer either gets a calibrated, provenance-backed
answer or an explicit `UNKNOWN` — never a confident-looking guess outside the sensor's
proven operating envelope.

**Status: developer preview, now wired at the crate level.** Phase 1 shipped nine new
crates. `ruview-certify` and `ruview-policy` now depend on `ruview-ood` and provide a
real adapter (`impl From<ruview_ood::DomainState> for _`) plus a composed entry point,
`ruview_policy::authorize_from_certificate`, that takes a real signed
`CapabilityCertificate` and a real `ruview_ood::DomainState` and drives them through
`authorize()` — not a hand-built `AssuranceInputs`. A cross-crate integration test
(`ruview-policy`'s `acceptance_test_b_real_integration` module) mints an actual signed
certificate and proves a real post-drift `Unknown` denies a `SafetyCritical` action
through that one composed pipeline.

**What's still not done:** none of this runs automatically inside the live
`sensing-server` request path yet — there is no continuous calibration/OOD-monitoring
loop wired into the running server that calls this pipeline on live sensor data. Treat
`authorize_from_certificate` as a real, tested library entry point you can call from your
own integration today, not something the server invokes for you on every request yet.
That remaining step is a genuinely separate, larger effort (deciding polling cadence,
where calibration state lives, what triggers re-certification) — see ADR-300 for the
phased plan.

### The crates

| Crate | Role |
|---|---|
| `ruview-ontology` | Canonical `Site → … → Event` types |
| `ruview-attest` | Signed measurement / RF chain-of-custody |
| `ruview-evidence` | Append-only per-context ledger (no pooling, no evidence upgrade) |
| `wifi-densepose-calibration` | Signed, drift-invalidatable calibration certificate |
| `ruview-ood` | `Known` / `Degraded` / `Unknown` staleness-guard domain gating |
| `ruview-witness` | Hash-linked staged provenance chain |
| `ruview-certify` | Capability certificate, conditional on a live domain signature |
| `ruview-scorecard` | Multi-domain scorecard, worst-domain promotion gate |
| `ruview-policy` | Fail-closed action authorization gate |

### Minting and checking a certificate

```rust
use ruview_certify::{mint, CapabilityCertificate, DomainState};

// `signer`, `request`, and `evidence_slice` come from your own calibration run —
// see each crate's README for how to build them.
let cert = mint(&signer, request, &evidence_slice)?;

// A certificate is only valid at a given instant AND domain state — the same
// signed certificate is rejected the moment the live domain degrades:
assert!(cert.is_valid(now_ms, DomainState::Known));
assert!(!cert.is_valid(now_ms, DomainState::Degraded));
assert!(!cert.is_valid(now_ms, DomainState::Unknown));
```

### Gating an action from a real certificate + a real OOD reading

```rust
use ruview_policy::authorize_from_certificate;

// `cert` (ruview_certify::CapabilityCertificate) and `domain`
// (ruview_ood::DomainState) come from your own certify/OOD calls.
let decision = authorize_from_certificate(
    ActionClass::SafetyCritical,
    &cert, &verifier, now_unix_s, domain,
    certificate_class, uncertainty, evidence_level,
);
// Deny with a named FailedCondition (e.g. DomainNotKnown) — not a silent
// false-positive — the moment `domain` degrades, even though `cert` itself
// is still validly signed and unexpired.
```

`ruview_certify::DomainState` and `ruview_policy::DomainState` are still each their own
type (`ruview-ood`'s `Degraded`/`Unknown` additionally carry a `DomainCause`), but the
conversion between them is no longer something you have to write yourself —
`authorize_from_certificate` does it via the crates' own `From<ruview_ood::DomainState>`
impls.

### What's genuinely enforced today, for comparison

Not every ADR-295–296 remediation item is preview-only. Three are live now:

- **UDP data-plane bind hardening (ADR-296)** — `sensing-server`'s `UdpSourceAllowlist`
  is checked on every incoming packet (`main.rs`), not just defined.
- **CSI data-incident repo controls (ADR-299)** — `scripts/csi-data-policy-check.sh`
  runs in CI on every push/PR and fails the build on a policy violation.
- **Synthetic-export watermarking (ADR-295)** — `start_recording` stamps a `SYNTHETIC`
  watermark on a recording's metadata (`GET /api/v1/recordings`, the start-recording
  response) whenever it captures while the live source is synthetic — an operator
  browsing or scripting against recordings can't mistake generated data for a capture.

---

## Hardware Setup

### Supported targets

| Target | Use case | Source target flag | Notes |
|---|---|---|---|
| **ESP32-S3** (default) | Production CSI capture mesh; presence/motion/vital heuristics | `idf.py set-target esp32s3` | Dual-core 240 MHz, PSRAM, native USB-OTG, DVP camera path; live 17-keypoint pose is not validated |
| **ESP32-C6** ([ADR-110](adr/ADR-110-esp32-c6-firmware-extension.md)) | Wi-Fi 6 / 802.15.4 research, battery seed nodes | `idf.py set-target esp32c6` | Single-core 160 MHz, no PSRAM, 802.11ax HE PHY, 802.15.4 (Thread/Zigbee), LP-core hibernation ~5 µA |

The same `firmware/esp32-csi-node` source tree builds for both. ESP-IDF picks up `sdkconfig.defaults.esp32c6` automatically when the target is set to `esp32c6`; otherwise it uses `sdkconfig.defaults` (S3). All C6-only modules are `#ifdef`-gated, so the S3 build is byte-identical to today.

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
| [v0.7.0](https://github.com/ruvnet/RuView/releases/tag/v0.7.0-esp32) | **Latest — ADR-110 firmware-side substrate closed.** Adds ESP-NOW mesh substrate with quantified ≤100 µs alignment (104.1 µs smoothed stdev, 3.95× suppression, 99.56 % cross-board match measured live), 32-byte sync-packet UDP emission with operator-tunable cadence, ADR-018 byte 19 bit 4 wire-fix sourced from working ESP-NOW path, Python SyncPacketParser stub for host wiring ([WITNESS-LOG-110 §A0.7-§A0.13](WITNESS-LOG-110.md)) | `v0.7.0-esp32` |
| [v0.6.9](https://github.com/ruvnet/RuView/releases/tag/v0.6.9-esp32) | Sync-packet UDP emission, `CONFIG_C6_SYNC_EVERY_N_FRAMES` tunable cadence | `v0.6.9-esp32` |
| [v0.6.8](https://github.com/ruvnet/RuView/releases/tag/v0.6.8-esp32) | ESP-NOW EMA-smoothed cross-board offset (3.95× suppression, 104 µs stdev) | `v0.6.8-esp32` |
| [v0.6.7](https://github.com/ruvnet/RuView/releases/tag/v0.6.7-esp32) | Real LP-core motion-gate RISC-V program (B4 code path complete) + Wi-Fi 6 soft-AP with TWT Responder for two-board iTWT benches (B1/B2 unblock) | `v0.6.7-esp32` |
| [v0.5.0](https://github.com/ruvnet/RuView/releases/tag/v0.5.0-esp32) | **Stable (S3 mesh, recommended)** — mmWave sensor fusion (MR60BHA2/LD2410 auto-detect), 48-byte fused vitals, all v0.4.3.1 fixes | `v0.5.0-esp32` |
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

**4MB flash boards** (e.g. ESP32-S3 SuperMini 4MB): download `esp32-csi-node-s3-4mb.bin` + `partition-table-s3-4mb.bin` from the [v0.6.7 release](https://github.com/ruvnet/RuView/releases/tag/v0.6.7-esp32) (882 KB binary, 52 % partition slack) and use `--flash-size 4MB`:

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

### ESP32-C6 (Wi-Fi 6 + 802.15.4 research target — ADR-110)

The C6 build adds four capabilities to the existing csi-node firmware, all opt-in via `idf.py menuconfig → ESP32-C6 capabilities (ADR-110)`:

| Capability | Kconfig | What it does |
|---|---|---|
| **Wi-Fi 6 HE-LTF tagging** | `CSI_FRAME_HE_TAGGING` (default on) | Each ADR-018 frame's previously-reserved bytes 18-19 now carry PPDU type (HT / HE-SU / HE-MU / HE-TB) + bandwidth flags. Magic stays `0xC5110001` — old aggregators see zeros and ignore. |
| **802.15.4 mesh time-sync** | `C6_TIMESYNC_ENABLE` (default on, channel 15) | Beacon-based cross-node clock alignment over the 802.15.4 radio. Frees the WiFi channel from coordination traffic — solves the ADR-029/030 multistatic clock-sync problem. |
| **TWT (Target Wake Time)** | `C6_TWT_ENABLE` (default on, 10 ms wake interval) | After WiFi connect, negotiates an individual TWT agreement with the AP for deterministic CSI cadence. Graceful NACK fallback if the AP doesn't support 11ax TWT. |
| **LP-core wake-on-motion hibernation** | `C6_LP_CORE_ENABLE` (default off) | Always-on motion gate on the LP RISC-V core; HP core stays in deep sleep until the configured GPIO wakes it. Targets ~5 µA for battery-powered Cognitum Seed nodes. |

**Build + flash:**

```bash
cd firmware/esp32-csi-node
idf.py set-target esp32c6
idf.py build                    # ~1.0 MB binary, 46% partition slack on 4 MB flash
idf.py -p COM6 flash
# Then provision the same way as S3 (provision.py works for both targets):
python provision.py --port COM6 --ssid "YourWiFi" --password "secret" --target-ip 192.168.1.20
```

**Verifying the C6 modules came up** — `idf.py -p COM6 monitor` should show:

```
I (353) main: ESP32-C6 CSI Node (ADR-018 / ADR-110) — v0.6.7 — Node ID: 1
I (413) c6_ts: init done: channel=15 EUI=<your-EUI64> leader=yes(candidate)
I (463) wifi: mac_version:HAL_MAC_ESP32AX_761      ← 802.11ax MAC firmware loaded
```

The `c6_ts: init done` line confirms the 802.15.4 stack is up; if TWT succeeds you'll also see an `iTWT setup event received from AP` line after the WiFi connect completes.

**Multi-room time-aligned multistatic capture (preview):**

Flash two or more C6 boards, leave them on the same 802.15.4 channel (default 15). One will elect itself leader (lowest EUI-64) and broadcast `TS_BEACON` frames every 100 ms; the others compute and apply offsets. Each CSI frame from a follower carries a `c6_timesync_get_epoch_us()` wall-clock estimate aligned to within ±100 µs of the leader's monotonic time. Target use case: ADR-029/030 multistatic fusion without burning WiFi airtime on coordination.

**Battery seed-node mode (v0.6.7 — real LP-core program):**

```bash
# Enable LP-core hibernation in menuconfig:
#   ESP32-C6 capabilities (ADR-110) → Enable LP-core wake-on-motion hibernation
#   → LP-core wake GPIO (default 4 — connect a PIR or accelerometer INT line here)
#   → LP-core poll period (default 10 ms)
#   → LP-core debounce sample count (default 3 consecutive matches)
idf.py menuconfig
idf.py build flash
```

When enabled, the C6 LP RISC-V coprocessor runs a real polling program
(`firmware/esp32-csi-node/main/lp_core/main.c`) that polls the wake GPIO at
the configured cadence, debounces N consecutive matching reads, and wakes the
HP core via `ulp_lp_core_wakeup_main_processor()`. `esp_sleep_get_wakeup_cause()`
returns `ESP_SLEEP_WAKEUP_ULP`, and `c6_lp_core_motion_count()` /
`c6_lp_core_poll_count()` expose the LP-side counters for the witness harness.
Target standby current ~5 µA (datasheet; pending INA measurement).

**Two-board iTWT bench (v0.6.7 — soft-AP HE/TWT, no router required):**

Pair two C6 boards — one acts as the iTWT-capable AP, the other as the STA
that negotiates and benchmarks the TWT agreement.

```bash
# Board #1 (AP role): append to sdkconfig.defaults.esp32c6:
CONFIG_C6_SOFTAP_HE_ENABLE=y
CONFIG_C6_SOFTAP_HE_SSID="ruview-c6-twt"
CONFIG_C6_SOFTAP_HE_PSK="ruviewtwt"
CONFIG_C6_SOFTAP_HE_CHANNEL=6

idf.py set-target esp32c6 && idf.py build && idf.py -p COM6 flash
```

Board #1 boots in `WIFI_MODE_APSTA`, advertising HE capabilities and TWT
Responder=1 on channel 6. Board #2 provisions to associate with that SSID:

```bash
python firmware/esp32-csi-node/provision.py --port COM9 \
  --ssid "ruview-c6-twt" --password "ruviewtwt" --target-ip 192.168.1.20
```

Board #2 runs the existing `c6_twt_setup_default()` on connect and now
negotiates a real iTWT agreement against the cooperative AP — the
`iTWT setup queued: wake_interval=10000 µs` log line should be followed by an
`iTWT setup event received from AP` instead of the `INVALID_ARG` graceful
fallback that fired against the bench's 11n-only `ruv.net` AP.

NVS overrides for AP role (namespace `ruview`): `softap_ssid`, `softap_psk`,
`softap_chan` — provision once and the values survive firmware updates.

**What's NOT on the C6 build** (vs S3 production): no AMOLED display (ADR-045 needs 8 MB + LCD touch driver), no WASM3 (ADR-040 needs PSRAM), no Seeed mmWave fusion (separate board). The C6 is a research/seed target, not a drop-in replacement for the S3 production node.

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

> **Accuracy note (2026-06-10):** the previously cited "92.9% PCK@20" figure is
> retracted — a forensic recheck of the surviving eval holdout showed it came
> from a constant-output model scored with an absolute (non-torso-normalized)
> threshold on 69 nearly-static frames, a protocol under which a trivial
> mean-pose predictor scores 100%. No measured camera-supervised PCK@20 is
> currently published (see CHANGELOG, PR #535). Treat this workflow as a data
> collection mechanism; accuracy claims will follow a ≥35-minute multi-pose
> collection session evaluated with torso-normalized PCK.

### Requirements

- Python 3.9+ with `mediapipe` and `opencv-python` (`pip install mediapipe opencv-python`)
- ESP32-S3 node streaming CSI over UDP (port 5005)
- A webcam (laptop, USB, or Mac camera via Tailscale)

### Step 0: Check your CSI rate and plan the session length

Window yield is `csi_frames / 20` — **your CSI packet rate sets how long you
must record.** Check it first (10-second probe):

```bash
python - <<'EOF'
import socket, time
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.bind(('0.0.0.0', 5005)); s.settimeout(2)
n, t0 = 0, time.time()
while time.time() - t0 < 10:
    try: s.recvfrom(4096); n += 1
    except socket.timeout: pass
print(f"{n/10:.1f} Hz -> {n/10*60/20:.0f} windows/min")
EOF
```

| CSI rate | Windows/min | Minutes for 2,000 windows (minimum trainable) |
|---|---|---|
| ~13 Hz (idle network) | ~39 | ~52 min |
| ~53 Hz (active self-ping, #985 firmware) | ~160 | ~13 min — record 35–40 min anyway for pose variety |

A 5-minute session is **not enough to train on** — it produces a few hundred
windows of one pose context, and models trained on it memorize rather than
generalize (this is what invalidated the earlier accuracy figure).

### Step 1: (Recommended) calibrate camera ↔ room

The two-checkerboard calibration (ADR-152 §2.1.3) puts labels in a shared 3D
room frame instead of raw camera coordinates, which is the published defense
against layout-brittle "coordinate overfitting" (PerceptAlign, MobiCom'26):

```bash
python scripts/calibrate-camera-room.py   # < 5 min, two checkerboards + a few photos
```

Without it, collection still works but labels are camera-frame only and the
trained model will not survive camera/node relocation.

### Step 2: Capture Camera + CSI Simultaneously

Run both scripts at the same time (in separate terminals):

```bash
# Terminal 1: Record ESP32 CSI (2400 s = 40 min)
python scripts/record-csi-udp.py --duration 2400

# Terminal 2: Capture camera keypoints
python scripts/collect-ground-truth.py --duration 2400 --preview \
  --calibration data/calibration/camera-room.json   # omit if you skipped Step 1
```

During capture: keep your **full body in frame** with good lighting (MediaPipe
confidence must stay above 0.5 — low-confidence frames are dropped at
alignment), and **change activity every 1–2 minutes**: walk, raise hands,
squat, hands up, kick, wave, turn, jump, sit, stand still. Pose variety is
what the model learns from; 40 minutes of sitting produces a constant-pose
predictor.

### Step 3: Align and Train

```bash
# Align camera keypoints with CSI windows (prints kept/dropped window counts —
# expect roughly csi_frames/20 kept; investigate if far below)
node scripts/align-ground-truth.js \
  --gt data/ground-truth/*.jsonl \
  --csi data/recordings/csi-*.csi.jsonl

# Train (pick the preset matching your window count)
node scripts/train-wiflow-supervised.js \
  --data data/paired/*.jsonl \
  --scale small \
  --epochs 50

# Evaluate — torso-normalized PCK on a TEMPORAL split
node scripts/eval-wiflow.js \
  --model models/wiflow-supervised/wiflow-v1.json \
  --data data/paired/*.jsonl
```

> **Model format boundary:** `train-wiflow-supervised.js` produces the
> JavaScript WiFlow model `wiflow-v1.json`. There is currently no supported
> command that converts that JSON model into the sensing server's binary RVF
> container, and renaming the file to `.rvf` does not convert it. Use the JSON
> model with the JavaScript evaluation/inference tools. To train a model that
> the Rust sensing server can load, use its native training path, which writes
> RVF directly:
>
> ```bash
> cargo run -p wifi-densepose-sensing-server --release -- \
>   --train --dataset data/mmfi --dataset-type mmfi \
>   --epochs 100 --save-rvf models/room-model.rvf
> ```
>
> The camera+CSI paired JSONL workflow and the native RVF trainer are separate
> pipelines today. A JSON-to-RVF exporter is future work.

**Evaluation protocol matters.** Use `eval-wiflow.js` (torso-normalized
PCK@20, the metric comparable to published WiFi-pose results) on a temporal
hold-out, and sanity-check that predictions actually vary across frames
(`pred std > 0`) — a constant-pose model can score deceptively well on
near-static data under weaker protocols. See
`benchmarks/wiflow-std/RESULTS.md` for the forensic case study.

### Scale Presets

| Preset | Params | Training Time | Best For |
|--------|--------|---------------|----------|
| `--scale lite` | 189K | ~19 min | sanity runs only (< 2K windows trains poorly) |
| `--scale small` | 474K | ~1 hr | 2K-10K windows (one 40-min session) |
| `--scale medium` | 800K | ~2 hrs | 10K-50K windows (multiple sessions/rooms) |
| `--scale full` | 7.7M | ~8 hrs | 50K+ windows (GPU recommended) |

See [ADR-079](adr/ADR-079-camera-ground-truth-training.md) for the full design and optimization details, and ADR-152 §2.2 for the external WiFlow-STD benchmark these numbers should be read against.

---

## Pre-Trained Models (No Training Required)

Pre-trained models are available on HuggingFace:
- **CSI encoder + presence head** — https://huggingface.co/ruvnet/wifi-densepose-pretrained
- **SOTA MM-Fi pose model** (82.69% torso-PCK@20) — https://huggingface.co/ruvnet/wifi-densepose-mmfi-pose
- **AetherArena leaderboard Space** — https://huggingface.co/spaces/ruvnet/aether-arena

The encoder artifact can be downloaded for its documented inference path without retraining. The
MM-Fi pose checkpoint is benchmark evidence, not a drop-in live ESP32 model; it requires its exact
input contract and runtime described in [Model and capture compatibility](#model-and-capture-compatibility).
The deterministic signal-pipeline proof is reproducible via `python archive/v1/data/proof/verify.py`
(see [ADR-168](adr/ADR-168-benchmark-proof.md)), but that proof does not validate pose accuracy.

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
#   presence-head.json   — presence detection head (v2 encoder: 82.3% held-out triplet acc)
#   node-1.json          — LoRA adapter for room 1
#   node-2.json          — LoRA adapter for room 2
```

### What the Models Do

The pre-trained encoder converts 8-dim CSI feature vectors into 128-dim embeddings. These embeddings power all 17 sensing applications:

- **Presence detection** — v2 encoder: 82.3% held-out temporal-triplet accuracy (v1's "100%" was a single-class recording — retracted, [#882](https://github.com/ruvnet/RuView/issues/882))
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
- If a standalone `aggregator` command is already listening on UDP `:5005`, stop it and run `sensing-server --source esp32 --udp-port 5005` instead; the Observatory reads the server WebSocket, not the standalone aggregator output
- Verify the ESP32 nodes are provisioned to the IP address of the machine running `sensing-server`
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
The strongest published RuView result is the MM-Fi transformer benchmark (82.69% torso-PCK@20 on
the matched `random_split` protocol), not a live ESP32 result. The committed cog model measured
3.0% PCK@20 on its holdout, below the ≥35% target, and the viewer's live skeleton is a heuristic
placeholder. No measured claim currently shows a 3-node ESP32 mesh reliably tracking 17 keypoints.
ADR-027's 10-second unlabeled MERIDIAN adaptation is Proposed; the available measured calibration
reference uses labeled, model-specific samples.

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
