# wifi-densepose-signal

[![Crates.io](https://img.shields.io/crates/v/wifi-densepose-signal.svg)](https://crates.io/crates/wifi-densepose-signal)
[![Documentation](https://docs.rs/wifi-densepose-signal/badge.svg)](https://docs.rs/wifi-densepose-signal)
[![License](https://img.shields.io/crates/l/wifi-densepose-signal.svg)](LICENSE)

State-of-the-art WiFi CSI signal processing for human pose estimation.

## Overview

`wifi-densepose-signal` implements six peer-reviewed signal processing algorithms that extract
human motion features from raw WiFi Channel State Information (CSI). Each algorithm is traced
back to its original publication and integrated with the
[ruvector](https://crates.io/crates/ruvector-mincut) family of crates for high-performance
graph and attention operations.

## Algorithms

| Algorithm | Module | Reference |
|-----------|--------|-----------|
| Conjugate Multiplication | `csi_ratio` | SpotFi, SIGCOMM 2015 |
| Hampel Filter | `hampel` | WiGest, 2015 |
| Fresnel Zone Model | `fresnel` | FarSense, MobiCom 2019 |
| CSI Spectrogram | `spectrogram` | Common in WiFi sensing literature since 2018 |
| Subcarrier Selection | `subcarrier_selection` | WiDance, MobiCom 2017 |
| Body Velocity Profile (BVP) | `bvp` | Widar 3.0, MobiSys 2019 |

## Features

- **CSI preprocessing** -- Noise removal, windowing, normalization via `CsiProcessor`.
- **Phase sanitization** -- Unwrapping, outlier removal, and smoothing via `PhaseSanitizer`.
- **Feature extraction** -- Amplitude, phase, correlation, Doppler, and PSD features.
- **Motion detection** -- Human presence detection with confidence scoring via `MotionDetector`.
- **ruvector integration** -- Graph min-cut (person matching), attention mechanisms (antenna and
  spatial attention), and sparse solvers (subcarrier interpolation).

## Quick Start

```rust
use wifi_densepose_signal::{
    CsiProcessor, CsiProcessorConfig,
    PhaseSanitizer, PhaseSanitizerConfig,
    MotionDetector,
};

// Configure and create a CSI processor
let config = CsiProcessorConfig::builder()
    .sampling_rate(1000.0)
    .window_size(256)
    .overlap(0.5)
    .noise_threshold(-30.0)
    .build();

let processor = CsiProcessor::new(config);
```

## Architecture

```text
wifi-densepose-signal/src/
  lib.rs                 -- Re-exports, SignalError, prelude
  bvp.rs                 -- Body Velocity Profile (Widar 3.0)
  csi_processor.rs       -- Core preprocessing pipeline
  csi_ratio.rs           -- Conjugate multiplication (SpotFi)
  features.rs            -- Amplitude/phase/Doppler/PSD feature extraction
  fresnel.rs             -- Fresnel zone diffraction model
  hampel.rs              -- Hampel outlier filter
  motion.rs              -- Motion and human presence detection
  phase_sanitizer.rs     -- Phase unwrapping and sanitization
  spectrogram.rs         -- Time-frequency CSI spectrograms
  subcarrier_selection.rs -- Variance-based subcarrier selection
```

## Related Crates

| Crate | Role |
|-------|------|
| [`wifi-densepose-core`](../wifi-densepose-core) | Foundation types and traits |
| [`ruvector-mincut`](https://crates.io/crates/ruvector-mincut) | Graph min-cut for person matching |
| [`ruvector-attn-mincut`](https://crates.io/crates/ruvector-attn-mincut) | Attention-weighted min-cut |
| [`ruvector-attention`](https://crates.io/crates/ruvector-attention) | Spatial attention for CSI |
| [`ruvector-solver`](https://crates.io/crates/ruvector-solver) | Sparse interpolation solver |

## License

MIT OR Apache-2.0
