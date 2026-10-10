# wifi-densepose-sensing-server

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-sensing-server.svg)](https://crates.io/crates/wifi-densepose-sensing-server)
[![Documentation](https://docs.rs/wifi-densepose-sensing-server/badge.svg)](https://docs.rs/wifi-densepose-sensing-server)
[![License](https://img.shields.io/crates/l/wifi-densepose-sensing-server.svg)](LICENSE)

Lightweight Axum server for real-time WiFi sensing with RuVector signal processing.

## Overview

`wifi-densepose-sensing-server` is the operational backend for WiFi-DensePose. It receives raw CSI
frames from ESP32 hardware over UDP, runs them through the RuVector-powered signal processing
pipeline, and broadcasts processed sensing updates to browser clients via WebSocket. A built-in
static file server hosts the sensing UI on the same port.

The crate ships both a library (`wifi_densepose_sensing_server`) exposing the training and inference
modules, and a binary (`sensing-server`) that starts the full server stack.

Integrates [wifi-densepose-wifiscan](../wifi-densepose-wifiscan) for multi-BSSID WiFi scanning
per ADR-022 Phase 3.

## Features

- **UDP CSI ingestion** -- Receives ESP32 CSI frames on port 5005 and parses them into the internal
  `CsiFrame` representation.
- **Vital sign detection** -- Pure-Rust FFT-based breathing rate (0.1--0.5 Hz) and heart rate
  (0.67--2.0 Hz) estimation from CSI amplitude time series (ADR-021).
- **RVF container** -- Standalone binary container format for packaging model weights, metadata, and
  configuration into a single `.rvf` file with 64-byte aligned segments.
- **RVF pipeline** -- Progressive model loading with streaming segment decoding.
- **Graph Transformer** -- Cross-attention bottleneck between antenna-space CSI features and the
  COCO 17-keypoint body graph, followed by GCN message passing (ADR-023 Phase 2). Pure `std`, no ML
  dependencies.
- **SONA adaptation** -- LoRA + EWC++ online adaptation for environment drift without catastrophic
  forgetting (ADR-023 Phase 5).
- **Contrastive CSI embeddings** -- Self-supervised SimCLR-style pretraining with InfoNCE loss,
  projection head, fingerprint indexing, and cross-modal pose alignment (ADR-024).
- **Sparse inference** -- Activation profiling, sparse matrix-vector multiply, INT8/FP16
  quantization, and a full sparse inference engine for edge deployment (ADR-023 Phase 6).
- **Dataset pipeline** -- Training dataset loading and batching.
- **Multi-BSSID scanning** -- Windows `netsh` integration for BSSID discovery via
  `wifi-densepose-wifiscan` (ADR-022).
- **WebSocket broadcast** -- Real-time sensing updates pushed to all connected clients at
  `ws://localhost:8765/ws/sensing`.
- **Static file serving** -- Hosts the sensing UI on port 8080 with CORS headers.
- **Private startup baseline** -- Restores a recent, installation-bound empty-room field model
  without storing raw CSI or authorizing numeric vital signs (ADR-355).

## Modules

| Module | Description |
|--------|-------------|
| `vital_signs` | Breathing and heart rate extraction via FFT spectral analysis |
| `rvf_container` | RVF binary format builder and reader |
| `rvf_pipeline` | Progressive model loading from RVF containers |
| `graph_transformer` | Graph Transformer + GCN for CSI-to-pose estimation |
| `trainer` | Training loop orchestration |
| `dataset` | Training data loading and batching |
| `sona` | LoRA adapters and EWC++ continual learning |
| `sparse_inference` | Neuron profiling, sparse matmul, INT8/FP16 quantization |
| `embedding` | Contrastive CSI embedding model and fingerprint index |

## Quick Start

```bash
# Build the server
cargo build -p wifi-densepose-sensing-server

# Run with default settings (HTTP :8080, UDP :5005, WS :8765)
cargo run -p wifi-densepose-sensing-server

# Run with custom ports
cargo run -p wifi-densepose-sensing-server -- \
    --http-port 9000 \
    --udp-port 5005 \
    --static-dir ./ui
```

### Empty-room startup baseline

Provide a stable installation ID and an application-owned private state directory:

```bash
cargo run -p wifi-densepose-sensing-server -- \
    --installation-id installation-01 \
    --data-dir /path/to/private/application-state
```

Complete the normal empty-room calibration, then call
`POST /api/v1/calibration/bootstrap/promote`. The server measures a separate 12-sample holdout
before storing an aggregate field-model snapshot. Promotion requires all of the following:

- Exactly 12 fresh samples, spaced at least one second apart, with no stale samples.
- At least 10 of the 12 samples classified as empty.
- Zero samples containing numeric vital-sign estimates.

The 10-of-12 tolerance applies only to the empty classification. A holdout with 12 empty samples
and five samples containing vital-sign estimates still fails; a vital-sign estimate does not
prove occupancy, but it prevents storing the startup baseline under this conservative check.

Completed holdout evaluations return the existing `validation` counters and additive
`validation_diagnostics.requirements` and `validation_diagnostics.failures` fields. A failed
check retains `success: false`, `error_code`, and `error`; the error describes the observed
counts and required limits. Each failure identifies its `check`, `observed` and `required`
counts, and `comparison`, so clients can explain every failed condition. For example, five
vital-sign samples produce a `vital_sign_sample_count` failure with `observed: 5`,
`required: 0`, and `comparison: "at_most"`.

The bounded refinement endpoint, `POST /api/v1/calibration/bootstrap/refine`, also requires
12 fresh samples with zero stale or vital-sign samples. Its diagnostics omit the empty-sample
minimum because refinement does not apply that promotion-only check.

On a later start with the same installation ID,
the snapshot is restored with `bootstrap_only` authority. It can reduce startup background false
positives, but cannot authorize calibrated evidence or numeric heart and breathing rates. Use
`POST /api/v1/calibration/reset` with administrator scope to remove it.

### Using as a library

```rust
use wifi_densepose_sensing_server::vital_signs::VitalSignDetector;

// Create a detector with 20 Hz sample rate
let mut detector = VitalSignDetector::new(20.0);

// Feed CSI amplitude samples
for amplitude in csi_amplitudes.iter() {
    detector.push_sample(*amplitude);
}

// Extract vital signs
if let Some(vitals) = detector.detect() {
    println!("Breathing: {:.1} BPM", vitals.breathing_rate_bpm);
    println!("Heart rate: {:.0} BPM", vitals.heart_rate_bpm);
}
```

## Live multi-node occupancy

For ESP32 CSI and edge-vitals updates, the room's debounced classification gates
the published count. An absent packet from one node does not clear a room that
other fresh nodes still classify as occupied. Conversely, a positive node reading
does not force a count while room presence is still absent or pending debounce.

Only nodes with a sensing timestamp less than ten seconds old contribute to the
count or room vote. A retained stale node can remain available for diagnostics,
but cannot raise current occupancy. Edge-vitals confidence is kept separate from
the raw CSI count score, so alternating packet types do not inflate the count.
The legacy `edge_vitals` message carries its named node's count, subject to room
absence/bootstrap suppression; `sensing_update.estimated_persons` is room-wide.

These are state-consistency rules, not an accuracy claim. Calibrated-zero
precedence over heuristic counts and aging a buffered update during a complete
stream outage remain separate work. The per-node diagnostic endpoint also retains
historical values with freshness labels. Consumers must honor source/freshness
status rather than treating a retained value as current evidence.

For a field report such as [#2058](https://github.com/ruvnet/RuView/issues/2058),
retain the exact server commit and configuration, timestamped `sensing_update`
and `/api/v1/nodes` samples, packet types/rates per node, calibration status and
known room occupancy. A deterministic regression establishes the software defect;
the reporter's logs are still required to attribute their observed fluctuations.
Exclude credentials and unrelated network/person data from shared diagnostics.

Focused checks (from `v2/`):

```bash
cargo test -p wifi-densepose-sensing-server --bin sensing-server --no-default-features person_count_tests
cargo test -p wifi-densepose-sensing-server --no-default-features
```

## Architecture

```text
ESP32 ──UDP:5005──> [ CSI Receiver ]
                          |
                    [ Signal Pipeline ]
                    (vital_signs, graph_transformer, sona)
                          |
                    [ WebSocket Broadcast ]
                          |
Browser <──WS:8765── [ Axum Server :8080 ] ──> Static UI files
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-wifiscan`](../wifi-densepose-wifiscan) | Multi-BSSID WiFi scanning (ADR-022) |
| [`wifi-densepose-core`](../wifi-densepose-core) | Shared types and traits |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | CSI signal processing algorithms |
| [`wifi-densepose-hardware`](../wifi-densepose-hardware) | ESP32 hardware interfaces |
| [`wifi-densepose-wasm`](../wifi-densepose-wasm) | Browser WASM bindings for the sensing UI |
| [`wifi-densepose-train`](../wifi-densepose-train) | Full training pipeline with ruvector |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Disaster detection module |

## License

MIT OR Apache-2.0
