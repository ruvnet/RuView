# ADR-020: Migrate AI/Model Inference to Rust with RuVector and ONNX Runtime

| Field | Value |
|-------|-------|
| **Status** | Accepted |
| **Date** | 2026-02-28 |
| **Deciders** | ruv |
| **Relates to** | ADR-016 (RuVector Integration), ADR-017 (RuVector-Signal-MAT), ADR-019 (Sensing-Only UI) |

## Context

The current Python DensePose backend requires ~2GB+ of dependencies:

| Python Dependency | Size | Purpose |
|-------------------|------|---------|
| PyTorch | ~2.0 GB | Neural network inference |
| torchvision | ~500 MB | Model loading, transforms |
| OpenCV | ~100 MB | Image processing |
| SQLAlchemy + asyncpg | ~20 MB | Database |
| scikit-learn | ~50 MB | Classification |
| **Total** | **~2.7 GB** | |

This makes the DensePose backend impractical for edge deployments, CI pipelines, and developer laptops where users only need WiFi sensing + pose estimation.

Meanwhile, the Rust port at `rust-port/wifi-densepose-rs/` already has:

- **12 workspace crates** covering core, signal, nn, api, db, config, hardware, wasm, cli, mat, train
- **5 RuVector crates** (v2.0.4, published on crates.io) integrated into signal, mat, and train crates
- **3 NN backends**: ONNX Runtime (default), tch (PyTorch C++), Candle (pure Rust)
- **Axum web framework** with WebSocket support in the MAT crate
- **Signal processing pipeline**: CSI processor, BVP, Fresnel geometry, spectrogram, subcarrier selection, motion detection, Hampel filter, phase sanitizer

## Decision

Adopt the Rust workspace as the **primary backend** for AI/model inference and signal processing, replacing the Python FastAPI stack for production deployments.

### Phase 1: ONNX Runtime Default (No libtorch)

Use the `wifi-densepose-nn` crate with `default-features = ["onnx"]` only. This avoids the libtorch C++ dependency entirely.

| Component | Rust Crate | Replaces Python |
|-----------|-----------|-----------------|
| CSI processing | `wifi-densepose-signal::csi_processor` | `v1/src/sensing/feature_extractor.py` |
| Motion detection | `wifi-densepose-signal::motion` | `v1/src/sensing/classifier.py` |
| BVP extraction | `wifi-densepose-signal::bvp` | N/A (new capability) |
| Fresnel geometry | `wifi-densepose-signal::fresnel` | N/A (new capability) |
| Subcarrier selection | `wifi-densepose-signal::subcarrier_selection` | N/A (new capability) |
| Spectrogram | `wifi-densepose-signal::spectrogram` | N/A (new capability) |
| Pose inference | `wifi-densepose-nn::onnx` | PyTorch + torchvision |
| DensePose mapping | `wifi-densepose-nn::densepose` | Python DensePose |
| REST API | `wifi-densepose-mat::api` (Axum) | FastAPI |
| WebSocket stream | `wifi-densepose-mat::api::websocket` | `ws_server.py` |
| Survivor detection | `wifi-densepose-mat::detection` | N/A (new capability) |
| Vital signs | `wifi-densepose-mat::ml` | N/A (new capability) |

### Phase 2: RuVector Signal Intelligence

The 5 RuVector crates provide subpolynomial algorithms already wired into the Rust signal pipeline:

| Crate | Algorithm | Use in Pipeline |
|-------|-----------|-----------------|
| `ruvector-mincut` | Subpolynomial min-cut | Dynamic subcarrier partitioning (sensitive vs insensitive) |
| `ruvector-attn-mincut` | Attention-gated min-cut | Noise-suppressed spectrogram generation |
| `ruvector-attention` | Sensitivity-weighted attention | Body velocity profile extraction |
| `ruvector-solver` | Sparse Fresnel solver | TX-body-RX distance estimation |
| `ruvector-temporal-tensor` | Compressed temporal buffers | Breathing + heartbeat spectrogram storage |

These replace the Python `RssiFeatureExtractor` with hardware-aware, subcarrier-level feature extraction.

### Phase 3: Unified Axum Server

Replace both the Python FastAPI backend (port 8000) and the Python sensing WebSocket (port 8765) with a single Rust Axum server:

```
ESP32 (UDP :5005) ──▶ Rust Axum server (:8000) ──▶ UI (browser)
                          ├── /health/*          (health checks)
                          ├── /api/v1/pose/*     (pose estimation)
                          ├── /api/v1/stream/*   (WebSocket pose stream)
                          ├── /ws/sensing        (sensing WebSocket — replaces :8765)
                          └── /ws/mat/stream     (MAT domain events)
```

### Build Configuration

```toml
# Lightweight build — no libtorch, no OpenBLAS
cargo build --release -p wifi-densepose-mat --no-default-features --features "std,api,onnx"

# Full build with all backends
cargo build --release --features "all-backends"
```

### Dependency Comparison

| | Python Backend | Rust Backend (ONNX only) |
|---|---|---|
| Install size | ~2.7 GB | ~50 MB binary |
| Runtime memory | ~500 MB | ~20 MB |
| Startup time | 3-5s | <100ms |
| Dependencies | 30+ pip packages | Single static binary |
| GPU support | CUDA via PyTorch | CUDA via ONNX Runtime |
| Model format | .pt/.pth (PyTorch) | .onnx (portable) |
| Cross-compile | Difficult | `cargo build --target` |
| WASM target | No | Yes (`wifi-densepose-wasm`) |

### Model Conversion

Export existing PyTorch models to ONNX for the Rust backend:

```python
# One-time conversion (Python)
import torch
model = torch.load("model.pth")
torch.onnx.export(model, dummy_input, "model.onnx", opset_version=17)
```

The `wifi-densepose-nn::onnx` module loads `.onnx` files directly.

## Consequences

### Positive
- Single ~50MB static binary replaces ~2.7GB Python environment
- ~20MB runtime memory vs ~500MB
- Sub-100ms startup vs 3-5 seconds
- Single port serves all endpoints (API, WebSocket sensing, WebSocket pose)
- RuVector subpolynomial algorithms run natively (no FFI overhead)
- WASM build target enables browser-side inference
- Cross-compilation for ARM (Raspberry Pi), ESP32-S3, etc.

### Negative
- ONNX model conversion required (one-time step per model)
- Developers need Rust toolchain for backend changes
- Python sensing pipeline (`ws_server.py`) remains useful for rapid prototyping
- `ndarray-linalg` requires OpenBLAS or system LAPACK for some signal crates

### Migration Path
1. Keep Python `ws_server.py` as fallback for development/prototyping
2. Build Rust binary with `cargo build --release -p wifi-densepose-mat`
3. UI detects which backend is running and adapts (existing `sensingOnlyMode` logic)
4. Deprecate Python backend once Rust API reaches feature parity

## Verification

```bash
# Build the Rust workspace (ONNX-only, no libtorch)
cd rust-port/wifi-densepose-rs
cargo check --workspace 2>&1

# Build release binary
cargo build --release -p wifi-densepose-mat --no-default-features --features "std,api"

# Run tests
cargo test --workspace

# Binary size
ls -lh target/release/wifi-densepose-mat
```
