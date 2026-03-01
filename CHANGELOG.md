# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Cross-platform RSSI adapters** — macOS CoreWLAN (`MacosCoreWlanScanner`) and Linux `iw` (`LinuxIwScanner`) Rust adapters with `#[cfg(target_os)]` gating
- macOS CoreWLAN Python sensing adapter with Swift helper (`mac_wifi.swift`)
- macOS synthetic BSSID generation (FNV-1a hash) for Sonoma 14.4+ BSSID redaction
- Linux `iw dev <iface> scan` parser with freq-to-channel conversion and `scan dump` (no-root) mode
- ADR-025: macOS CoreWLAN WiFi Sensing (ORCA)

### Fixed
- Removed synthetic byte counters from Python `MacosWifiCollector` — now reports `tx_bytes=0, rx_bytes=0` instead of fake incrementing values

---

## [3.0.0] - 2026-03-01

Major release: AETHER contrastive embedding model, Docker Hub images, and comprehensive UI overhaul.

### Added — AETHER Contrastive Embedding Model (ADR-024)
- **Project AETHER** — self-supervised contrastive learning for WiFi CSI fingerprinting, similarity search, and anomaly detection (`9bbe956`)
- `embedding.rs` module: `ProjectionHead`, `InfoNceLoss`, `CsiAugmenter`, `FingerprintIndex`, `PoseEncoder`, `EmbeddingExtractor` (909 lines, zero external ML dependencies)
- SimCLR-style pretraining with 5 physically-motivated augmentations (temporal jitter, subcarrier masking, Gaussian noise, phase rotation, amplitude scaling)
- CLI flags: `--pretrain`, `--pretrain-epochs`, `--embed`, `--build-index <type>`
- Four HNSW-compatible fingerprint index types: `env_fingerprint`, `activity_pattern`, `temporal_baseline`, `person_track`
- Cross-modal `PoseEncoder` for WiFi-to-camera embedding alignment
- VICReg regularization for embedding collapse prevention
- 53K total parameters (55 KB at INT8) — fits on ESP32

### Added — Docker & Deployment
- Published Docker Hub images: `ruvnet/wifi-densepose:latest` (132 MB Rust) and `ruvnet/wifi-densepose:python` (569 MB) (`add9f19`)
- Multi-stage Dockerfile for Rust sensing server with RuVector crates
- `docker-compose.yml` orchestrating both Rust and Python services
- RVF model export via `--export-rvf` and load via `--load-rvf` CLI flags

### Added — Documentation
- 33 use cases across 4 vertical tiers: Everyday, Specialized, Robotics & Industrial, Extreme (`0afd9c5`)
- "Why WiFi Wins" comparison table (WiFi vs camera vs LIDAR vs wearable vs PIR)
- Mermaid architecture diagrams: end-to-end pipeline, signal processing detail, deployment topology (`50f0fc9`)
- Models & Training section with RuVector crate links (GitHub + crates.io), SONA component table (`965a1cc`)
- RVF container section with deployment targets table (ESP32 0.7 MB to server 50+ MB)
- Collapsible README sections for improved navigation (`478d964`, `99ec980`, `0ebd6be`)
- Installation and Quick Start moved above Table of Contents (`50acbf7`)
- CSI hardware requirement notice (`528b394`)

### Fixed
- **UI auto-detects server port from page origin** — no more hardcoded `localhost:8080`; works on any port (Docker :3000, native :8080, custom) (`3b72f35`, closes #55)
- **Docker port mismatch** — server now binds 3000/3001 inside container as documented (`44b9c30`)
- Added `/ws/sensing` WebSocket route to the HTTP server so UI only needs one port
- Fixed README API endpoint references: `/api/v1/health` → `/health`, `/api/v1/sensing` → `/api/v1/sensing/latest`
- Multi-person tracking limit corrected: configurable default 10, no hard software cap (`e2ce250`)

---

## [2.0.0] - 2026-02-28

Major release: complete Rust sensing server, full DensePose training pipeline, RuVector v2.0.4 integration, ESP32-S3 firmware, and 6 security hardening patches.

### Added — Rust Sensing Server
- **Full DensePose-compatible REST API** served by Axum (`d956c30`)
  - `GET /health` — server health
  - `GET /api/v1/sensing/latest` — live CSI sensing data
  - `GET /api/v1/vital-signs` — breathing rate (6-30 BPM) and heartbeat (40-120 BPM)
  - `GET /api/v1/pose/current` — 17 COCO keypoints derived from WiFi signal field
  - `GET /api/v1/info` — server build and feature info
  - `GET /api/v1/model/info` — RVF model container metadata
  - `ws://host/ws/sensing` — real-time WebSocket stream
- Three data sources: `--source esp32` (UDP CSI), `--source windows` (netsh RSSI), `--source simulated` (deterministic reference)
- Auto-detection: server probes ESP32 UDP and Windows WiFi, falls back to simulated
- Three.js visualization UI with 3D body skeleton, signal heatmap, phase plot, Doppler bars, vital signs panel
- Static UI serving via `--ui-path` flag
- Throughput: 9,520–11,665 frames/sec (release build)

### Added — ADR-021: Vital Sign Detection
- `VitalSignDetector` with breathing (6-30 BPM) and heartbeat (40-120 BPM) extraction from CSI fluctuations (`1192de9`)
- FFT-based spectral analysis with configurable band-pass filters
- Confidence scoring based on spectral peak prominence
- REST endpoint `/api/v1/vital-signs` with real-time JSON output

### Added — ADR-023: DensePose Training Pipeline (Phases 1-8)
- `wifi-densepose-train` crate with complete 8-phase pipeline (`fc409df`, `ec98e40`, `fce1271`)
  - Phase 1: `DataPipeline` with MM-Fi and Wi-Pose dataset loaders
  - Phase 2: `CsiToPoseTransformer` — 4-head cross-attention + 2-layer GCN on COCO skeleton
  - Phase 3: 6-term composite loss (MSE, bone length, symmetry, joint angle, temporal, confidence)
  - Phase 4: `DynamicPersonMatcher` via ruvector-mincut (O(n^1.5 log n) Hungarian assignment)
  - Phase 5: `SonaAdapter` — MicroLoRA rank-4 with EWC++ memory preservation
  - Phase 6: `SparseInference` — progressive 3-layer model loading (A: essential, B: refinement, C: full)
  - Phase 7: `RvfContainer` — single-file model packaging with segment-based binary format
  - Phase 8: End-to-end training with cosine-annealing LR, early stopping, checkpoint saving
- CLI: `--train`, `--dataset`, `--epochs`, `--save-rvf`, `--load-rvf`, `--export-rvf`
- Benchmark: ~11,665 fps inference, 229 tests passing

### Added — ADR-016: RuVector Training Integration (all 5 crates)
- `ruvector-mincut` → `DynamicPersonMatcher` in `metrics.rs` + subcarrier selection (`81ad09d`, `a7dd31c`)
- `ruvector-attn-mincut` → antenna attention in `model.rs` + noise-gated spectrogram
- `ruvector-temporal-tensor` → `CompressedCsiBuffer` in `dataset.rs` + compressed breathing/heartbeat
- `ruvector-solver` → sparse subcarrier interpolation (114→56) + Fresnel triangulation
- `ruvector-attention` → spatial attention in `model.rs` + attention-weighted BVP
- Vendored all 11 RuVector crates under `vendor/ruvector/` (`d803bfe`)

### Added — ADR-017: RuVector Signal & MAT Integration (7 integration points)
- `gate_spectrogram()` — attention-gated noise suppression (`18170d7`)
- `attention_weighted_bvp()` — sensitivity-weighted velocity profiles
- `mincut_subcarrier_partition()` — dynamic sensitive/insensitive subcarrier split
- `solve_fresnel_geometry()` — TX-body-RX distance estimation
- `CompressedBreathingBuffer` + `CompressedHeartbeatSpectrogram`
- `BreathingDetector` + `HeartbeatDetector` (MAT crate, real FFT + micro-Doppler)
- Feature-gated behind `cfg(feature = "ruvector")` (`ab2453e`)

### Added — ADR-018: ESP32-S3 Firmware & Live CSI Pipeline
- ESP32-S3 firmware with FreeRTOS CSI extraction (`92a5182`)
- ADR-018 binary frame format: `[0xAD, 0x18, len_hi, len_lo, payload]`
- Rust `Esp32Aggregator` receiving UDP frames on port 5005
- `bridge.rs` converting I/Q pairs to amplitude/phase vectors
- NVS provisioning for WiFi credentials
- Pre-built binary quick start documentation (`696a726`)

### Added — ADR-014: SOTA Signal Processing
- 6 algorithms, 83 tests (`fcb93cc`)
  - Hampel filter (median + MAD, resistant to 50% contamination)
  - Conjugate multiplication (reference-antenna ratio, cancels common-mode noise)
  - Phase sanitization (unwrap + linear detrend, removes CFO/SFO)
  - Fresnel zone geometry (TX-body-RX distance from first-principles physics)
  - Body Velocity Profile (micro-Doppler extraction, 5.7x speedup)
  - Attention-gated spectrogram (learned noise suppression)

### Added — ADR-015: Public Dataset Training Strategy
- MM-Fi and Wi-Pose dataset specifications with download links (`4babb32`, `5dc2f66`)
- Verified dataset dimensions, sampling rates, and annotation formats
- Cross-dataset evaluation protocol

### Added — WiFi-Mat Disaster Detection Module
- Multi-AP triangulation for through-wall survivor detection (`a17b630`, `6b20ff0`)
- Triage classification (breathing, heartbeat, motion)
- Domain events: `survivor_detected`, `survivor_updated`, `alert_created`
- WebSocket broadcast at `/ws/mat/stream`

### Added — Infrastructure
- Guided 7-step interactive installer with 8 hardware profiles (`8583f3e`)
- Comprehensive build guide for Linux, macOS, Windows, Docker, ESP32 (`45f8a0d`)
- 12 Architecture Decision Records (ADR-001 through ADR-012) (`337dd96`)

### Added — UI & Visualization
- Sensing-only UI mode with Gaussian splat visualization (`b7e0f07`)
- Three.js 3D body model (17 joints, 16 limbs) with signal-viz components
- Tabs: Dashboard, Hardware, Live Demo, Sensing, Architecture, Performance, Applications
- WebSocket client with automatic reconnection and exponential backoff

### Added — Rust Signal Processing Crate
- Complete Rust port of WiFi-DensePose with modular workspace (`6ed69a3`)
  - `wifi-densepose-signal` — CSI processing, phase sanitization, feature extraction
  - `wifi-densepose-core` — shared types and configuration
  - `wifi-densepose-nn` — neural network inference (DensePose head, RCNN)
  - `wifi-densepose-hardware` — ESP32 aggregator, hardware interfaces
  - `wifi-densepose-config` — configuration management
- Comprehensive benchmarks and validation tests (`3ccb301`)

### Added — Python Sensing Pipeline
- `WindowsWifiCollector` — RSSI collection via `netsh wlan show networks`
- `RssiFeatureExtractor` — variance, spectral bands (motion 0.5-4 Hz, breathing 0.1-0.5 Hz), change points
- `PresenceClassifier` — rule-based 3-state classification (ABSENT / PRESENT_STILL / ACTIVE)
- Cross-receiver agreement scoring for multi-AP confidence boosting
- WebSocket sensing server (`ws_server.py`) broadcasting JSON at 2 Hz
- Deterministic CSI proof bundles for reproducible verification (`v1/data/proof/`)
- Commodity sensing unit tests (`b391638`)

### Changed
- Rust hardware adapters now return explicit errors instead of silent empty data (`6e0e539`)

### Fixed
- Review fixes for end-to-end training pipeline (`45f0304`)
- Dockerfile paths updated from `src/` to `v1/src/` (`7872987`)
- IoT profile installer instructions updated for aggregator CLI (`f460097`)
- `process.env` reference removed from browser ES module (`e320bc9`)

### Performance
- 5.7x Doppler extraction speedup via optimized FFT windowing (`32c75c8`)
- Single 2.1 MB static binary, zero Python dependencies for Rust server

### Security
- Fix SQL injection in status command and migrations (`f9d125d`)
- Fix XSS vulnerabilities in UI components (`5db55fd`)
- Fix command injection in statusline.cjs (`4cb01fd`)
- Fix path traversal vulnerabilities (`896c4fc`)
- Fix insecure WebSocket connections — enforce wss:// on non-localhost (`ac094d4`)
- Fix GitHub Actions shell injection (`ab2e7b4`)
- Fix 10 additional vulnerabilities, remove 12 dead code instances (`7afdad0`)

---

## [1.1.0] - 2025-06-07

### Added
- Complete Python WiFi-DensePose system with CSI data extraction and router interface
- CSI processing and phase sanitization modules
- Batch processing for CSI data in `CSIProcessor` and `PhaseSanitizer`
- Hardware, pose, and stream services for WiFi-DensePose API
- Comprehensive CSS styles for UI components and dark mode support
- API and Deployment documentation

### Fixed
- Badge links for PyPI and Docker in README
- Async engine creation poolclass specification

---

## [1.0.0] - 2024-12-01

### Added
- Initial release of WiFi-DensePose
- Real-time WiFi-based human pose estimation using Channel State Information (CSI)
- DensePose neural network integration for body surface mapping
- RESTful API with comprehensive endpoint coverage
- WebSocket streaming for real-time pose data
- Multi-person tracking with configurable capacity (default 10, up to 50+)
- Fall detection and activity recognition
- Domain configurations: healthcare, fitness, smart home, security
- CLI interface for server management and configuration
- Hardware abstraction layer for multiple WiFi chipsets
- Phase sanitization and signal processing pipeline
- Authentication and rate limiting
- Background task management
- Cross-platform support (Linux, macOS, Windows)

### Documentation
- User guide and API reference
- Deployment and troubleshooting guides
- Hardware setup and calibration instructions
- Performance benchmarks
- Contributing guidelines

[Unreleased]: https://github.com/ruvnet/wifi-densepose/compare/v3.0.0...HEAD
[3.0.0]: https://github.com/ruvnet/wifi-densepose/compare/v2.0.0...v3.0.0
[2.0.0]: https://github.com/ruvnet/wifi-densepose/compare/v1.1.0...v2.0.0
[1.1.0]: https://github.com/ruvnet/wifi-densepose/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/ruvnet/wifi-densepose/releases/tag/v1.0.0
