# ruv-neural-mincut

Dynamic minimum cut analysis for brain network topology detection.

## Overview

`ruv-neural-mincut` provides algorithms for computing minimum cuts on brain
connectivity graphs, tracking topology changes over time, and detecting neural
coherence events such as network formation, dissolution, merger, and split.
These algorithms form the core of the rUv Neural cognitive state detection
pipeline, identifying when brain network topology undergoes significant
structural transitions.

## Features

- **Stoer-Wagner** (`stoer_wagner`): Global minimum cut in O(V^3) time, returning
  cut value, partitions, and cut edges
- **Normalized cut** (`normalized`): Shi-Malik spectral bisection via the Fiedler
  vector for balanced graph partitioning
- **Multiway cut** (`multiway`): Recursive normalized cut for k-module detection;
  `detect_modules` for automatic module count selection
- **Spectral cut** (`spectral_cut`): Cheeger constant computation, spectral bisection,
  and Cheeger bound estimation
- **Dynamic tracking** (`dynamic`): `DynamicMincutTracker` for temporal mincut
  evolution tracking with `TopologyTransition` and `TransitionDirection` detection
- **Coherence detection** (`coherence`): `CoherenceDetector` identifying
  `CoherenceEventType` events (formation, dissolution, merger, split) from
  temporal graph sequences
- **Benchmarks** (`benchmark`): Performance benchmarking utilities

## Usage

```rust
use ruv_neural_mincut::{
    stoer_wagner_mincut, normalized_cut, spectral_bisection,
    cheeger_constant, multiway_cut, detect_modules,
    DynamicMincutTracker, CoherenceDetector,
};
use ruv_neural_core::graph::BrainGraph;

// Compute global minimum cut
let result = stoer_wagner_mincut(&graph);
println!("Cut value: {:.3}", result.cut_value);
println!("Partition A: {:?}", result.partition_a);
println!("Partition B: {:?}", result.partition_b);

// Normalized cut (spectral bisection)
let ncut = normalized_cut(&graph);

// Spectral analysis
let (partition, cheeger) = spectral_bisection(&graph);
let h = cheeger_constant(&graph);

// Multiway cut for k modules
let multi = multiway_cut(&graph, 4);
let auto_modules = detect_modules(&graph);

// Track topology transitions over time
let mut tracker = DynamicMincutTracker::new();
for graph in &graph_sequence.graphs {
    let result = tracker.update(graph).unwrap();
}

// Detect coherence events
let mut detector = CoherenceDetector::new();
for graph in &graph_sequence.graphs {
    if let Some(event) = detector.check(graph) {
        println!("Event: {:?} at t={}", event.event_type, event.timestamp);
    }
}
```

## API Reference

| Module          | Key Types / Functions                                           |
|-----------------|-----------------------------------------------------------------|
| `stoer_wagner`  | `stoer_wagner_mincut`                                           |
| `normalized`    | `normalized_cut`                                                |
| `multiway`      | `multiway_cut`, `detect_modules`                                |
| `spectral_cut`  | `spectral_bisection`, `cheeger_constant`, `cheeger_bound`       |
| `dynamic`       | `DynamicMincutTracker`, `TopologyTransition`, `TransitionDirection` |
| `coherence`     | `CoherenceDetector`, `CoherenceEvent`, `CoherenceEventType`     |
| `benchmark`     | Benchmark utilities                                             |

## Feature Flags

| Feature     | Default | Description                      |
|-------------|---------|----------------------------------|
| `std`       | Yes     | Standard library support         |
| `wasm`      | No      | WASM-compatible implementations  |
| `sublinear` | No      | Sublinear mincut algorithms      |

## Integration

Depends on `ruv-neural-core` for `BrainGraph`, `MincutResult`, and `MultiPartition`
types. Receives graphs from `ruv-neural-graph`. Mincut results feed into
`ruv-neural-embed` for topology-aware embeddings and `ruv-neural-decoder`
for cognitive state classification.

## License

MIT OR Apache-2.0
