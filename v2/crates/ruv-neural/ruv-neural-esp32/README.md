# ruv-neural-esp32

ESP32 edge integration for neural sensor data acquisition and preprocessing.

## Overview

`ruv-neural-esp32` provides lightweight processing modules designed to run on
ESP32 microcontrollers for real-time neural sensor data acquisition and
preprocessing at the edge. It handles ADC sampling, time-division multiplexing
for multi-sensor coordination, IIR filtering and downsampling on-device, power
management for battery operation, a binary communication protocol for streaming
data to the rUv Neural backend, and multi-node data aggregation.

## Features

- **ADC interface** (`adc`): `AdcReader` with configurable `AdcConfig` including
  sample rate, resolution, attenuation levels, and multi-channel support via
  `AdcChannel`
- **TDM scheduling** (`tdm`): `TdmScheduler` and `TdmNode` for time-division
  multiplexed multi-sensor coordination with configurable `SyncMethod`
  (GPIO trigger, I2S clock, software timer)
- **Edge preprocessing** (`preprocessing`): `EdgePreprocessor` with fixed-point
  IIR filters (`IirCoeffs`), downsampling, and DC offset removal optimized
  for constrained embedded environments
- **Communication protocol** (`protocol`): `NeuralDataPacket` with `PacketHeader`
  and `ChannelData` for efficient binary data streaming to the backend over
  UART, SPI, or WiFi
- **Power management** (`power`): `PowerManager` with `PowerConfig` and `PowerMode`
  (active, light sleep, deep sleep, hibernate) for battery-powered deployments
- **Multi-node aggregation** (`aggregator`): `NodeAggregator` for combining data
  from multiple ESP32 nodes into synchronized multi-channel streams

## Usage

```rust
use ruv_neural_esp32::{
    AdcReader, AdcConfig, Attenuation,
    TdmScheduler, TdmNode, SyncMethod,
    EdgePreprocessor, IirCoeffs,
    NeuralDataPacket, PacketHeader, ChannelData,
    PowerManager, PowerConfig, PowerMode,
    NodeAggregator,
};

// Configure ADC for 4-channel acquisition
let config = AdcConfig {
    sample_rate_hz: 1000,
    resolution_bits: 12,
    attenuation: Attenuation::Db11,
    channels: vec![
        AdcChannel { pin: 32, gain: 1.0 },
        AdcChannel { pin: 33, gain: 1.0 },
        AdcChannel { pin: 34, gain: 1.0 },
        AdcChannel { pin: 35, gain: 1.0 },
    ],
};
let mut adc = AdcReader::new(config);

// Set up TDM scheduling for multi-sensor sync
let scheduler = TdmScheduler::new(SyncMethod::GpioTrigger);
let node = TdmNode::new(0, scheduler);

// Preprocess on-device with IIR filter
let mut preprocessor = EdgePreprocessor::new(1000.0);
let filtered = preprocessor.process(&raw_samples);

// Build a data packet for transmission
let packet = NeuralDataPacket {
    header: PacketHeader::new(4, 250),
    channels: vec![ChannelData { samples: filtered }],
};

// Power management
let mut power = PowerManager::new(PowerConfig::default());
power.set_mode(PowerMode::LightSleep);
```

## API Reference

| Module          | Key Types                                                    |
|-----------------|--------------------------------------------------------------|
| `adc`           | `AdcReader`, `AdcConfig`, `AdcChannel`, `Attenuation`        |
| `tdm`           | `TdmScheduler`, `TdmNode`, `SyncMethod`                      |
| `preprocessing` | `EdgePreprocessor`, `IirCoeffs`                               |
| `protocol`      | `NeuralDataPacket`, `PacketHeader`, `ChannelData`             |
| `power`         | `PowerManager`, `PowerConfig`, `PowerMode`                    |
| `aggregator`    | `NodeAggregator`                                              |

## Feature Flags

| Feature     | Default | Description                              |
|-------------|---------|------------------------------------------|
| `std`       | Yes     | Standard library (desktop simulation)    |
| `no_std`    | No      | Bare-metal ESP32 target                  |
| `simulator` | No      | Simulated ADC for testing (requires std) |

## Integration

Depends on `ruv-neural-core` for shared types. Preprocessed data packets are
sent to the host system where `ruv-neural-sensor` or `ruv-neural-signal` can
consume them for further processing. Designed to run independently on ESP32
hardware or in simulation mode on desktop for testing.

## License

MIT OR Apache-2.0
