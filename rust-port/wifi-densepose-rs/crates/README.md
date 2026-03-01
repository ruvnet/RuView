# WiFi-DensePose Rust Crates

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org/)
[![Workspace](https://img.shields.io/badge/workspace-14%20crates-green.svg)](https://github.com/ruvnet/wifi-densepose)
[![RuVector v2.0.4](https://img.shields.io/badge/ruvector-v2.0.4-purple.svg)](https://crates.io/crates/ruvector-mincut)
[![Tests](https://img.shields.io/badge/tests-542%2B-brightgreen.svg)](#testing)

**See through walls with WiFi. No cameras. No wearables. Just radio waves.**

A modular Rust workspace for WiFi-based human pose estimation, vital sign monitoring, and disaster response using Channel State Information (CSI). Built on [RuVector](https://crates.io/crates/ruvector-mincut) graph algorithms and the [WiFi-DensePose](https://github.com/ruvnet/wifi-densepose) research platform by [rUv](https://github.com/ruvnet).

---

## Performance

| Operation | Python v1 | Rust v2 | Speedup |
|-----------|-----------|---------|---------|
| CSI Preprocessing | ~5 ms | 5.19 us | **~1000x** |
| Phase Sanitization | ~3 ms | 3.84 us | **~780x** |
| Feature Extraction | ~8 ms | 9.03 us | **~890x** |
| Motion Detection | ~1 ms | 186 ns | **~5400x** |
| Full Pipeline | ~15 ms | 18.47 us | **~810x** |
| Vital Signs | N/A | 86 us (11,665 fps) | -- |

## Crate Overview

### Core Foundation

| Crate | Description | crates.io |
|-------|-------------|-----------|
| [`wifi-densepose-core`](wifi-densepose-core/) | Types, traits, and utilities (`CsiFrame`, `PoseEstimate`, `SignalProcessor`) | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-core.svg)](https://crates.io/crates/wifi-densepose-core) |
| [`wifi-densepose-config`](wifi-densepose-config/) | Configuration management (env, TOML, YAML) | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-config.svg)](https://crates.io/crates/wifi-densepose-config) |
| [`wifi-densepose-db`](wifi-densepose-db/) | Database persistence (PostgreSQL, SQLite, Redis) | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-db.svg)](https://crates.io/crates/wifi-densepose-db) |

### Signal Processing & Sensing

| Crate | Description | RuVector Integration | crates.io |
|-------|-------------|---------------------|-----------|
| [`wifi-densepose-signal`](wifi-densepose-signal/) | SOTA CSI signal processing (6 algorithms from SpotFi, FarSense, Widar 3.0) | `ruvector-mincut`, `ruvector-attn-mincut`, `ruvector-attention`, `ruvector-solver` | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-signal.svg)](https://crates.io/crates/wifi-densepose-signal) |
| [`wifi-densepose-vitals`](wifi-densepose-vitals/) | Vital sign extraction: breathing (6-30 BPM) and heart rate (40-120 BPM) | -- | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-vitals.svg)](https://crates.io/crates/wifi-densepose-vitals) |
| [`wifi-densepose-wifiscan`](wifi-densepose-wifiscan/) | Multi-BSSID WiFi scanning for Windows-enhanced sensing | -- | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-wifiscan.svg)](https://crates.io/crates/wifi-densepose-wifiscan) |

### Neural Network & Training

| Crate | Description | RuVector Integration | crates.io |
|-------|-------------|---------------------|-----------|
| [`wifi-densepose-nn`](wifi-densepose-nn/) | Multi-backend inference (ONNX, PyTorch, Candle) with DensePose head (24 body parts) | -- | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-nn.svg)](https://crates.io/crates/wifi-densepose-nn) |
| [`wifi-densepose-train`](wifi-densepose-train/) | Training pipeline with MM-Fi dataset, 114->56 subcarrier interpolation | **All 5 crates** | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-train.svg)](https://crates.io/crates/wifi-densepose-train) |

### Disaster Response

| Crate | Description | RuVector Integration | crates.io |
|-------|-------------|---------------------|-----------|
| [`wifi-densepose-mat`](wifi-densepose-mat/) | Mass Casualty Assessment Tool -- survivor detection, triage, multi-AP localization | `ruvector-solver`, `ruvector-temporal-tensor` | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-mat.svg)](https://crates.io/crates/wifi-densepose-mat) |

### Hardware & Deployment

| Crate | Description | crates.io |
|-------|-------------|-----------|
| [`wifi-densepose-hardware`](wifi-densepose-hardware/) | ESP32, Intel 5300, Atheros CSI sensor interfaces (pure Rust, no FFI) | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-hardware.svg)](https://crates.io/crates/wifi-densepose-hardware) |
| [`wifi-densepose-wasm`](wifi-densepose-wasm/) | WebAssembly bindings for browser-based disaster dashboard | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-wasm.svg)](https://crates.io/crates/wifi-densepose-wasm) |
| [`wifi-densepose-sensing-server`](wifi-densepose-sensing-server/) | Axum server: ESP32 UDP ingestion, WebSocket broadcast, sensing UI | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-sensing-server.svg)](https://crates.io/crates/wifi-densepose-sensing-server) |

### Applications

| Crate | Description | crates.io |
|-------|-------------|-----------|
| [`wifi-densepose-api`](wifi-densepose-api/) | REST + WebSocket API layer | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-api.svg)](https://crates.io/crates/wifi-densepose-api) |
| [`wifi-densepose-cli`](wifi-densepose-cli/) | Command-line tool for MAT disaster scanning | [![crates.io](https://img.shields.io/crates/v/wifi-densepose-cli.svg)](https://crates.io/crates/wifi-densepose-cli) |

---

## Architecture

```
                          wifi-densepose-core
                         (types, traits, errors)
                                  |
              +-------------------+-------------------+
              |                   |                   |
    wifi-densepose-signal   wifi-densepose-nn   wifi-densepose-hardware
    (CSI processing)        (inference)         (ESP32, Intel 5300)
    + ruvector-mincut       + ONNX Runtime          |
    + ruvector-attn-mincut  + PyTorch (tch)   wifi-densepose-vitals
    + ruvector-attention    + Candle          (breathing, heart rate)
    + ruvector-solver            |
              |                  |             wifi-densepose-wifiscan
              +--------+---------+            (BSSID scanning)
                       |
          +------------+------------+
          |                         |
  wifi-densepose-train    wifi-densepose-mat
  (training pipeline)     (disaster response)
  + ALL 5 ruvector        + ruvector-solver
                          + ruvector-temporal-tensor
                                |
              +-----------------+-----------------+
              |                 |                 |
    wifi-densepose-api  wifi-densepose-wasm  wifi-densepose-cli
    (REST/WS)           (browser WASM)       (CLI tool)
              |
    wifi-densepose-sensing-server
    (Axum + WebSocket)
```

## RuVector Integration

All [RuVector](https://github.com/ruvnet/ruvector) crates at **v2.0.4** from crates.io:

| RuVector Crate | Used In | Purpose |
|----------------|---------|---------|
| [`ruvector-mincut`](https://crates.io/crates/ruvector-mincut) | signal, train | Dynamic min-cut for subcarrier selection & person matching |
| [`ruvector-attn-mincut`](https://crates.io/crates/ruvector-attn-mincut) | signal, train | Attention-weighted min-cut for antenna gating & spectrograms |
| [`ruvector-temporal-tensor`](https://crates.io/crates/ruvector-temporal-tensor) | train, mat | Tiered temporal compression (4-10x memory reduction) |
| [`ruvector-solver`](https://crates.io/crates/ruvector-solver) | signal, train, mat | Sparse Neumann solver for interpolation & triangulation |
| [`ruvector-attention`](https://crates.io/crates/ruvector-attention) | signal, train | Scaled dot-product attention for spatial features & BVP |

## Signal Processing Algorithms

Six state-of-the-art algorithms implemented in `wifi-densepose-signal`:

| Algorithm | Paper | Year | Module |
|-----------|-------|------|--------|
| Conjugate Multiplication | SpotFi (SIGCOMM) | 2015 | `csi_ratio.rs` |
| Hampel Filter | WiGest | 2015 | `hampel.rs` |
| Fresnel Zone Model | FarSense (MobiCom) | 2019 | `fresnel.rs` |
| CSI Spectrogram | Standard STFT | 2018+ | `spectrogram.rs` |
| Subcarrier Selection | WiDance (MobiCom) | 2017 | `subcarrier_selection.rs` |
| Body Velocity Profile | Widar 3.0 (MobiSys) | 2019 | `bvp.rs` |

## Quick Start

### As a Library

```rust
use wifi_densepose_core::{CsiFrame, CsiMetadata, SignalProcessor};
use wifi_densepose_signal::{CsiProcessor, CsiProcessorConfig};

// Configure the CSI processor
let config = CsiProcessorConfig::default();
let processor = CsiProcessor::new(config);

// Process a CSI frame
let frame = CsiFrame { /* ... */ };
let processed = processor.process(&frame)?;
```

### Vital Sign Monitoring

```rust
use wifi_densepose_vitals::{
    CsiVitalPreprocessor, BreathingExtractor, HeartRateExtractor,
    VitalAnomalyDetector,
};

let mut preprocessor = CsiVitalPreprocessor::new(56); // 56 subcarriers
let mut breathing = BreathingExtractor::new(100.0);    // 100 Hz sample rate
let mut heartrate = HeartRateExtractor::new(100.0);

// Feed CSI frames and extract vitals
for frame in csi_stream {
    let residuals = preprocessor.update(&frame.amplitudes);
    if let Some(bpm) = breathing.push_residuals(&residuals) {
        println!("Breathing: {:.1} BPM", bpm);
    }
}
```

### Disaster Response (MAT)

```rust
use wifi_densepose_mat::{DisasterResponse, DisasterConfig, DisasterType};

let config = DisasterConfig {
    disaster_type: DisasterType::Earthquake,
    max_scan_zones: 16,
    ..Default::default()
};

let mut responder = DisasterResponse::new(config);
responder.add_scan_zone(zone)?;
responder.start_continuous_scan().await?;
```

### Hardware (ESP32)

```rust
use wifi_densepose_hardware::{Esp32CsiParser, CsiFrame};

let parser = Esp32CsiParser::new();
let raw_bytes: &[u8] = /* UDP packet from ESP32 */;
let frame: CsiFrame = parser.parse(raw_bytes)?;
println!("RSSI: {} dBm, {} subcarriers", frame.metadata.rssi, frame.subcarriers.len());
```

### Training

```bash
# Check training crate (no GPU needed)
cargo check -p wifi-densepose-train --no-default-features

# Run training with GPU (requires tch/libtorch)
cargo run -p wifi-densepose-train --features tch-backend --bin train -- \
    --config training.toml --dataset /path/to/mmfi

# Verify deterministic training proof
cargo run -p wifi-densepose-train --features tch-backend --bin verify-training
```

## Building

```bash
# Clone the repository
git clone https://github.com/ruvnet/wifi-densepose.git
cd wifi-densepose/rust-port/wifi-densepose-rs

# Check workspace (no GPU dependencies)
cargo check --workspace --no-default-features

# Run all tests
cargo test --workspace --no-default-features

# Build release
cargo build --release --workspace
```

### Feature Flags

| Crate | Feature | Description |
|-------|---------|-------------|
| `wifi-densepose-nn` | `onnx` (default) | ONNX Runtime backend |
| `wifi-densepose-nn` | `tch-backend` | PyTorch (libtorch) backend |
| `wifi-densepose-nn` | `candle-backend` | Candle (pure Rust) backend |
| `wifi-densepose-nn` | `cuda` | CUDA GPU acceleration |
| `wifi-densepose-train` | `tch-backend` | Enable GPU training modules |
| `wifi-densepose-mat` | `ruvector` (default) | RuVector graph algorithms |
| `wifi-densepose-mat` | `api` (default) | REST + WebSocket API |
| `wifi-densepose-mat` | `distributed` | Multi-node coordination |
| `wifi-densepose-mat` | `drone` | Drone-mounted scanning |
| `wifi-densepose-hardware` | `esp32` | ESP32 protocol support |
| `wifi-densepose-hardware` | `intel5300` | Intel 5300 CSI Tool |
| `wifi-densepose-hardware` | `linux-wifi` | Linux commodity WiFi |
| `wifi-densepose-wifiscan` | `wlanapi` | Windows WLAN API async scanning |
| `wifi-densepose-core` | `serde` | Serialization support |
| `wifi-densepose-core` | `async` | Async trait support |

## Testing

```bash
# Unit tests (all crates)
cargo test --workspace --no-default-features

# Signal processing benchmarks
cargo bench -p wifi-densepose-signal

# Training benchmarks
cargo bench -p wifi-densepose-train --no-default-features

# Detection benchmarks
cargo bench -p wifi-densepose-mat
```

## Supported Hardware

| Hardware | Crate Feature | CSI Subcarriers | Cost |
|----------|---------------|-----------------|------|
| ESP32-S3 Mesh (3-6 nodes) | `hardware/esp32` | 52-56 | ~$54 |
| Intel 5300 NIC | `hardware/intel5300` | 30 | ~$50 |
| Atheros AR9580 | `hardware/linux-wifi` | 56 | ~$100 |
| Any WiFi (Windows/Linux) | `wifiscan` | RSSI-only | $0 |

## Architecture Decision Records

Key design decisions documented in [`docs/adr/`](https://github.com/ruvnet/wifi-densepose/tree/main/docs/adr):

| ADR | Title | Status |
|-----|-------|--------|
| [ADR-014](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-014-sota-signal-processing.md) | SOTA Signal Processing | Accepted |
| [ADR-015](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-015-public-dataset-training-strategy.md) | MM-Fi + Wi-Pose Training Datasets | Accepted |
| [ADR-016](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-016-ruvector-integration.md) | RuVector Training Pipeline | Accepted (Complete) |
| [ADR-017](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-017-ruvector-signal-mat-integration.md) | RuVector Signal + MAT Integration | Accepted |
| [ADR-021](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-021-vital-sign-detection.md) | Vital Sign Detection Pipeline | Accepted |
| [ADR-022](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-022-windows-wifi-enhanced.md) | Windows WiFi Enhanced Sensing | Accepted |
| [ADR-024](https://github.com/ruvnet/wifi-densepose/blob/main/docs/adr/ADR-024-contrastive-csi-embedding.md) | Contrastive CSI Embedding Model | Accepted |

## Related Projects

- **[WiFi-DensePose](https://github.com/ruvnet/wifi-densepose)** -- Main repository (Python v1 + Rust v2)
- **[RuVector](https://github.com/ruvnet/ruvector)** -- Graph algorithms for neural networks (5 crates, v2.0.4)
- **[rUv](https://github.com/ruvnet)** -- Creator and maintainer

## License

All crates are dual-licensed under [MIT](https://opensource.org/licenses/MIT) OR [Apache-2.0](https://www.apache.org/licenses/LICENSE-2.0).

Copyright (c) 2024 rUv
