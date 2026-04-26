# wifi-densepose-core

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-core.svg)](https://crates.io/crates/wifi-densepose-core)
[![Documentation](https://docs.rs/wifi-densepose-core/badge.svg)](https://docs.rs/wifi-densepose-core)
[![License](https://img.shields.io/crates/l/wifi-densepose-core.svg)](LICENSE)

Core types, traits, and utilities for the WiFi-DensePose pose estimation system.

## Overview

`wifi-densepose-core` is the foundation crate for the WiFi-DensePose workspace. It defines the
shared data structures, error types, and trait contracts used by every other crate in the
ecosystem. The crate is `no_std`-compatible (with the `std` feature disabled) and forbids all
unsafe code.

## Features

- **Core data types** -- `CsiFrame`, `ProcessedSignal`, `PoseEstimate`, `PersonPose`, `Keypoint`,
  `KeypointType`, `BoundingBox`, `Confidence`, `Timestamp`, and more.
- **Trait abstractions** -- `SignalProcessor`, `NeuralInference`, and `DataStore` define the
  contracts for signal processing, neural network inference, and data persistence respectively.
- **Error hierarchy** -- `CoreError`, `SignalError`, `InferenceError`, and `StorageError` provide
  typed error handling across subsystem boundaries.
- **`no_std` support** -- Disable the default `std` feature for embedded or WASM targets.
- **Constants** -- `MAX_KEYPOINTS` (17, COCO format), `MAX_SUBCARRIERS` (256),
  `DEFAULT_CONFIDENCE_THRESHOLD` (0.5).

### Feature flags

| Flag    | Default | Description                                |
|---------|---------|--------------------------------------------|
| `std`   | yes     | Enable standard library support            |
| `serde` | no      | Serialization via serde (+ ndarray serde)  |
| `async` | no      | Async trait definitions via `async-trait`   |

## Quick Start

```rust
use wifi_densepose_core::{CsiFrame, Keypoint, KeypointType, Confidence};

// Create a keypoint with high confidence
let keypoint = Keypoint::new(
    KeypointType::Nose,
    0.5,
    0.3,
    Confidence::new(0.95).unwrap(),
);

assert!(keypoint.is_visible());
```

Or use the prelude for convenient bulk imports:

```rust
use wifi_densepose_core::prelude::*;
```

## Architecture

```text
wifi-densepose-core/src/
  lib.rs          -- Re-exports, constants, prelude
  types.rs        -- CsiFrame, PoseEstimate, Keypoint, etc.
  traits.rs       -- SignalProcessor, NeuralInference, DataStore
  error.rs        -- CoreError, SignalError, InferenceError, StorageError
  utils.rs        -- Shared helper functions
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-signal`](../wifi-densepose-signal) | CSI signal processing algorithms |
| [`wifi-densepose-nn`](../wifi-densepose-nn) | Neural network inference backends |
| [`wifi-densepose-train`](../wifi-densepose-train) | Training pipeline with ruvector |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Disaster detection (MAT) |
| [`wifi-densepose-hardware`](../wifi-densepose-hardware) | Hardware sensor interfaces |
| [`wifi-densepose-vitals`](../wifi-densepose-vitals) | Vital sign extraction |
| [`wifi-densepose-wifiscan`](../wifi-densepose-wifiscan) | Multi-BSSID WiFi scanning |

## License

MIT OR Apache-2.0
