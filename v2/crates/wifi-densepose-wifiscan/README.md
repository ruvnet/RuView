# wifi-densepose-wifiscan

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-wifiscan.svg)](https://crates.io/crates/wifi-densepose-wifiscan)
[![Documentation](https://docs.rs/wifi-densepose-wifiscan/badge.svg)](https://docs.rs/wifi-densepose-wifiscan)
[![License](https://img.shields.io/crates/l/wifi-densepose-wifiscan.svg)](LICENSE)

Multi-BSSID WiFi scanning for Windows-enhanced DensePose sensing (ADR-022).

## Overview

`wifi-densepose-wifiscan` implements the BSSID Acquisition bounded context for the WiFi-DensePose
system. It discovers and tracks nearby WiFi access points, parses platform-specific scan output,
and feeds multi-AP signal data into a sensing pipeline that performs motion detection, breathing
estimation, attention weighting, and fingerprint matching.

The crate uses `#[forbid(unsafe_code)]` and is designed as a pure-Rust domain layer with
pluggable platform adapters.

## Features

- **BSSID registry** -- Tracks observed access points with running RSSI statistics, band/radio
  type classification, and metadata. Types: `BssidId`, `BssidObservation`, `BssidRegistry`,
  `BssidEntry`.
- **Netsh adapter** (Tier 1) -- Parses `netsh wlan show networks mode=bssid` output into
  structured `BssidObservation` records. Zero platform dependencies.
- **WLAN API scanner** (Tier 2, `wlanapi` feature) -- Async scanning via the Windows WLAN API
  with `tokio` integration.
- **Multi-AP frame** -- `MultiApFrame` aggregates observations from multiple BSSIDs into a single
  timestamped frame for downstream processing.
- **Sensing pipeline** (`pipeline` feature) -- `WindowsWifiPipeline` orchestrates motion
  detection, breathing estimation, attention-weighted AP selection, and location fingerprint
  matching.

### Feature flags

| Flag       | Default | Description                                          |
|------------|---------|------------------------------------------------------|
| `serde`    | yes     | Serialization for domain types                       |
| `pipeline` | yes     | WindowsWifiPipeline sensing orchestration            |
| `wlanapi`  | no      | Tier 2 async scanning via tokio (Windows WLAN API)   |

## Quick Start

```rust
use wifi_densepose_wifiscan::{
    NetshBssidScanner, BssidRegistry, WlanScanPort,
};

// Parse netsh output (works on any platform for testing)
let netsh_output = "..."; // output of `netsh wlan show networks mode=bssid`
let observations = wifi_densepose_wifiscan::parse_netsh_output(netsh_output);

// Register observations
let mut registry = BssidRegistry::new();
for obs in &observations {
    registry.update(obs);
}

println!("Tracking {} access points", registry.len());
```

With the `pipeline` feature enabled:

```rust
use wifi_densepose_wifiscan::WindowsWifiPipeline;

let pipeline = WindowsWifiPipeline::new();
// Feed MultiApFrame data into the pipeline for sensing...
```

## Architecture

```text
wifi-densepose-wifiscan/src/
  lib.rs          -- Re-exports, feature gates
  domain/
    bssid.rs      -- BssidId, BssidObservation, BandType, RadioType
    registry.rs   -- BssidRegistry, BssidEntry, BssidMeta, RunningStats
    frame.rs      -- MultiApFrame (multi-BSSID aggregated frame)
    result.rs     -- EnhancedSensingResult
  port.rs         -- WlanScanPort trait (platform abstraction)
  adapter.rs      -- NetshBssidScanner (Tier 1), WlanApiScanner (Tier 2)
  pipeline.rs     -- WindowsWifiPipeline (motion, breathing, attention, fingerprint)
  error.rs        -- WifiScanError
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-signal`](../wifi-densepose-signal) | Advanced CSI signal processing |
| [`wifi-densepose-vitals`](../wifi-densepose-vitals) | Vital sign extraction from CSI |
| [`wifi-densepose-hardware`](../wifi-densepose-hardware) | ESP32 and other hardware interfaces |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Disaster detection using multi-AP data |

## License

MIT OR Apache-2.0
