# wifi-densepose-vitals

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-vitals.svg)](https://crates.io/crates/wifi-densepose-vitals)
[![Documentation](https://docs.rs/wifi-densepose-vitals/badge.svg)](https://docs.rs/wifi-densepose-vitals)
[![License](https://img.shields.io/crates/l/wifi-densepose-vitals.svg)](LICENSE)

ESP32 CSI-grade vital sign extraction: heart rate and respiratory rate from WiFi Channel State
Information (ADR-021).

## Overview

`wifi-densepose-vitals` implements a four-stage pipeline that extracts respiratory rate and heart
rate from multi-subcarrier CSI amplitude and phase data. The crate has zero external dependencies
beyond `tracing` (and optional `serde`), uses `#[forbid(unsafe_code)]`, and is designed for
resource-constrained edge deployments alongside ESP32 hardware.

## Pipeline Stages

1. **Preprocessing** (`CsiVitalPreprocessor`) -- EMA-based static component suppression,
   producing per-subcarrier residuals that isolate body-induced signal variation.
2. **Breathing extraction** (`BreathingExtractor`) -- Bandpass filtering at 0.1--0.5 Hz with
   zero-crossing analysis for respiratory rate estimation.
3. **Heart rate extraction** (`HeartRateExtractor`) -- Bandpass filtering at 0.8--2.0 Hz with
   autocorrelation peak detection and inter-subcarrier phase coherence weighting.
4. **Anomaly detection** (`VitalAnomalyDetector`) -- Z-score analysis using Welford running
   statistics for real-time clinical alerts (apnea, tachycardia, bradycardia).

Results are stored in a `VitalSignStore` with configurable retention for historical trend
analysis.

### Feature flags

| Flag    | Default | Description                              |
|---------|---------|------------------------------------------|
| `serde` | yes     | Serialization for vital sign types       |

## Quick Start

```rust
use wifi_densepose_vitals::{
    CsiVitalPreprocessor, BreathingExtractor, HeartRateExtractor,
    VitalAnomalyDetector, VitalSignStore, CsiFrame,
    VitalReading, VitalEstimate, VitalStatus,
};

let mut preprocessor = CsiVitalPreprocessor::new(56, 0.05);
let mut breathing = BreathingExtractor::new(56, 100.0, 30.0);
let mut heartrate = HeartRateExtractor::new(56, 100.0, 15.0);
let mut anomaly = VitalAnomalyDetector::default_config();
let mut store = VitalSignStore::new(3600);

// Process a CSI frame
let frame = CsiFrame {
    amplitudes: vec![1.0; 56],
    phases: vec![0.0; 56],
    n_subcarriers: 56,
    sample_index: 0,
    sample_rate_hz: 100.0,
};

if let Some(residuals) = preprocessor.process(&frame) {
    let weights = vec![1.0 / 56.0; 56];
    let rr = breathing.extract(&residuals, &weights);
    let hr = heartrate.extract(&residuals, &frame.phases);

    let reading = VitalReading {
        respiratory_rate: rr.unwrap_or_else(VitalEstimate::unavailable),
        heart_rate: hr.unwrap_or_else(VitalEstimate::unavailable),
        subcarrier_count: frame.n_subcarriers,
        signal_quality: 0.9,
        timestamp_secs: 0.0,
    };

    let alerts = anomaly.check(&reading);
    store.push(reading);
}
```

## Architecture

```text
wifi-densepose-vitals/src/
  lib.rs            -- Re-exports, module declarations
  types.rs          -- CsiFrame, VitalReading, VitalEstimate, VitalStatus
  preprocessor.rs   -- CsiVitalPreprocessor (EMA static suppression)
  breathing.rs      -- BreathingExtractor (0.1-0.5 Hz bandpass)
  heartrate.rs      -- HeartRateExtractor (0.8-2.0 Hz autocorrelation)
  anomaly.rs        -- VitalAnomalyDetector (Z-score, Welford stats)
  store.rs          -- VitalSignStore, VitalStats (historical retention)
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-hardware`](../wifi-densepose-hardware) | Provides raw CSI frames from ESP32 |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Uses vital signs for survivor triage |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | Advanced signal processing algorithms |

## License

MIT OR Apache-2.0
