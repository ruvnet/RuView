# wifi-densepose-mat

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-mat.svg)](https://crates.io/crates/wifi-densepose-mat)
[![Documentation](https://docs.rs/wifi-densepose-mat/badge.svg)](https://docs.rs/wifi-densepose-mat)
[![License](https://img.shields.io/crates/l/wifi-densepose-mat.svg)](LICENSE)

Mass Casualty Assessment Tool for WiFi-based disaster survivor detection and localization.

## Overview

`wifi-densepose-mat` uses WiFi Channel State Information (CSI) to detect and locate survivors
trapped in rubble, debris, or collapsed structures. The crate follows Domain-Driven Design (DDD)
with event sourcing, organized into three bounded contexts -- detection, localization, and
alerting -- plus a machine learning layer for debris penetration modeling and vital signs
classification.

Use cases include earthquake search and rescue, building collapse response, avalanche victim
location, flood rescue operations, and mine collapse detection.

## Features

- **Vital signs detection** -- Breathing patterns, heartbeat signatures, and movement
  classification with ensemble classifier combining all three modalities.
- **Survivor localization** -- 3D position estimation through debris via triangulation, depth
  estimation, and position fusion.
- **Triage classification** -- Automatic START protocol-compatible triage with priority-based
  alert generation and dispatch.
- **Event sourcing** -- All state changes emitted as domain events (`DetectionEvent`,
  `AlertEvent`, `ZoneEvent`) stored in a pluggable `EventStore`.
- **ML debris model** -- Debris material classification, signal attenuation prediction, and
  uncertainty-aware vital signs classification.
- **REST + WebSocket API** -- `axum`-based HTTP API for real-time monitoring dashboards.
- **ruvector integration** -- `ruvector-solver` for triangulation math, `ruvector-temporal-tensor`
  for compressed CSI buffering.

### Feature flags

| Flag          | Default | Description                                        |
|---------------|---------|----------------------------------------------------|
| `std`         | yes     | Standard library support                           |
| `api`         | yes     | REST + WebSocket API (enables serde for all types) |
| `ruvector`    | yes     | ruvector-solver and ruvector-temporal-tensor        |
| `serde`       | no      | Serialization (also enabled by `api`)              |
| `portable`    | no      | Low-power mode for field-deployable devices        |
| `distributed` | no      | Multi-node distributed scanning                    |
| `drone`       | no      | Drone-mounted scanning (implies `distributed`)     |

## Quick Start

```rust
use wifi_densepose_mat::{
    DisasterResponse, DisasterConfig, DisasterType,
    ScanZone, ZoneBounds,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = DisasterConfig::builder()
        .disaster_type(DisasterType::Earthquake)
        .sensitivity(0.8)
        .build();

    let mut response = DisasterResponse::new(config);

    // Define scan zone
    let zone = ScanZone::new(
        "Building A - North Wing",
        ZoneBounds::rectangle(0.0, 0.0, 50.0, 30.0),
    );
    response.add_zone(zone)?;

    // Start scanning
    response.start_scanning().await?;

    Ok(())
}
```

## Architecture

```text
wifi-densepose-mat/src/
  lib.rs            -- DisasterResponse coordinator, config builder, MatError
  domain/
    survivor.rs     -- Survivor aggregate root
    disaster_event.rs -- DisasterEvent, DisasterType
    scan_zone.rs    -- ScanZone, ZoneBounds
    alert.rs        -- Alert, Priority
    vital_signs.rs  -- VitalSignsReading, BreathingPattern, HeartbeatSignature
    triage.rs       -- TriageStatus, TriageCalculator (START protocol)
    coordinates.rs  -- Coordinates3D, LocationUncertainty
    events.rs       -- DomainEvent, EventStore, InMemoryEventStore
  detection/        -- BreathingDetector, HeartbeatDetector, MovementClassifier, EnsembleClassifier
  localization/     -- Triangulator, DepthEstimator, PositionFuser
  alerting/         -- AlertGenerator, AlertDispatcher, TriageService
  ml/               -- DebrisPenetrationModel, VitalSignsClassifier, UncertaintyEstimate
  api/              -- axum REST + WebSocket router
  integration/      -- SignalAdapter, NeuralAdapter, HardwareAdapter
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Foundation types and traits |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | CSI preprocessing for detection pipeline |
| [`wifi-densepose-nn`](../wifi-densepose-nn) | Neural inference for ML models |
| [`wifi-densepose-hardware`](../wifi-densepose-hardware) | Hardware sensor data ingestion |
| [`ruvector-solver`](https://crates.io/crates/ruvector-solver) | Triangulation and position math |
| [`ruvector-temporal-tensor`](https://crates.io/crates/ruvector-temporal-tensor) | Compressed CSI buffering |

## License

MIT OR Apache-2.0
