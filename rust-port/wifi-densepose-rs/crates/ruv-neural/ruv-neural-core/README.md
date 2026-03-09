# ruv-neural-core

Core types, traits, and error types for the rUv Neural brain topology analysis system.

## Overview

`ruv-neural-core` is the foundation crate of the rUv Neural workspace. It defines all
shared data types, trait interfaces, and the RVF binary file format used across the
other eleven crates. This crate has **zero** internal dependencies -- every other
ruv-neural crate depends on it.

## Features

- **Sensor types**: `SensorType`, `SensorChannel`, `SensorArray` with sensitivity specs
  for NV diamond, OPM, SQUID MEG, and EEG sensors
- **Signal types**: `MultiChannelTimeSeries`, `FrequencyBand` (delta through gamma + custom),
  `SpectralFeatures`, `TimeFrequencyMap`
- **Brain atlas**: `Atlas` (Desikan-Killiany 68, Destrieux 148, Schaefer 100/200/400, custom),
  `BrainRegion`, `Parcellation` with hemisphere and lobe queries
- **Graph types**: `BrainGraph` with adjacency matrix, density, and degree methods;
  `BrainEdge`, `ConnectivityMetric`, `BrainGraphSequence`
- **Topology types**: `MincutResult`, `MultiPartition`, `TopologyMetrics`, `CognitiveState`,
  `SleepStage`
- **Embedding types**: `NeuralEmbedding` with cosine similarity and Euclidean distance,
  `EmbeddingTrajectory`, `EmbeddingMetadata`
- **RVF format**: Binary RuVector File format with magic bytes, versioned headers,
  typed payloads, and read/write round-trip support
- **Trait definitions**: `SensorSource`, `SignalProcessor`, `GraphConstructor`,
  `TopologyAnalyzer`, `EmbeddingGenerator`, `NeuralMemory`, `StateDecoder`,
  `RvfSerializable`
- **Error handling**: `RuvNeuralError` enum with `DimensionMismatch`, `ChannelOutOfRange`,
  `InsufficientData`, and domain-specific variants
- **Feature flags**: `std` (default), `no_std` (ESP32/embedded), `wasm`, `rvf`

## Usage

```rust
use ruv_neural_core::{
    BrainGraph, BrainEdge, ConnectivityMetric, FrequencyBand, Atlas,
    NeuralEmbedding, EmbeddingMetadata, CognitiveState,
    MultiChannelTimeSeries, RvfFile, RvfDataType,
};

// Create a brain graph
let graph = BrainGraph {
    num_nodes: 3,
    edges: vec![BrainEdge {
        source: 0, target: 1, weight: 0.8,
        metric: ConnectivityMetric::PhaseLockingValue,
        frequency_band: FrequencyBand::Alpha,
    }],
    timestamp: 0.0,
    window_duration_s: 1.0,
    atlas: Atlas::DesikanKilliany68,
};
let matrix = graph.adjacency_matrix();
let density = graph.density();

// Create a neural embedding
let meta = EmbeddingMetadata {
    subject_id: Some("sub-01".into()),
    session_id: None,
    cognitive_state: Some(CognitiveState::Focused),
    source_atlas: Atlas::Schaefer100,
    embedding_method: "spectral".into(),
};
let emb = NeuralEmbedding::new(vec![3.0, 4.0], 1000.0, meta).unwrap();
assert_eq!(emb.dimension, 2);
assert!((emb.norm() - 5.0).abs() < 1e-10);

// Write/read RVF files
let mut rvf = RvfFile::new(RvfDataType::BrainGraph);
rvf.data = serde_json::to_vec(&graph).unwrap();
let mut buf = Vec::new();
rvf.write_to(&mut buf).unwrap();
```

## API Reference

| Module      | Key Types                                                      |
|-------------|----------------------------------------------------------------|
| `sensor`    | `SensorType`, `SensorChannel`, `SensorArray`                   |
| `signal`    | `MultiChannelTimeSeries`, `FrequencyBand`, `SpectralFeatures`  |
| `brain`     | `Atlas`, `BrainRegion`, `Parcellation`, `Hemisphere`, `Lobe`   |
| `graph`     | `BrainGraph`, `BrainEdge`, `ConnectivityMetric`                |
| `topology`  | `MincutResult`, `TopologyMetrics`, `CognitiveState`            |
| `embedding` | `NeuralEmbedding`, `EmbeddingTrajectory`, `EmbeddingMetadata`  |
| `rvf`       | `RvfFile`, `RvfHeader`, `RvfDataType`                          |
| `traits`    | `SensorSource`, `SignalProcessor`, `EmbeddingGenerator`, etc.  |
| `error`     | `RuvNeuralError`, `Result<T>`                                  |

## Integration

This crate is a dependency of every other crate in the ruv-neural workspace.
It provides the shared type vocabulary that allows crates to interoperate --
for example, `ruv-neural-signal` produces `MultiChannelTimeSeries` values,
`ruv-neural-graph` consumes them, and `ruv-neural-embed` outputs
`NeuralEmbedding` values that `ruv-neural-memory` stores.

## License

MIT OR Apache-2.0
