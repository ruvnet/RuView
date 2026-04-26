# wifi-densepose-hardware

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-hardware.svg)](https://crates.io/crates/wifi-densepose-hardware)
[![Documentation](https://docs.rs/wifi-densepose-hardware/badge.svg)](https://docs.rs/wifi-densepose-hardware)
[![License](https://img.shields.io/crates/l/wifi-densepose-hardware.svg)](LICENSE)

Hardware interface abstractions for WiFi CSI sensors (ESP32, Intel 5300, Atheros).

## Overview

`wifi-densepose-hardware` provides platform-agnostic parsers for WiFi CSI data from multiple
hardware sources. All parsing operates on byte buffers with no C FFI or hardware dependencies at
compile time, making the crate fully portable and deterministic -- the same bytes in always produce
the same parsed output.

## Features

- **ESP32 binary parser** -- Parses ADR-018 binary CSI frames streamed over UDP from ESP32 and
  ESP32-S3 devices.
- **UDP aggregator** -- Receives and aggregates CSI frames from multiple ESP32 nodes (ADR-018
  Layer 2). Provided as a standalone binary.
- **Bridge** -- Converts hardware `CsiFrame` into the `CsiData` format expected by the detection
  pipeline (ADR-018 Layer 3).
- **No mock data** -- Parsers either parse real bytes or return explicit `ParseError` values.
  There are no synthetic fallbacks.
- **Pure byte-buffer parsing** -- No FFI to ESP-IDF or kernel modules. Safe to compile and test
  on any platform.

### Feature flags

| Flag        | Default | Description                                |
|-------------|---------|--------------------------------------------|
| `std`       | yes     | Standard library support                   |
| `esp32`     | no      | ESP32 serial CSI frame parsing             |
| `intel5300` | no      | Intel 5300 CSI Tool log parsing            |
| `linux-wifi`| no      | Linux WiFi interface for commodity sensing |

## Quick Start

```rust
use wifi_densepose_hardware::{CsiFrame, Esp32CsiParser, ParseError};

// Parse ESP32 CSI data from raw UDP bytes
let raw_bytes: &[u8] = &[/* ADR-018 binary frame */];
match Esp32CsiParser::parse_frame(raw_bytes) {
    Ok((frame, consumed)) => {
        println!("Parsed {} subcarriers ({} bytes)",
                 frame.subcarrier_count(), consumed);
        let (amplitudes, phases) = frame.to_amplitude_phase();
        // Feed into detection pipeline...
    }
    Err(ParseError::InsufficientData { needed, got }) => {
        eprintln!("Need {} bytes, got {}", needed, got);
    }
    Err(e) => eprintln!("Parse error: {}", e),
}
```

## Architecture

```text
wifi-densepose-hardware/src/
  lib.rs            -- Re-exports: CsiFrame, Esp32CsiParser, ParseError, CsiData
  csi_frame.rs      -- CsiFrame, CsiMetadata, SubcarrierData, Bandwidth, AntennaConfig
  esp32_parser.rs   -- Esp32CsiParser (ADR-018 binary protocol)
  error.rs          -- ParseError
  bridge.rs         -- CsiData bridge to detection pipeline
  aggregator/       -- UDP multi-node frame aggregator (binary)
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Foundation types (`CsiFrame` definitions) |
| [`wifi-densepose-signal`](../wifi-densepose-signal) | Consumes parsed CSI data for processing |
| [`wifi-densepose-mat`](../wifi-densepose-mat) | Uses hardware adapters for disaster detection |
| [`wifi-densepose-vitals`](../wifi-densepose-vitals) | Vital sign extraction from parsed frames |

## License

MIT OR Apache-2.0
