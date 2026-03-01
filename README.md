# WiFi DensePose

**See through walls with WiFi.** No cameras. No wearables. Just radio waves.

WiFi DensePose turns commodity WiFi signals into real-time human pose estimation, vital sign monitoring, and presence detection — all without a single pixel of video. By analyzing Channel State Information (CSI) disturbances caused by human movement, the system reconstructs body position, breathing rate, and heartbeat using physics-based signal processing and machine learning.

[![Rust 1.85+](https://img.shields.io/badge/rust-1.85+-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tests: 542+](https://img.shields.io/badge/tests-542%2B-brightgreen.svg)](https://github.com/ruvnet/wifi-densepose)
[![Docker: 132 MB](https://img.shields.io/badge/docker-132%20MB-blue.svg)](https://hub.docker.com/r/ruvnet/wifi-densepose)
[![Vital Signs](https://img.shields.io/badge/vital%20signs-breathing%20%2B%20heartbeat-red.svg)](#vital-sign-detection)
[![ESP32 Ready](https://img.shields.io/badge/ESP32--S3-CSI%20streaming-purple.svg)](#esp32-s3-hardware-pipeline)

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

> [!NOTE]
> **CSI-capable hardware required.** Pose estimation, vital signs, and through-wall sensing rely on Channel State Information (CSI) — per-subcarrier amplitude and phase data that standard consumer WiFi does not expose. You need CSI-capable hardware (ESP32-S3 or a research NIC) for full functionality. Consumer WiFi laptops can only provide RSSI-based presence detection, which is significantly less capable.

> **Hardware options** for live CSI capture:
>
> | Option | Hardware | Cost | Full CSI | Capabilities |
> |--------|----------|------|----------|-------------|
> | **ESP32 Mesh** (recommended) | 3-6x ESP32-S3 + WiFi router | ~$54 | Yes | Pose, breathing, heartbeat, motion, presence |
> | **Research NIC** | Intel 5300 / Atheros AR9580 | ~$50-100 | Yes | Full CSI with 3x3 MIMO |
> | **Any WiFi** | Windows/Linux laptop | $0 | No | RSSI-only: coarse presence and motion |
>
> No hardware? Verify the signal processing pipeline with the deterministic reference signal: `python v1/data/proof/verify.py`

---

## 🚀 Key Features

| | Feature | What It Means |
|---|---------|---------------|
| 🔒 | **Privacy-First** | Tracks human pose using only WiFi signals — no cameras, no video, no images stored |
| ⚡ | **Real-Time** | Analyzes WiFi signals in under 100 microseconds per frame — fast enough for live monitoring |
| 💓 | **Vital Signs** | Detects breathing rate (6-30 breaths/min) and heart rate (40-120 bpm) without any wearable |
| 👥 | **Multi-Person** | Tracks multiple people simultaneously, each with independent pose and vitals — no hard software limit (physics: ~3-5 per AP with 56 subcarriers, more with multi-AP) |
| 🧱 | **Through-Wall** | WiFi passes through walls, furniture, and debris — works where cameras cannot |
| 🚑 | **Disaster Response** | Detects trapped survivors through rubble and classifies injury severity (START triage) |
| 🐳 | **One-Command Setup** | `docker pull ruvnet/wifi-densepose:latest` — live sensing in 30 seconds, no toolchain needed |
| 📦 | **Portable Models** | Trained models package into a single `.rvf` file — runs on edge, cloud, or browser (WASM) |
| 🦀 | **810x Faster** | Complete Rust rewrite: 54,000 frames/sec pipeline, 132 MB Docker image, 542+ tests |

---

<details>
<summary><strong>🏢 Use Cases & Applications</strong> — From hospital rooms to concert halls</summary>

WiFi sensing works anywhere WiFi exists. No new hardware in most cases — just software on existing access points or a $8 ESP32 add-on. Because there are no cameras, deployments avoid privacy regulations (GDPR video, HIPAA imaging) by design.

**Scaling note:** Each access point can distinguish ~3-5 people using 56 subcarriers (ESP32-S3). Multi-AP setups multiply capacity linearly — a 4-AP mesh in a retail store covers ~15-20 simultaneous occupants. There is no hard software limit; the `persons: Vec<PersonPose>` vector grows dynamically. The practical ceiling is signal physics: subcarrier count, AP placement, and room geometry.

### Everyday — Proven with commodity WiFi

| Use Case | What It Does | Hardware Needed |
|----------|-------------|-----------------|
| **Elderly care / assisted living** | Fall detection, nighttime activity monitoring, breathing rate during sleep — no wearable compliance needed | 1 ESP32-S3 per room ($8) |
| **Hospital patient monitoring** | Continuous breathing + heart rate for non-critical beds without wired sensors; nurse alert on anomaly | 1-2 APs per ward, existing WiFi |
| **Emergency room triage** | Automated occupancy count + wait-time estimation; detect patient distress (abnormal breathing) in waiting areas | Existing hospital WiFi |
| **Retail occupancy & flow** | Real-time foot traffic, dwell time by zone, queue length — no cameras, no opt-in, GDPR-friendly | Existing store WiFi + 1 ESP32 |
| **Office space utilization** | Which desks/rooms are actually occupied, meeting room no-shows, HVAC optimization based on real presence | Existing enterprise WiFi |
| **Hotel & hospitality** | Room occupancy without door sensors, minibar/bathroom usage patterns, energy savings on empty rooms | Existing hotel WiFi |

### Specialized — Requires CSI-capable hardware

| Use Case | What It Does | Hardware Needed |
|----------|-------------|-----------------|
| **Smart home automation** | Room-level presence triggers (lights, HVAC, music) that work through walls — no dead zones, no motion-sensor timeouts | 2-3 ESP32-S3 nodes ($24) |
| **Fitness & sports** | Rep counting, posture correction, breathing cadence during exercise — no wearable, no camera in locker rooms | 3+ ESP32-S3 mesh |
| **Childcare & schools** | Naptime breathing monitoring, playground headcount, restricted-area alerts — privacy-safe for minors | 2-4 ESP32-S3 per zone |
| **Event venues & concerts** | Crowd density mapping, crush-risk detection via breathing compression, emergency evacuation flow tracking | Multi-AP mesh (4-8 APs) |
| **Warehouse & logistics** | Worker safety zones, forklift proximity alerts, occupancy in hazardous areas — works through shelving and pallets | Industrial AP mesh |
| **Civic infrastructure** | Public restroom occupancy (no cameras possible), subway platform crowding, shelter headcount during emergencies | Municipal WiFi + ESP32 |

### Extreme — Through-wall and disaster scenarios

| Use Case | What It Does | Hardware Needed |
|----------|-------------|-----------------|
| **Search & rescue (WiFi-Mat)** | Detect survivors through rubble/debris via breathing signature, START triage color classification, 3D localization | Portable ESP32 mesh + laptop |
| **Prison & secure facilities** | Cell occupancy verification, distress detection (abnormal vitals), perimeter sensing — no camera blind spots | Dedicated AP infrastructure |
| **Military / tactical** | Through-wall personnel detection, room clearing confirmation, hostage vital signs at standoff distance | Directional WiFi + custom firmware |
| **Mining & underground** | Worker presence in tunnels where GPS/cameras fail, breathing detection after collapse, headcount at safety points | Ruggedized ESP32 mesh |
| **Wildlife research** | Non-invasive animal activity monitoring in enclosures or dens — no light pollution, no visual disturbance | Weatherproof ESP32 nodes |

</details>

---

## 📦 Installation

<details>
<summary><strong>Guided Installer</strong> — Interactive hardware detection and profile selection</summary>

```bash
./install.sh
```

The installer walks through 7 steps: system detection, toolchain check, WiFi hardware scan, profile recommendation, dependency install, build, and verification.

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

```bash
# Non-interactive
./install.sh --profile rust --yes

# Hardware check only
./install.sh --check-only
```

</details>

<details>
<summary><strong>From Source</strong> — Rust (primary) or Python</summary>

```bash
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose

# Rust (primary — 810x faster)
cd rust-port/wifi-densepose-rs
cargo build --release
cargo test --workspace

# Python (legacy v1)
pip install -r requirements.txt
pip install -e .

# Or via pip
pip install wifi-densepose
pip install wifi-densepose[gpu]   # GPU acceleration
pip install wifi-densepose[all]   # All optional deps
```

</details>

<details>
<summary><strong>Docker</strong> — Pre-built images, no toolchain needed</summary>

```bash
# Rust sensing server (132 MB — recommended)
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 -p 3001:3001 -p 5005:5005/udp ruvnet/wifi-densepose:latest

# Python sensing pipeline (569 MB)
docker pull ruvnet/wifi-densepose:python
docker run -p 8765:8765 -p 8080:8080 ruvnet/wifi-densepose:python

# Both via docker-compose
cd docker && docker compose up

# Export RVF model
docker run --rm -v $(pwd):/out ruvnet/wifi-densepose:latest --export-rvf /out/model.rvf
```

| Image | Tag | Size | Ports |
|-------|-----|------|-------|
| `ruvnet/wifi-densepose` | `latest`, `rust` | 132 MB | 3000 (REST), 3001 (WS), 5005/udp (ESP32) |
| `ruvnet/wifi-densepose` | `python` | 569 MB | 8765 (WS), 8080 (UI) |

</details>

<details>
<summary><strong>System Requirements</strong></summary>

- **Rust**: 1.70+ (primary runtime — install via [rustup](https://rustup.rs/))
- **Python**: 3.8+ (for verification and legacy v1 API)
- **OS**: Linux (Ubuntu 18.04+), macOS (10.15+), Windows 10+
- **Memory**: Minimum 4GB RAM, Recommended 8GB+
- **Storage**: 2GB free space for models and data
- **Network**: WiFi interface with CSI capability (optional — installer detects what you have)
- **GPU**: Optional (NVIDIA CUDA or Apple Metal)

</details>

---

## 🚀 Quick Start

<details open>
<summary><strong>First API call in 3 commands</strong></summary>

### 1. Install

```bash
# Fastest path — Docker
docker pull ruvnet/wifi-densepose:latest
docker run -p 3000:3000 ruvnet/wifi-densepose:latest

# Or from source (Rust)
./install.sh --profile rust --yes
```

### 2. Start the System

```python
from wifi_densepose import WiFiDensePose

system = WiFiDensePose()
system.start()
poses = system.get_latest_poses()
print(f"Detected {len(poses)} persons")
system.stop()
```

### 3. REST API

```bash
# Health check
curl http://localhost:3000/api/v1/health

# Latest sensing frame
curl http://localhost:3000/api/v1/sensing

# Vital signs
curl http://localhost:3000/api/v1/vital-signs
```

### 4. Real-time WebSocket

```python
import asyncio, websockets, json

async def stream():
    async with websockets.connect("ws://localhost:3001/ws/sensing") as ws:
        async for msg in ws:
            data = json.loads(msg)
            print(f"Persons: {len(data.get('persons', []))}")

asyncio.run(stream())
```

</details>

---

## 📋 Table of Contents

<details open>
<summary><strong>📡 Signal Processing & Sensing</strong> — From raw WiFi frames to vital signs</summary>

The signal processing stack transforms raw WiFi Channel State Information into actionable human sensing data. Starting from 56-192 subcarrier complex values captured at 20 Hz, the pipeline applies research-grade algorithms (SpotFi phase correction, Hampel outlier rejection, Fresnel zone modeling) to extract breathing rate, heart rate, motion level, and multi-person body pose — all in pure Rust with zero external ML dependencies.

| Section | Description | Docs |
|---------|-------------|------|
| [Key Features](#key-features) | Privacy-first sensing, real-time performance, multi-person tracking, Docker | — |
| [ESP32-S3 Hardware Pipeline](#esp32-s3-hardware-pipeline) | 20 Hz CSI streaming, binary frame parsing, flash & provision | [ADR-018](docs/adr/ADR-018-esp32-dev-implementation.md) · [Tutorial #34](https://github.com/ruvnet/wifi-densepose/issues/34) |
| [Vital Sign Detection](#vital-sign-detection) | Breathing 6-30 BPM, heartbeat 40-120 BPM, FFT peak detection | [ADR-021](docs/adr/ADR-021-vital-sign-detection-rvdna-pipeline.md) |
| [WiFi Scan Domain Layer](#wifi-scan-domain-layer) | 8-stage RSSI pipeline, multi-BSSID fingerprinting, Windows WiFi | [ADR-022](docs/adr/ADR-022-windows-wifi-enhanced-fidelity-ruvector.md) · [Tutorial #36](https://github.com/ruvnet/wifi-densepose/issues/36) |
| [WiFi-Mat Disaster Response](#wifi-mat-disaster-response) | Search & rescue, START triage, 3D localization through debris | [ADR-001](docs/adr/ADR-001-wifi-mat-disaster-detection.md) · [User Guide](docs/wifi-mat-user-guide.md) |
| [SOTA Signal Processing](#sota-signal-processing) | SpotFi, Hampel, Fresnel, STFT spectrogram, subcarrier selection, BVP | [ADR-014](docs/adr/ADR-014-sota-signal-processing.md) |

</details>

<details>
<summary><strong>🧠 Models & Training</strong> — DensePose pipeline, RVF containers, SONA adaptation</summary>

The neural pipeline uses a graph transformer with cross-attention to map CSI feature matrices to 17 COCO body keypoints and DensePose UV coordinates. Models are packaged as single-file `.rvf` containers with progressive loading (Layer A instant, Layer B warm, Layer C full). SONA (Self-Optimizing Neural Architecture) enables continuous on-device adaptation via micro-LoRA + EWC++ without catastrophic forgetting.

| Section | Description | Docs |
|---------|-------------|------|
| [RVF Model Container](#rvf-model-container) | Binary packaging with Ed25519 signing, progressive 3-layer loading, SIMD quantization | [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md) |
| [Training & Fine-Tuning](#training--fine-tuning) | MM-Fi/Wi-Pose pre-training, 6-term composite loss, cosine-scheduled SGD, SONA LoRA | [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md) |
| [RuVector Crates](#ruvector-crates) | 11 vendored Rust crates: HNSW, attention, GNN, temporal compression, min-cut, solver | [Source](vendor/ruvector/) |

</details>

<details>
<summary><strong>🖥️ Usage & Configuration</strong> — CLI flags, API endpoints, hardware setup</summary>

The Rust sensing server is the primary interface, offering a comprehensive CLI with flags for data source selection, model loading, training, benchmarking, and RVF export. A REST API (Axum) and WebSocket server provide real-time data access. The Python v1 CLI remains available for legacy workflows.

| Section | Description | Docs |
|---------|-------------|------|
| [CLI Usage](#cli-usage) | `--source`, `--train`, `--benchmark`, `--export-rvf`, `--model`, `--progressive` | — |
| [REST API & WebSocket](#rest-api--websocket) | 6 REST endpoints (sensing, vitals, BSSID, SONA), WebSocket real-time stream | — |
| [Hardware Support](#hardware-support-1) | ESP32-S3 ($8), Intel 5300 ($15), Atheros AR9580 ($20), Windows RSSI ($0) | [ADR-012](docs/adr/ADR-012-esp32-csi-sensor-mesh.md) · [ADR-013](docs/adr/ADR-013-feature-level-sensing-commodity-gear.md) |

</details>

<details>
<summary><strong>⚙️ Development & Testing</strong> — 542+ tests, CI, deployment</summary>

The project maintains 542+ pure-Rust tests across 7 crate suites with zero mocks — every test runs against real algorithm implementations. Hardware-free simulation mode (`--source simulate`) enables full-stack testing without physical devices. Docker images are published on Docker Hub for zero-setup deployment.

| Section | Description | Docs |
|---------|-------------|------|
| [Testing](#testing) | 7 test suites: sensing-server (229), signal (83), mat (139), wifiscan (91), RVF (16), vitals (18) | — |
| [Deployment](#deployment) | Docker images (132 MB Rust / 569 MB Python), docker-compose, env vars | — |
| [Contributing](#contributing) | Fork → branch → test → PR workflow, Rust and Python dev setup | — |

</details>

<details>
<summary><strong>📊 Performance & Benchmarks</strong> — Measured throughput, latency, resource usage</summary>

All benchmarks are measured on the Rust sensing server using `cargo bench` and the built-in `--benchmark` CLI flag. The Rust v2 implementation delivers 810x end-to-end speedup over the Python v1 baseline, with motion detection reaching 5,400x improvement. The vital sign detector processes 11,665 frames/second in a single-threaded benchmark.

| Section | Description | Key Metric |
|---------|-------------|------------|
| [Performance Metrics](#performance-metrics) | Vital signs, CSI pipeline, motion detection, Docker image, memory | 11,665 fps vitals · 54K fps pipeline |
| [Rust vs Python](#python-vs-rust) | Side-by-side benchmarks across 5 operations | **810x** full pipeline speedup |

</details>

<details>
<summary><strong>📄 Meta</strong> — License, changelog, support</summary>

WiFi DensePose is MIT-licensed open source, developed by [ruvnet](https://github.com/ruvnet). The project has been in active development since March 2025, with 3 major releases delivering the Rust port, SOTA signal processing, disaster response module, and end-to-end training pipeline.

| Section | Description | Link |
|---------|-------------|------|
| [Changelog](#changelog) | v2.3.0 (training pipeline + Docker), v2.2.0 (SOTA + WiFi-Mat), v2.1.0 (Rust port) | — |
| [License](#license) | MIT License | [LICENSE](LICENSE) |
| [Support](#support) | Bug reports, feature requests, community discussion | [Issues](https://github.com/ruvnet/wifi-densepose/issues) · [Discussions](https://github.com/ruvnet/wifi-densepose/discussions) |

</details>

---

## 📡 Signal Processing & Sensing

<details>
<summary><a id="esp32-s3-hardware-pipeline"></a><strong>📡 ESP32-S3 Hardware Pipeline (ADR-018)</strong> — 20 Hz CSI streaming, flash & provision</summary>

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

<details>
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
<summary><a id="vital-sign-detection"></a><strong>💓 Vital Sign Detection (ADR-021)</strong> — Breathing and heartbeat via FFT</summary>

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
<summary><a id="wifi-scan-domain-layer"></a><strong>📡 WiFi Scan Domain Layer (ADR-022)</strong> — 8-stage RSSI pipeline for Windows WiFi</summary>

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
<summary><a id="wifi-mat-disaster-response"></a><strong>🚨 WiFi-Mat: Disaster Response</strong> — Search & rescue, START triage, 3D localization</summary>

WiFi signals penetrate non-metallic debris (concrete, wood, drywall) where cameras and thermal sensors cannot reach. The WiFi-Mat module (`wifi-densepose-mat`, 139 tests) uses CSI analysis to detect survivors trapped under rubble, classify their condition using the START triage protocol, and estimate their 3D position — giving rescue teams actionable intelligence within seconds of deployment.

| Capability | How It Works | Performance Target |
|------------|-------------|-------------------|
| **Breathing Detection** | Bandpass 0.07-1.0 Hz + Fresnel zone modeling detects chest displacement of 5-10mm at 5 GHz | 4-60 BPM, <500ms latency |
| **Heartbeat Detection** | Micro-Doppler shift extraction from fine-grained CSI phase variation | Via ruvector-temporal-tensor |
| **3D Localization** | Multi-AP triangulation + CSI fingerprint matching + depth estimation through rubble layers | 3-5m penetration |
| **START Triage** | Ensemble classifier votes on breathing + movement + vital stability → P1-P4 priority | <1% false negative |
| **Zone Scanning** | 16+ concurrent scan zones with periodic re-scan and audit logging | Full disaster site |

**Triage classification (START protocol compatible):**

| Status | Color | Detection Criteria | Priority |
|--------|-------|-------------------|----------|
| Immediate | Red | Breathing detected, no movement | P1 |
| Delayed | Yellow | Movement + breathing, stable vitals | P2 |
| Minor | Green | Strong movement, responsive patterns | P3 |
| Deceased | Black | No vitals for >30 min continuous scan | P4 |

**Deployment modes:** portable (single TX/RX handheld), distributed (multiple APs around collapse site), drone-mounted (UAV scanning), vehicle-mounted (mobile command post).

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

**Safety guarantees:** fail-safe defaults (assume life present on ambiguous signals), redundant multi-algorithm voting, complete audit trail, offline-capable (no network required).

- [WiFi-Mat User Guide](docs/wifi-mat-user-guide.md) | [ADR-001](docs/adr/ADR-001-wifi-mat-disaster-detection.md) | [Domain Model](docs/ddd/wifi-mat-domain-model.md)

</details>

<details>
<summary><a id="sota-signal-processing"></a><strong>🔬 SOTA Signal Processing (ADR-014)</strong> — 6 research-grade algorithms</summary>

The signal processing layer bridges the gap between raw commodity WiFi hardware output and research-grade sensing accuracy. Each algorithm addresses a specific limitation of naive CSI processing — from hardware-induced phase corruption to environment-dependent multipath interference. All six are implemented in `wifi-densepose-signal/src/` with deterministic tests and no mock data.

| Algorithm | What It Does | Why It Matters | Math | Source |
|-----------|-------------|----------------|------|--------|
| **Conjugate Multiplication** | Multiplies CSI antenna pairs: `H₁[k] × conj(H₂[k])` | Cancels CFO, SFO, and packet detection delay that corrupt raw phase — preserves only environment-caused phase differences | `CSI_ratio[k] = H₁[k] * conj(H₂[k])` | [SpotFi](https://dl.acm.org/doi/10.1145/2789168.2790124) (SIGCOMM 2015) |
| **Hampel Filter** | Replaces outliers using running median ± scaled MAD | Z-score uses mean/std which are corrupted by the very outliers it detects (masking effect). Hampel uses median/MAD, resisting up to 50% contamination | `σ̂ = 1.4826 × MAD` | Standard DSP; WiGest (2015) |
| **Fresnel Zone Model** | Models signal variation from chest displacement crossing Fresnel zone boundaries | Zero-crossing counting fails in multipath-rich environments. Fresnel predicts *where* breathing should appear based on TX-RX-body geometry | `ΔΦ = 2π × 2Δd / λ`, `A = \|sin(ΔΦ/2)\|` | [FarSense](https://dl.acm.org/doi/10.1145/3300061.3345431) (MobiCom 2019) |
| **CSI Spectrogram** | Sliding-window FFT (STFT) per subcarrier → 2D time-frequency matrix | Breathing = 0.2-0.4 Hz band, walking = 1-2 Hz, static = noise. 2D structure enables CNN spatial pattern recognition that 1D features miss | `S[t,f] = \|Σₙ x[n] w[n-t] e^{-j2πfn}\|²` | Standard since 2018 |
| **Subcarrier Selection** | Ranks subcarriers by motion sensitivity (variance ratio) and selects top-K | Not all subcarriers respond to motion — some sit in multipath nulls. Selecting the 10-20 most sensitive improves SNR by 6-10 dB | `sensitivity[k] = var_motion / var_static` | [WiDance](https://dl.acm.org/doi/10.1145/3117811.3117826) (MobiCom 2017) |
| **Body Velocity Profile** | Extracts velocity distribution from Doppler shifts across subcarriers | BVP is domain-independent — same velocity profile regardless of room layout, furniture, or AP placement. Basis for cross-environment recognition | `BVP[v,t] = Σₖ \|STFTₖ[v,t]\|` | [Widar 3.0](https://dl.acm.org/doi/10.1145/3328916) (MobiSys 2019) |

**Processing pipeline order:** Raw CSI → Conjugate multiplication (phase cleaning) → Hampel filter (outlier removal) → Subcarrier selection (top-K) → CSI spectrogram (time-frequency) → Fresnel model (breathing) + BVP (activity)

See [ADR-014](docs/adr/ADR-014-sota-signal-processing.md) for full mathematical derivations.

</details>

---

## 🧠 Models & Training

<details>
<summary><a id="rvf-model-container"></a><strong>📦 RVF Model Container</strong> — Single-file deployment with progressive loading</summary>

The RuVector Format (RVF) packages an entire trained model — weights, HNSW indexes, quantization codebooks, SONA adaptation deltas, and WASM inference runtime — into a single self-contained binary file. No external dependencies are needed at deployment time.

**Container structure:**

```
┌──────────────────────────────────────────────────────┐
│ RVF Container (.rvf)                                  │
│                                                       │
│  ┌─────────────┐  64-byte header per segment          │
│  │ Manifest     │  Magic: 0x52564653 ("RVFS")         │
│  ├─────────────┤  Type + content hash + compression   │
│  │ Weights      │  Model parameters (f32/f16/u8)      │
│  ├─────────────┤                                      │
│  │ HNSW Index   │  Vector search index                │
│  ├─────────────┤                                      │
│  │ Quant        │  Quantization codebooks              │
│  ├─────────────┤                                      │
│  │ SONA Profile │  LoRA deltas + EWC++ Fisher matrix  │
│  ├─────────────┤                                      │
│  │ Witness      │  Ed25519 training proof              │
│  ├─────────────┤                                      │
│  │ Vitals Config│  Breathing/HR filter parameters     │
│  └─────────────┘                                      │
└──────────────────────────────────────────────────────┘
```

| Property | Detail |
|----------|--------|
| **Format** | Segment-based binary, 20+ segment types, CRC32 integrity per segment |
| **Progressive Loading** | **Layer A** (<5ms): manifest + entry points → **Layer B** (100ms-1s): hot weights + adjacency → **Layer C** (seconds): full graph |
| **Signing** | Ed25519 training proofs for verifiable provenance — chain of custody from training data to deployed model |
| **Quantization** | Per-segment temperature-tiered: f32 (full), f16 (half), u8 (int8), binary — with SIMD-accelerated distance computation |
| **CLI** | `--export-rvf` (generate), `--load-rvf` (config), `--save-rvf` (persist), `--model` (inference), `--progressive` (3-layer load) |
| **Size** | Typical: 13 KB (sensing-only) to 50+ MB (full DensePose) |

```bash
# Export model package
./target/release/sensing-server --export-rvf wifi-densepose-v1.rvf

# Load and run with progressive loading
./target/release/sensing-server --model wifi-densepose-v1.rvf --progressive

# Export via Docker
docker run --rm -v $(pwd):/out ruvnet/wifi-densepose:latest --export-rvf /out/model.rvf
```

See [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md).

</details>

<details>
<summary><a id="training--fine-tuning"></a><strong>🧬 Training & Fine-Tuning</strong> — MM-Fi/Wi-Pose pre-training, SONA adaptation</summary>

The training pipeline implements 8 phases in pure Rust (7,832 lines, zero external ML dependencies). It trains a graph transformer with cross-attention to map CSI feature matrices to 17 COCO body keypoints and DensePose UV coordinates — following the approach from the CMU "DensePose From WiFi" paper (arXiv:2301.00250).

**Three-tier data strategy:**

| Tier | Method | Purpose |
|------|--------|---------|
| **1. Pre-train** | MM-Fi + Wi-Pose public datasets | Cross-environment generalization (multi-subject, multi-room) |
| **2. Fine-tune** | ESP32 CSI + camera pseudo-labels | Environment-specific multipath adaptation |
| **3. SONA adapt** | Micro-LoRA (rank-4) + EWC++ | Continuous on-device learning without catastrophic forgetting |

**Training pipeline components:**

| Phase | Module | What It Does |
|-------|--------|-------------|
| 1 | `dataset.rs` (850 lines) | MM-Fi `.npy` + Wi-Pose `.mat` loaders, subcarrier resampling (114→56, 30→56), windowing, normalization |
| 2 | `graph_transformer.rs` (855 lines) | COCO BodyGraph (17 kp, 16 edges), AntennaGraph, multi-head CrossAttention, GCN message passing |
| 3 | `trainer.rs` (881 lines) | 6-term composite loss (MSE, cross-entropy, UV regression, temporal consistency, bone length, symmetry), SGD+momentum, cosine+warmup scheduler, PCK/OKS metrics |
| 4 | `sona.rs` (639 lines) | LoRA adapters (A×B delta), EWC++ Fisher regularization, EnvironmentDetector (3-sigma drift detection) |
| 5 | `sparse_inference.rs` (753 lines) | NeuronProfiler hot/cold partitioning, SparseLinear (skip cold rows), INT8/FP16 quantization with <0.01 MSE |
| 6 | `rvf_pipeline.rs` (1,027 lines) | Progressive 3-layer loader, HNSW index, OverlayGraph, `RvfModelBuilder` |
| 7 | `rvf_container.rs` (914 lines) | Binary container format, 6+ segment types, CRC32 integrity |
| 8 | `main.rs` integration | `--train`, `--model`, `--progressive` CLI flags, REST endpoints |

**Best-epoch snapshotting**: the trainer saves the best validation loss weights and restores them before checkpoint/export — prevents exporting overfit final-epoch parameters.

```bash
# Pre-train on MM-Fi dataset
./target/release/sensing-server --train --dataset data/ --dataset-type mmfi --epochs 100

# Train and export to RVF in one step
./target/release/sensing-server --train --dataset data/ --epochs 100 --save-rvf model.rvf

# Via Docker (no toolchain needed)
docker run --rm -v $(pwd)/data:/data ruvnet/wifi-densepose:latest \
  --train --dataset /data --epochs 100 --export-rvf /data/model.rvf
```

See [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md).

</details>

<details>
<summary><a id="ruvector-crates"></a><strong>🔩 RuVector Crates</strong> — 11 vendored signal intelligence crates</summary>

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

---

<details>
<summary><strong>🏗️ System Architecture</strong> — End-to-end data flow from CSI capture to REST/WebSocket API</summary>

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

---

## 🖥️ CLI Usage

<details>
<summary><strong>Rust Sensing Server</strong> — Primary CLI interface</summary>

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

</details>

<details>
<summary><a id="rest-api--websocket"></a><strong>REST API & WebSocket</strong> — Endpoints reference</summary>

#### REST API (Rust Sensing Server)

```bash
GET  /api/v1/sensing              # Latest sensing frame
GET  /api/v1/vital-signs          # Breathing, heart rate, confidence
GET  /api/v1/bssid                # Multi-BSSID registry
GET  /api/v1/model/layers         # Progressive loading status
GET  /api/v1/model/sona/profiles  # SONA profiles
POST /api/v1/model/sona/activate  # Activate SONA profile
```

WebSocket: `ws://localhost:8765/ws/sensing` (real-time sensing + vital signs)

</details>

<details>
<summary><a id="hardware-support-1"></a><strong>Hardware Support</strong> — Devices, cost, and guides</summary>

| Hardware | CSI | Cost | Guide |
|----------|-----|------|-------|
| **ESP32-S3** | Native | ~$8 | [Tutorial #34](https://github.com/ruvnet/wifi-densepose/issues/34) |
| Intel 5300 | Firmware mod | ~$15 | Linux `iwl-csi` |
| Atheros AR9580 | ath9k patch | ~$20 | Linux only |
| Any Windows WiFi | RSSI only | $0 | [Tutorial #36](https://github.com/ruvnet/wifi-densepose/issues/36) |

</details>

<details>
<summary><strong>Python Legacy CLI</strong> — v1 API server commands</summary>

```bash
wifi-densepose start                    # Start API server
wifi-densepose -c config.yaml start     # Custom config
wifi-densepose -v start                 # Verbose logging
wifi-densepose status                   # Check status
wifi-densepose stop                     # Stop server
wifi-densepose config show              # Show configuration
wifi-densepose db init                  # Initialize database
wifi-densepose tasks list               # List background tasks
```

</details>

<details>
<summary><strong>Documentation Links</strong></summary>

- [WiFi-Mat User Guide](docs/wifi-mat-user-guide.md) | [Domain Model](docs/ddd/wifi-mat-domain-model.md)
- [ADR-021](docs/adr/ADR-021-vital-sign-detection-rvdna-pipeline.md) | [ADR-022](docs/adr/ADR-022-windows-wifi-enhanced-fidelity-ruvector.md) | [ADR-023](docs/adr/ADR-023-trained-densepose-model-ruvector-pipeline.md)

</details>

---

## 🧪 Testing

<details>
<summary><strong>542+ tests across 7 suites</strong> — zero mocks, hardware-free simulation</summary>

```bash
# Rust tests (primary — 542+ tests)
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

---

## 🚀 Deployment

<details>
<summary><strong>Docker deployment</strong> — Production setup with docker-compose</summary>

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

---

## 📊 Performance Metrics

<details>
<summary><strong>Measured benchmarks</strong> — Rust sensing server, validated via cargo bench</summary>

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

---

## 🤝 Contributing

<details>
<summary><strong>Dev setup, code standards, PR process</strong></summary>

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

---

## 📄 Changelog

<details>
<summary><strong>Release history</strong></summary>

### v2.3.0 — 2026-03-01

The largest release to date — delivers the complete end-to-end training pipeline, Docker images, and vital sign detection. The Rust sensing server now supports full model training, RVF export, and progressive model loading from a single binary.

- **Docker images published** — `ruvnet/wifi-densepose:latest` (132 MB Rust) and `:python` (569 MB)
- **8-phase DensePose training pipeline (ADR-023)** — Dataset loaders (MM-Fi, Wi-Pose), graph transformer with cross-attention, 6-term composite loss, cosine-scheduled SGD, PCK/OKS validation, SONA adaptation, sparse inference engine, RVF model packaging
- **`--export-rvf` CLI flag** — Standalone RVF model container generation with vital config, training proof, and SONA profiles
- **`--train` CLI flag** — Full training mode with best-epoch snapshotting and checkpoint saving
- **Vital sign detection (ADR-021)** — FFT-based breathing (6-30 BPM) and heartbeat (40-120 BPM) extraction, 11,665 fps benchmark
- **WiFi scan domain layer (ADR-022)** — 8-stage pure-Rust signal intelligence pipeline for Windows WiFi RSSI
- **New crates** — `wifi-densepose-vitals` (1,863 lines) and `wifi-densepose-wifiscan` (4,829 lines)
- **542+ Rust tests** — All passing, zero mocks

### v2.2.0 — 2026-02-28

Introduced the guided installer, SOTA signal processing algorithms, and the WiFi-Mat disaster response module. This release established the ESP32 hardware path and security hardening.

- **Guided installer** — `./install.sh` with 7-step hardware detection and 8 install profiles
- **6 SOTA signal algorithms (ADR-014)** — SpotFi conjugate multiplication, Hampel filter, Fresnel zone model, CSI spectrogram, subcarrier selection, body velocity profile
- **WiFi-Mat disaster response** — START triage, scan zones, 3D localization, priority alerts — 139 tests
- **ESP32 CSI hardware parser** — Binary frame parsing with I/Q extraction — 28 tests
- **Security hardening** — 10 vulnerabilities fixed (CVE remediation, input validation, path security)

### v2.1.0 — 2026-02-28

The foundational Rust release — ported the Python v1 pipeline to Rust with 810x speedup, integrated the RuVector signal intelligence crates, and added the Three.js real-time visualization.

- **RuVector integration** — 11 vendored crates (ADR-002 through ADR-013) for HNSW indexing, attention, GNN, temporal compression, min-cut, solver
- **ESP32 CSI sensor mesh** — $54 starter kit with 3-6 ESP32-S3 nodes streaming at 20 Hz
- **Three.js visualization** — 3D body model with 17 joints, real-time WebSocket streaming
- **CI verification pipeline** — Determinism checks and unseeded random scan across all signal operations

</details>

---

## 📄 License

MIT License — see [LICENSE](LICENSE) for details.

## 📞 Support

[GitHub Issues](https://github.com/ruvnet/wifi-densepose/issues) | [Discussions](https://github.com/ruvnet/wifi-densepose/discussions) | [PyPI](https://pypi.org/project/wifi-densepose/)

---

**WiFi DensePose** — Privacy-preserving human pose estimation through WiFi signals.
