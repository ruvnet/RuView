# ruv-neural-signal

Signal processing: filtering, spectral analysis, connectivity metrics, and artifact
rejection for neural time series data.

## Overview

`ruv-neural-signal` provides a complete digital signal processing pipeline for
multi-channel neural magnetic field and electrophysiology data. It covers IIR
filtering in second-order sections form, FFT-based spectral analysis, Hilbert
transform for instantaneous phase extraction, artifact detection and rejection,
cross-channel connectivity metrics, and a configurable multi-stage preprocessing
pipeline.

## Features

- **IIR Filters** (`filter`): Butterworth bandpass, highpass, lowpass, and notch
  filters in SOS (second-order sections) form for numerical stability
- **Spectral analysis** (`spectral`): Welch PSD estimation, STFT, band power
  extraction, spectral entropy, and peak frequency detection
- **Hilbert transform** (`hilbert`): FFT-based analytic signal for instantaneous
  phase and amplitude envelope computation
- **Artifact detection** (`artifact`): Eye blink, muscle artifact, and cardiac
  artifact detection with configurable rejection
- **Connectivity metrics** (`connectivity`): Phase locking value (PLV), coherence,
  imaginary coherence, amplitude envelope correlation (AEC), and all-pairs
  computation for connectivity matrix construction
- **Preprocessing pipeline** (`preprocessing`): Configurable multi-stage pipeline
  chaining filters, artifact rejection, and re-referencing

## Usage

```rust
use ruv_neural_signal::{
    BandpassFilter, PreprocessingPipeline, SignalProcessor,
    compute_psd, band_power, hilbert_transform, instantaneous_phase,
    compute_all_pairs, ConnectivityMetric,
};
use ruv_neural_core::FrequencyBand;

// Apply a bandpass filter (8-13 Hz alpha band)
let filter = BandpassFilter::new(8.0, 13.0, 1000.0, 4).unwrap();
let filtered = filter.apply(&raw_signal);

// Compute power spectral density (Welch method)
let psd = compute_psd(&signal, 1000.0, 256, 128);
let alpha_power = band_power(&psd, 1000.0, 8.0, 13.0);

// Extract instantaneous phase via Hilbert transform
let analytic = hilbert_transform(&signal);
let phases = instantaneous_phase(&analytic);

// Compute all-pairs connectivity matrix
let connectivity_matrix = compute_all_pairs(
    &multi_channel_data,
    ConnectivityMetric::PhaseLockingValue,
);

// Run full preprocessing pipeline
let pipeline = PreprocessingPipeline::default();
let clean_data = pipeline.process(&raw_data).unwrap();
```

## API Reference

| Module          | Key Types / Functions                                           |
|-----------------|-----------------------------------------------------------------|
| `filter`        | `BandpassFilter`, `HighpassFilter`, `LowpassFilter`, `NotchFilter`, `SignalProcessor` |
| `spectral`      | `compute_psd`, `compute_stft`, `band_power`, `spectral_entropy`, `peak_frequency` |
| `hilbert`       | `hilbert_transform`, `instantaneous_phase`, `instantaneous_amplitude` |
| `artifact`      | `detect_eye_blinks`, `detect_muscle_artifact`, `detect_cardiac`, `reject_artifacts` |
| `connectivity`  | `phase_locking_value`, `coherence`, `imaginary_coherence`, `amplitude_envelope_correlation`, `compute_all_pairs` |
| `preprocessing` | `PreprocessingPipeline`                                         |

## Feature Flags

| Feature | Default | Description                      |
|---------|---------|----------------------------------|
| `std`   | Yes     | Standard library support         |
| `simd`  | No      | SIMD-accelerated filter kernels  |

## Integration

Depends on `ruv-neural-core` for `MultiChannelTimeSeries` and `FrequencyBand` types.
Feeds processed data into `ruv-neural-graph` for connectivity graph construction.
Uses `rustfft` for FFT operations and `ndarray` for matrix computations.

## License

MIT OR Apache-2.0
