# ruv-neural-sensor

Sensor data acquisition for NV diamond, OPM, EEG, and simulated sources.

## Overview

`ruv-neural-sensor` provides uniform sensor interfaces for multiple neural
magnetometry and electrophysiology sensor types. Each sensor backend implements
the `SensorSource` trait from `ruv-neural-core`, producing `MultiChannelTimeSeries`
data. The crate also includes calibration utilities and real-time signal quality
monitoring.

## Features

- **Simulated sensor** (`simulator` feature, default): Synthetic multi-channel data
  generation with configurable alpha rhythm injection, noise floor control, and
  event injection (spikes, artifacts)
- **NV diamond** (`nv_diamond` feature): Nitrogen-vacancy diamond magnetometer
  interface with configurable sensitivity and channel layout
- **OPM** (`opm` feature): Optically pumped magnetometer array with configurable
  geometry
- **EEG** (`eeg` feature): Electroencephalography sensor interface
- **Calibration**: Gain/offset correction, noise floor estimation, and cross-calibration
  between reference and target channels
- **Quality monitoring**: Real-time SNR estimation, artifact probability scoring,
  and saturation detection with configurable alert thresholds

## Usage

```rust
use ruv_neural_sensor::simulator::{SimulatedSensorArray, SensorEvent};
use ruv_neural_sensor::{SensorSource, SensorType};

// Create a simulated 16-channel array at 1000 Hz
let mut sim = SimulatedSensorArray::new(16, 1000.0);
sim.inject_alpha(100.0); // 100 fT alpha rhythm

// Read 500 samples via the SensorSource trait
let data = sim.read_chunk(500).unwrap();
assert_eq!(data.num_channels, 16);
assert_eq!(data.num_samples, 500);

// Inject a spike event
sim.inject_event(SensorEvent::Spike {
    channel: 0,
    amplitude_ft: 500.0,
    sample_offset: 100,
});

// Calibrate channels
use ruv_neural_sensor::calibration::{CalibrationData, calibrate_channel};
let cal = CalibrationData {
    gains: vec![2.0],
    offsets: vec![10.0],
    noise_floors: vec![1.0],
};
let corrected = calibrate_channel(100.0, 0, &cal); // (100 - 10) * 2 = 180

// Monitor signal quality
use ruv_neural_sensor::quality::QualityMonitor;
let mut monitor = QualityMonitor::new(2);
let qualities = monitor.check_quality(&[&data.data[0], &data.data[1]]);
```

## API Reference

| Module        | Key Types / Functions                                        |
|---------------|--------------------------------------------------------------|
| `simulator`   | `SimulatedSensorArray`, `SensorEvent`                        |
| `nv_diamond`  | `NvDiamondArray`, `NvDiamondConfig`                          |
| `opm`         | `OpmArray`, `OpmConfig`                                      |
| `eeg`         | `EegArray`, `EegConfig`                                      |
| `calibration` | `CalibrationData`, `calibrate_channel`, `cross_calibrate`    |
| `quality`     | `QualityMonitor`, `SignalQuality`                            |

## Feature Flags

| Feature     | Default | Description                          |
|-------------|---------|--------------------------------------|
| `simulator` | Yes     | Synthetic test data generator        |
| `nv_diamond`| No      | NV diamond magnetometer backend      |
| `opm`       | No      | Optically pumped magnetometer backend|
| `eeg`       | No      | EEG sensor backend                   |

## Integration

Depends on `ruv-neural-core` for the `SensorSource` trait and `MultiChannelTimeSeries`
type. Produced data feeds into `ruv-neural-signal` for preprocessing and filtering.

## License

MIT OR Apache-2.0
