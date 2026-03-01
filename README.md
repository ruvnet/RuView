# WiFi DensePose

**See through walls with WiFi.** No cameras. No wearables. Just radio waves.

WiFi DensePose turns commodity WiFi signals into real-time human pose estimation, vital sign monitoring, and presence detection — all without a single pixel of video. By analyzing Channel State Information (CSI) disturbances caused by human movement, the system reconstructs body position, breathing rate, and heartbeat using physics-based signal processing and machine learning.

[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests: 542+](https://img.shields.io/badge/tests-542%2B-brightgreen.svg)](https://github.com/ruvnet/wifi-densepose)
[![Docker: 132 MB](https://img.shields.io/badge/docker-132%20MB-blue.svg)](https://hub.docker.com/r/ruvnet/wifi-densepose)
[![Vital Signs](https://img.shields.io/badge/vital%20signs-breathing%20%2B%20heartbeat-red.svg)](#-vital-sign-detection-adr-021)
[![ESP32 Ready](https://img.shields.io/badge/ESP32--S3-CSI%20streaming-purple.svg)](#esp32-s3-hardware-pipeline-adr-018)

> | What | How | Speed |
> |------|-----|-------|
> | **Pose estimation** | CSI subcarrier amplitude/phase → DensePose UV maps | 54K fps (Rust) |
> | **Breathing detection** | Bandpass 0.1-0.5 Hz → FFT peak | 6-30 BPM |
> | **Heart rate** | Bandpass 0.8-2.0 Hz → FFT peak | 40-120 BPM |
> | **Presence sensing** | RSSI variance + motion band power | < 1ms latency |
> | **Through-wall** | Fresnel zone geometry + multipath modeling | Up to 5m depth |

```bash
# 30 seconds to live sensing — no toolchain required
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 ruvnet/wifi-densepose:latest
# Open http://localhost:3000
```

> **Hardware options** for live CSI capture:
>
> | Option | Hardware | Cost | Capabilities |
> |--------|----------|------|-------------|
> | **ESP32 Mesh** (recommended) | 3-6x ESP32-S3 + WiFi router | ~$54 | Presence, motion, breathing, heartbeat |
> | **Research NIC** | Intel 5300 / Atheros AR9580 | ~$50-100 | Full CSI with 3x3 MIMO |
> | **Any WiFi** | Windows/Linux laptop | $0 | RSSI-based presence and motion |
>
> No hardware? Verify the pipeline with the deterministic reference signal: `python v1/data/proof/verify.py`

## 🚀 Key Features

| Feature | Description |
|---------|-------------|
| **Privacy-First** | No cameras — uses WiFi signals for pose detection |
| **Real-Time** | Sub-100µs/frame (Rust), 11,665 fps vital sign benchmark |
| **Vital Signs** | Contactless breathing (6-30 BPM) and heart rate (40-120 BPM) |
| **Multi-Person** | Simultaneous tracking of up to 10 individuals |
| **Docker Ready** | `docker pull ruvnet/wifi-densepose:latest` (132 MB) |
| **RVF Portable Models** | Single-file `.rvf` containers with progressive loading |
| **542+ Tests** | Comprehensive Rust test suite, zero mocks |

<details>
<summary><strong>📡 ESP32-S3 Hardware Pipeline (ADR-018)</strong> — 20 Hz CSI streaming, flash & provision</summary>

```
ESP32-S3 (STA + promiscuous)     UDP/5005      Rust aggregator
┌─────────────────────────┐    ──────────>    ┌──────────────────┐
│ WiFi CSI callback 20 Hz │    ADR-018        │ Esp32CsiParser   │
│ ADR-018 binary frames   │    binary         │ CsiFrame output  │
│ stream_sender (UDP)     │                   │ presence detect  │
└─────────────────────────┘                   └──────────────────┘
```

| Metric | Measured |
|--------|----------|
| Frame rate | ~20 Hz sustained |
| Subcarriers | 64 / 128 / 192 (LLTF, HT, HT40) |
| Latency | < 1ms (UDP loopback) |
| Presence detection | Motion score 10/10 at 3m |

```bash
# Pre-built binaries — no toolchain required
# https://github.com/ruvnet/wifi-densepose/releases/tag/v0.1.0-esp32

python -m esptool --chip esp32s3 --port COM7 --baud 460800 \
  write-flash --flash-mode dio --flash-size 4MB \
  0x0 bootloader.bin 0x8000 partition-table.bin 0x10000 esp32-csi-node.bin

python scripts/provision.py --port COM7 \
  --ssid "YourWiFi" --password "secret" --target-ip 192.168.1.20

cargo run -p wifi-densepose-hardware --bin aggregator -- --bind 0.0.0.0:5005 --verbose
```

See [firmware/esp32-csi-node/README.md](firmware/esp32-csi-node/README.md) and [Tutorial #34](https://github.com/ruvnet/wifi-densepose/issues/34).

</details>

<details open>
<summary><strong>🦀 Rust Implementation (v2)</strong> — 810x faster, 54K fps pipeline</summary>

### Performance Benchmarks (Validated)

| Operation | Python (v1) | Rust (v2) | Speedup |
|-----------|-------------|-----------|---------|
| CSI Preprocessing (4x64) | ~5ms | **5.19 µs** | ~1000x |
| Phase Sanitization (4x64) | ~3ms | **3.84 µs** | ~780x |
| Feature Extraction (4x64) | ~8ms | **9.03 µs** | ~890x |
| Motion Detection | ~1ms | **186 ns** | ~5400x |
| **Full Pipeline** | ~15ms | **18.47 µs** | ~810x |
| **Vital Signs** | N/A | **86 µs** | 11,665 fps |

| Resource | Python (v1) | Rust (v2) |
|----------|-------------|-----------|
| Memory | ~500 MB | ~100 MB |
| Docker Image | 569 MB | 132 MB |
| Tests | 41 | 542+ |
| WASM Support | No | Yes |

```bash
cd rust-port/wifi-densepose-rs
cargo build --release
cargo test --workspace
cargo bench --package wifi-densepose-signal
```

</details>

<details>
<summary><strong>💓 Vital Sign Detection (ADR-021)</strong> — Breathing and heartbeat via FFT</summary>

| Capability | Range | Method |
|------------|-------|--------|
| **Breathing Rate** | 6-30 BPM (0.1-0.5 Hz) | Bandpass filter + FFT peak detection |
| **Heart Rate** | 40-120 BPM (0.8-2.0 Hz) | Bandpass filter + FFT peak detection |
| **Sampling Rate** | 20 Hz (ESP32 CSI) | Real-time streaming |
| **Confidence** | 0.0-1.0 per sign | Spectral coherence + signal quality |

```bash
./target/release/sensing-server --source simulate --ui-path ../../ui
curl http://localhost:8080/api/v1/vital-signs
```

See [ADR-021](docs/adr/ADR-021-vital-sign-detection-rvdna-pipeline.md).

</details>

<details>
<summary><strong>📡 WiFi Scan Domain Layer (ADR-022)</strong> — 8-stage RSSI pipeline for Windows WiFi</summary>

| Stage | Purpose |
|-------|---------|
| **Predictive Gating** | Pre-filter scan results using temporal prediction |
| **Attention Weighting** | Weight BSSIDs by signal relevance |
| **Spatial Correlation** | Cross-AP spatial signal correlation |
| **Motion Estimation** | Detect movement from RSSI variance |
| **Breathing Extraction** | Extract respiratory rate from sub-Hz oscillations |
| **Quality Gating** | Reject low-confidence estimates |
| **Fingerprint Matching** | Location and posture classification via RF fingerprints |
| **Orchestration** | Fuse all stages into unified sensing output |

```bash
cargo test -p wifi-densepose-wifiscan
```

See [ADR-022](docs/adr/ADR-022-windows-wifi-enhanced-fidelity-ruvector.md) and [Tutorial #36](https://github.com/ruvnet/wifi-densepose/issues/36).

</details>

<details>
<summary><strong>🚨 WiFi-Mat: Disaster Response</strong> — Search & rescue, START triage, 3D localization</summary>

| Feature | Description |
|---------|-------------|
| **Vital Signs** | Breathing (4-60 BPM), heartbeat via micro-Doppler |
| **3D Localization** | Position estimation through debris up to 5m |
| **START Triage** | Automatic Immediate/Delayed/Minor/Deceased classification |
| **Real-time Alerts** | Priority-based notifications with escalation |

```rust
use wifi_densepose_mat::{DisasterResponse, DisasterConfig, DisasterType, ScanZone, ZoneBounds};

let config = DisasterConfig::builder()
    .disaster_type(DisasterType::Earthquake)
    .sensitivity(0.85)
    .max_depth(5.0)
    .build();

let mut response = DisasterResponse::new(config);
response.initialize_event(location, "Building collapse")?;
response.add_zone(ScanZone::new("North Wing", ZoneBounds::rectangle(0.0, 0.0, 30.0, 20.0)))?;
response.start_scanning().await?;
```

- [WiFi-Mat User Guide](docs/wifi-mat-user-guide.md) | [ADR-001](docs/adr/ADR-001-wifi-mat-disaster-detection.md) | [Domain Model](docs/ddd/wifi-mat-domain-model.md)

</details>

<details>
<summary><strong>📦 RVF Model Container</strong> — Single-file deployment with progressive loading</summary>

| Property | Detail |
|----------|--------|
| **Format** | Segment-based binary (magic `0x52564653`) with 64-byte headers |
| **Progressive Loading** | Layer A <5ms, Layer B 100ms-1s, Layer C full graph |
| **Signing** | Ed25519 training proofs for verifiable provenance |
| **Quantization** | f32/f16/u8 via `rvf-quant` with SIMD distance |
| **CLI** | `--export-rvf`, `--save-rvf`, `--load-rvf`, `--model` |

```bash
# Export model package
./target/release/sensing-server --export-rvf wifi-densepose-v1.rvf

# Load and run with progressive loading
./target/release/sensing-server --model wifi-densepose-v1.rvf --progressive
```

See [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md).

</details>

<details>
<summary><strong>🧬 Training & Fine-Tuning</strong> — MM-Fi/Wi-Pose pre-training, SONA adaptation</summary>

Three-tier data strategy:

1. **Pre-train** on public datasets (MM-Fi, Wi-Pose) for cross-environment generalization
2. **Fine-tune** with ESP32 data + camera pseudo-labels for environment-specific multipath
3. **SONA adaptation** via micro-LoRA + EWC++ for continuous on-device learning

```bash
# Pre-train
./target/release/sensing-server --train --dataset data/ --dataset-type mmfi --epochs 100

# Or via Docker
docker run --rm -v $(pwd)/data:/data ruvnet/wifi-densepose:latest \
  --train --dataset /data --epochs 100 --export-rvf /data/model.rvf
```

</details>

<details>
<summary><strong>🔩 RuVector Crates</strong> — 11 vendored signal intelligence crates</summary>

| Crate | Purpose |
|-------|---------|
| `ruvector-core` | VectorDB, HNSW index, SIMD distance, quantization |
| `ruvector-attention` | Scaled dot-product, MoE, sparse attention |
| `ruvector-gnn` | Graph neural network, graph attention, EWC training |
| `ruvector-nervous-system` | PredictiveLayer, OscillatoryRouter, Hopfield |
| `ruvector-coherence` | Spectral coherence, HNSW health, Fiedler value |
| `ruvector-temporal-tensor` | Tiered temporal compression (8/7/5/3-bit) |
| `ruvector-mincut` | Subpolynomial dynamic min-cut |
| `ruvector-attn-mincut` | Attention-gated min-cut |
| `ruvector-solver` | Sparse Neumann solver O(sqrt(n)) |
| `ruvector-graph-transformer` | Proof-gated graph transformer |
| `ruvector-sparse-inference` | PowerInfer-style sparse execution |

See `vendor/ruvector/` for full source.

</details>

<details>
<summary><strong>🔬 SOTA Signal Processing (ADR-014)</strong> — 6 research-grade algorithms</summary>

| Algorithm | Purpose | Reference |
|-----------|---------|-----------|
| **Conjugate Multiplication** | Cancels CFO/SFO from raw CSI phase | SpotFi (SIGCOMM 2015) |
| **Hampel Filter** | Robust outlier removal using median/MAD | Hampel (1974) |
| **Fresnel Zone Model** | Physics-based breathing detection | FarSense (MobiCom 2019) |
| **CSI Spectrogram** | STFT time-frequency matrices | Standard since 2018 |
| **Subcarrier Selection** | Variance-ratio top-K ranking | WiDance (MobiCom 2017) |
| **Body Velocity Profile** | Domain-independent velocity x time | Widar 3.0 (MobiSys 2019) |

</details>

## 📋 Table of Contents

<details open>
<summary><strong>🚀 Getting Started</strong> — Install, Docker, first API call</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [Key Features](#-key-features) | Capabilities overview — privacy, real-time, multi-person |
| [Rust Implementation (v2)](#-rust-implementation-v2) | 810x faster signal processing, 54K fps pipeline |
| [Installation](#-installation) | Guided installer, Docker, Rust, or Python setup |
| [Quick Start](#-quick-start) | First API call in 3 commands |
| [Using Docker](#using-docker) | `docker pull` and run — 132 MB, no toolchain needed |

</details>

<details>
<summary><strong>📡 Signal Processing & Sensing</strong> — From raw WiFi frames to vital signs</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [ESP32-S3 Hardware Pipeline](#esp32-s3-hardware-pipeline-adr-018) | 20 Hz CSI streaming, flash & provision guide |
| [Vital Sign Detection (ADR-021)](#-vital-sign-detection-adr-021) | Breathing 6-30 BPM, heartbeat 40-120 BPM via FFT |
| [WiFi Scan Domain Layer (ADR-022)](#-wifi-scan-domain-layer-adr-022) | 8-stage RSSI pipeline for Windows WiFi |
| [WiFi-Mat Disaster Response](#-wifi-mat-disaster-response-module) | Search & rescue, START triage, 3D localization |
| [SOTA Signal Processing (ADR-014)](#sota-signal-processing-adr-014) | Conjugate multiplication, Hampel filter, Fresnel model |

</details>

<details>
<summary><strong>🧠 Models & Training</strong> — DensePose pipeline, RVF containers, SONA adaptation</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [RVF Model Container](#-rvf-model-container-format) | Single-file `.rvf` packaging with progressive loading |
| [Training and Fine-Tuning](#-training-and-fine-tuning) | MM-Fi/Wi-Pose pre-training, `--train` CLI mode |
| [RuVector Crates](#-ruvector-crates) | 11 vendored signal intelligence crates |
| [System Architecture](#️-system-architecture) | End-to-end data flow from CSI to API |

</details>

<details>
<summary><strong>🖥️ Usage & Configuration</strong> — CLI flags, API endpoints, hardware setup</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [CLI Usage](#️-cli-usage) | `--export-rvf`, `--train`, `--benchmark`, `--source` |
| [Documentation](#-documentation) | Core docs, API overview, quick links |
| [Hardware Setup](#-hardware-setup) | Supported devices, physical placement, calibration |
| [Configuration](#️-configuration) | Environment variables, domain-specific configs |

</details>

<details>
<summary><strong>⚙️ Development & Testing</strong> — 542+ tests, CI, deployment</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [Testing](#-testing) | 542+ tests, hardware-free simulation, CI pipeline |
| [Deployment](#-deployment) | Docker, docker-compose, production monitoring |
| [Contributing](#-contributing) | Dev setup, code standards, review checklist |

</details>

<details>
<summary><strong>📊 Performance & Benchmarks</strong> — Measured throughput, latency, resource usage</summary>

| Section | What You'll Learn |
|---------|-------------------|
| [Performance Metrics](#-performance-metrics) | 11,665 fps vital signs, 54K fps signal pipeline |
| [Rust vs Python](#performance-benchmarks-validated) | 810x full pipeline, 5400x motion detection |
| [Docker Images](#using-docker) | 132 MB Rust / 569 MB Python, port mappings |

</details>

<details>
<summary><strong>📄 Meta</strong> — License, acknowledgments, support</summary>

| | |
|---|---|
| [License](#-license) | MIT |
| [Acknowledgments](#-acknowledgments) | Research references and credits |
| [Support](#-support) | Issues, discussions, contact |

</details>

<details>
<summary><strong>🏗️ System Architecture</strong> — End-to-end data flow from CSI to API</summary>

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   WiFi Router   │    │   WiFi Router   │    │   WiFi Router   │
│   (CSI Source)  │    │   (CSI Source)  │    │   (CSI Source)  │
└─────────┬───────┘    └─────────┬───────┘    └─────────┬───────┘
          │                      │                      │
          └──────────────────────┼──────────────────────┘
                                 │
                    ┌─────────────▼─────────────┐
                    │     CSI Data Collector    │
                    │   (Hardware Interface)    │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │    Signal Processor       │
                    │  (RuVector + Phase San.)  │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │   Graph Transformer       │
                    │  (DensePose + GNN Head)   │
                    └─────────────┬─────────────┘
                                  │
                    ┌─────────────▼─────────────┐
                    │   Vital Signs + Tracker   │
                    │  (Breathing, Heart, Pose) │
                    └─────────────┬─────────────┘
                                  │
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
┌─────────▼─────────┐   ┌─────────▼─────────┐   ┌─────────▼─────────┐
│   REST API        │   │  WebSocket API    │   │   Analytics       │
│  (Axum / FastAPI) │   │ (Real-time Stream)│   │  (Fall Detection) │
└───────────────────┘   └───────────────────┘   └───────────────────┘
```

| Component | Description |
|-----------|-------------|
| **CSI Processor** | Extracts Channel State Information from WiFi signals (ESP32 or RSSI) |
| **Signal Processor** | RuVector-powered phase sanitization, Hampel filter, Fresnel model |
| **Graph Transformer** | GNN body-graph reasoning with cross-attention CSI-to-pose mapping |
| **Vital Signs** | FFT-based breathing (0.1-0.5 Hz) and heartbeat (0.8-2.0 Hz) extraction |
| **REST API** | Axum (Rust) or FastAPI (Python) for data access and control |
| **WebSocket** | Real-time pose, sensing, and vital sign streaming |
| **Analytics** | Fall detection, activity recognition, START triage |

</details>

<details>
<summary><strong>📦 Installation</strong> — Guided installer, Docker, Rust, or Python</summary>

### Guided Installer (Recommended)

The interactive installer detects your hardware, checks your environment, and builds the right profile automatically:

```bash
./install.sh
```

It walks through 7 steps:
1. **System detection** — OS, RAM, disk, GPU
2. **Toolchain detection** — Python, Rust, Docker, Node.js, ESP-IDF
3. **WiFi hardware detection** — interfaces, ESP32 USB, Intel CSI debug
4. **Profile recommendation** — picks the best profile for your hardware
5. **Dependency installation** — installs what's missing
6. **Build** — compiles the selected profile
7. **Summary** — shows next steps and verification commands

#### Install Profiles

| Profile | What it installs | Size | Requirements |
|---------|-----------------|------|-------------|
| `verify` | Pipeline verification only | ~5 MB | Python 3.8+ |
| `python` | Full Python API server + sensing | ~500 MB | Python 3.8+ |
| `rust` | Rust pipeline (~810x faster) | ~200 MB | Rust 1.70+ |
| `browser` | WASM for in-browser execution | ~10 MB | Rust + wasm-pack |
| `iot` | ESP32 sensor mesh + aggregator | varies | Rust + ESP-IDF |
| `docker` | Docker-based deployment | ~1 GB | Docker |
| `field` | WiFi-Mat disaster response kit | ~62 MB | Rust + wasm-pack |
| `full` | Everything available | ~2 GB | All toolchains |

#### Non-Interactive Install

```bash
# Install a specific profile without prompts
./install.sh --profile rust --yes

# Just run hardware detection (no install)
./install.sh --check-only

# Or use make targets
make install              # Interactive
make install-verify       # Verification only
make install-python       # Python pipeline
make install-rust         # Rust pipeline
make install-browser      # WASM browser build
make install-docker       # Docker deployment
make install-field        # Disaster response kit
make install-full         # Everything
make check                # Hardware check only
```

### From Source (Rust — Primary)

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

# Install Rust pipeline (810x faster than Python)
./install.sh --profile rust --yes

# Or manually:
cd rust-port/wifi-densepose-rs
cargo build --release
cargo test --workspace
```

### From Source (Python)

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose
pip install -r requirements.txt
pip install -e .
```

### Using pip (Python only)

```bash
pip install wifi-densepose

# With optional dependencies
pip install wifi-densepose[gpu]  # For GPU acceleration
pip install wifi-densepose[all]  # All optional dependencies
```

### Using Docker

Pre-built images are published on Docker Hub:

```bash
# Rust sensing server (132 MB — recommended)
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp ruvnet/wifi-densepose:latest

# Python sensing pipeline (569 MB)
docker pull ruvnet/wifi-densepose:python
docker run -p 8765:8765 -p 8080:8080 ruvnet/wifi-densepose:python

# Or use docker-compose for both
cd docker && docker compose up
```

| Image | Tag | Size | Ports |
|-------|-----|------|-------|
| `ruvnet/wifi-densepose` | `latest`, `rust` | 132 MB | 3000 (REST), 3001 (WS), 5005/udp (ESP32) |
| `ruvnet/wifi-densepose` | `python` | 569 MB | 8765 (WS), 8080 (UI) |

**Export RVF model package:**
```bash
docker run --rm -v $(pwd):/out ruvnet/wifi-densepose:latest --export-rvf /out/wifi-densepose-v1.rvf
```

### System Requirements

- **Rust**: 1.70+ (primary runtime — install via [rustup](https://rustup.rs/))
- **Python**: 3.8+ (for verification and legacy v1 API)
- **Operating System**: Linux (Ubuntu 18.04+), macOS (10.15+), Windows 10+
- **Memory**: Minimum 4GB RAM, Recommended 8GB+
- **Storage**: 2GB free space for models and data
- **Network**: WiFi interface with CSI capability (optional — installer detects what you have)
- **GPU**: Optional (NVIDIA CUDA or Apple Metal)

</details>

<details>
<summary><strong>🚀 Quick Start</strong> — First API call in 3 commands</summary>

### 1. Basic Setup

```bash
# Install the package (Rust — recommended)
./install.sh --profile rust --yes

# Or Python legacy
pip install wifi-densepose

# Copy example configuration
cp example.env .env

# Edit configuration (set your WiFi interface)
nano .env
```

### 2. Start the System

```python
from wifi_densepose import WiFiDensePose

# Initialize with default configuration
system = WiFiDensePose()

# Start pose estimation
system.start()

# Get latest pose data
poses = system.get_latest_poses()
print(f"Detected {len(poses)} persons")

# Stop the system
system.stop()
```

### 3. Using the REST API

```bash
# Start the API server
wifi-densepose start

# Start with custom configuration
wifi-densepose -c /path/to/config.yaml start

# Start with verbose logging
wifi-densepose -v start

# Check server status
wifi-densepose status
```

The API will be available at `http://localhost:8000`

- **API Documentation**: http://localhost:8000/docs
- **Health Check**: http://localhost:8000/api/v1/health
- **Latest Poses**: http://localhost:8000/api/v1/pose/latest

### 4. Real-time Streaming

```python
import asyncio
import websockets
import json

async def stream_poses():
    uri = "ws://localhost:8000/ws/pose/stream"
    async with websockets.connect(uri) as websocket:
        while True:
            data = await websocket.recv()
            poses = json.loads(data)
            print(f"Received poses: {len(poses['persons'])} persons detected")

# Run the streaming client
asyncio.run(stream_poses())
```

</details>

<details>
<summary><strong>🖥️ CLI Usage</strong> — Server management, Rust sensing server flags</summary>

#### Rust Sensing Server (Primary)

```bash
# Start with simulated data (no hardware)
./target/release/sensing-server --source simulate --ui-path ../../ui

# Start with ESP32 CSI hardware
./target/release/sensing-server --source esp32 --udp-port 5005

# Start with Windows WiFi RSSI
./target/release/sensing-server --source wifi

# Run vital sign benchmark
./target/release/sensing-server --benchmark

# Export RVF model package
./target/release/sensing-server --export-rvf model.rvf

# Train a model
./target/release/sensing-server --train --dataset data/ --epochs 100

# Load trained model with progressive loading
./target/release/sensing-server --model wifi-densepose-v1.rvf --progressive
```

| Flag | Description |
|------|-------------|
| `--source` | Data source: `auto`, `wifi`, `esp32`, `simulate` |
| `--http-port` | HTTP port for UI and REST API (default: 8080) |
| `--ws-port` | WebSocket port (default: 8765) |
| `--udp-port` | UDP port for ESP32 CSI frames (default: 5005) |
| `--benchmark` | Run vital sign benchmark (1000 frames) and exit |
| `--export-rvf` | Export RVF container package and exit |
| `--load-rvf` | Load model config from RVF container |
| `--save-rvf` | Save model state on shutdown |
| `--model` | Load trained `.rvf` model for inference |
| `--progressive` | Enable progressive loading (Layer A instant start) |
| `--train` | Train a model and exit |
| `--dataset` | Path to dataset directory (MM-Fi or Wi-Pose) |
| `--epochs` | Training epochs (default: 100) |

#### Python Legacy CLI

WiFi DensePose provides a comprehensive command-line interface for easy system management, configuration, and monitoring.

### CLI Installation

The CLI is automatically installed with the package:

```bash
# Install WiFi DensePose with CLI
pip install wifi-densepose

# Verify CLI installation
wifi-densepose --help
wifi-densepose version
```

### Basic Commands

The WiFi-DensePose CLI provides the following commands:

```bash
wifi-densepose [OPTIONS] COMMAND [ARGS]...

Options:
  -c, --config PATH  Path to configuration file
  -v, --verbose      Enable verbose logging
  --debug            Enable debug mode
  --help             Show this message and exit.

Commands:
  config   Configuration management commands.
  db       Database management commands.
  start    Start the WiFi-DensePose API server.
  status   Show the status of the WiFi-DensePose API server.
  stop     Stop the WiFi-DensePose API server.
  tasks    Background task management commands.
  version  Show version information.
```

#### Server Management
```bash
# Start the WiFi-DensePose API server
wifi-densepose start

# Start with custom configuration
wifi-densepose -c /path/to/config.yaml start

# Start with verbose logging
wifi-densepose -v start

# Start with debug mode
wifi-densepose --debug start

# Check server status
wifi-densepose status

# Stop the server
wifi-densepose stop

# Show version information
wifi-densepose version
```

### Configuration Commands

#### Configuration Management
```bash
# Configuration management commands
wifi-densepose config [SUBCOMMAND]

# Examples:
# Show current configuration
wifi-densepose config show

# Validate configuration file
wifi-densepose config validate

# Create default configuration
wifi-densepose config init

# Edit configuration
wifi-densepose config edit
```

#### Database Management
```bash
# Database management commands
wifi-densepose db [SUBCOMMAND]

# Examples:
# Initialize database
wifi-densepose db init

# Run database migrations
wifi-densepose db migrate

# Check database status
wifi-densepose db status

# Backup database
wifi-densepose db backup

# Restore database
wifi-densepose db restore
```

#### Background Tasks
```bash
# Background task management commands
wifi-densepose tasks [SUBCOMMAND]

# Examples:
# List running tasks
wifi-densepose tasks list

# Start background tasks
wifi-densepose tasks start

# Stop background tasks
wifi-densepose tasks stop

# Check task status
wifi-densepose tasks status
```

### REST API (Rust Sensing Server)

```bash
GET  /api/v1/sensing           # Latest sensing frame
GET  /api/v1/vital-signs       # Breathing, heart rate, confidence
GET  /api/v1/bssid             # Multi-BSSID registry
GET  /api/v1/model/layers      # Progressive loading status
GET  /api/v1/model/sona/profiles  # SONA profiles
POST /api/v1/model/sona/activate  # Activate SONA profile
```

WebSocket: `ws://localhost:8765/ws/sensing` (real-time sensing + vital signs)

### Hardware Support

| Hardware | CSI | Cost | Guide |
|----------|-----|------|-------|
| **ESP32-S3** | Native | ~$8 | [Tutorial #34](https://github.com/ruvnet/wifi-densepose/issues/34) |
| Intel 5300 | Firmware mod | ~$15 | Linux `iwl-csi` |
| Atheros AR9580 | ath9k patch | ~$20 | Linux only |
| Any Windows WiFi | RSSI only | $0 | [Tutorial #36](https://github.com/ruvnet/wifi-densepose/issues/36) |

### Docs

- [User Guide](docs/user_guide.md) | [API Reference](docs/api_reference.md) | [Deployment](docs/deployment.md) | [Troubleshooting](docs/troubleshooting.md)
- [ADR-021](docs/adr/ADR-021-vital-sign-detection-rvdna-pipeline.md) | [ADR-022](docs/adr/ADR-022-windows-wifi-enhanced-fidelity-ruvector.md) | [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md)

</details>

<details>
<summary><strong>🧪 Testing</strong> — 542+ tests, hardware-free simulation, CI</summary>

```bash
# Rust tests (primary — 542+ tests, zero mocks)
cd rust-port/wifi-densepose-rs
cargo test --workspace

# Sensing server tests (229 tests)
cargo test -p wifi-densepose-sensing-server

# Vital sign benchmark
./target/release/sensing-server --benchmark

# Python tests
python -m pytest v1/tests/ -v

# Pipeline verification (no hardware needed)
./verify
```

| Suite | Tests | What It Covers |
|-------|-------|----------------|
| sensing-server lib | 147 | Graph transformer, trainer, SONA, sparse inference, RVF |
| sensing-server bin | 48 | CLI integration, WebSocket, REST API |
| RVF integration | 16 | Container build, read, progressive load |
| Vital signs integration | 18 | FFT detection, breathing, heartbeat |
| wifi-densepose-signal | 83 | SOTA algorithms, Doppler, Fresnel |
| wifi-densepose-mat | 139 | Disaster response, triage, localization |
| wifi-densepose-wifiscan | 91 | 8-stage RSSI pipeline |

</details>

<details>
<summary><strong>🚀 Deployment</strong> — Docker, docker-compose, production</summary>

### Docker (Recommended)

```bash
# Rust sensing server (132 MB)
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp ruvnet/wifi-densepose:latest

# Python pipeline (569 MB)
docker pull ruvnet/wifi-densepose:python
docker run -p 8765:8765 -p 8080:8080 ruvnet/wifi-densepose:python

# Both via docker-compose
cd docker && docker compose up

# Export RVF model
docker run --rm -v $(pwd):/out ruvnet/wifi-densepose:latest --export-rvf /out/model.rvf
```

### Environment Variables

```bash
RUST_LOG=info                    # Logging level
WIFI_INTERFACE=wlan0             # WiFi interface for RSSI
POSE_CONFIDENCE_THRESHOLD=0.7    # Minimum confidence
POSE_MAX_PERSONS=10              # Max tracked individuals
```

</details>

<details>
<summary><strong>📊 Performance Metrics</strong> — Measured benchmarks</summary>

### Rust Sensing Server

| Metric | Value |
|--------|-------|
| Vital sign detection | **11,665 fps** (86 µs/frame) |
| Full CSI pipeline | **54,000 fps** (18.47 µs/frame) |
| Motion detection | **186 ns** (~5,400x vs Python) |
| Docker image | 132 MB |
| Memory usage | ~100 MB |
| Test count | 542+ |

### Python vs Rust

| Operation | Python | Rust | Speedup |
|-----------|--------|------|---------|
| CSI Preprocessing | ~5 ms | 5.19 µs | 1000x |
| Phase Sanitization | ~3 ms | 3.84 µs | 780x |
| Feature Extraction | ~8 ms | 9.03 µs | 890x |
| Motion Detection | ~1 ms | 186 ns | 5400x |
| **Full Pipeline** | ~15 ms | 18.47 µs | **810x** |

</details>

<details>
<summary><strong>🤝 Contributing</strong> — Dev setup, code standards, PR process</summary>

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

# Rust development
cd rust-port/wifi-densepose-rs
cargo build --release
cargo test --workspace

# Python development
python -m venv venv && source venv/bin/activate
pip install -r requirements-dev.txt && pip install -e .
pre-commit install
```

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes
4. **Push** and open a Pull Request

</details>

<details>
<summary><strong>📄 Changelog</strong> — Release history</summary>

### v2.3.0 — 2026-03-01

- **Docker images published** — `ruvnet/wifi-densepose:latest` (132 MB Rust) and `:python` (569 MB)
- **8-phase DensePose training pipeline (ADR-023)** — Dataset loaders, graph transformer, trainer, SONA adaptation, sparse inference, RVF pipeline, server integration
- **`--export-rvf` CLI flag** — Standalone RVF model package generation
- **`--train` CLI flag** — Full training mode with cosine-scheduled SGD, PCK/OKS validation
- **Vital sign detection (ADR-021)** — FFT-based breathing and heartbeat extraction, 11,665 fps
- **542+ Rust tests** — All passing, zero mocks

### v2.2.0 — 2026-02-28

- **Guided installer** — `./install.sh` with 7-step hardware detection
- **6 SOTA signal algorithms (ADR-014)** — SpotFi, Hampel, Fresnel, spectrogram, subcarrier selection, BVP
- **WiFi-Mat disaster response** — START triage, scan zones, API endpoints — 139 tests
- **ESP32 CSI hardware parser** — Binary frame parsing with I/Q extraction — 28 tests
- **WiFi scan domain layer (ADR-022)** — 8-stage pure-Rust signal intelligence pipeline
- **Security hardening** — 10 vulnerabilities fixed

### v2.1.0 — 2026-02-28

- **RuVector RVF integration** — ADR-002 through ADR-013
- **ESP32 CSI sensor mesh** — $54 starter kit with 3-6 ESP32-S3 nodes
- **Three.js visualization** — 3D body model with WebSocket streaming
- **CI verification pipeline** — Determinism checks and unseeded random scan

</details>

## 📄 License

MIT License — see [LICENSE](LICENSE) for details.

## 📞 Support

[GitHub Issues](https://github.com/ruvnet/wifi-densepose/issues) | [Discussions](https://github.com/ruvnet/wifi-densepose/discussions) | [PyPI](https://pypi.org/project/wifi-densepose/)

---

**WiFi DensePose** — Privacy-preserving human pose estimation through WiFi signals.