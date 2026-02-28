# ruvector-cognitive-container

Verifiable WASM cognitive container with canonical witness chains for the RuVector ecosystem.

## Features

- **Epoch Controller**: Phase-budgeted tick execution (ingest/mincut/spectral/evidence/witness)
- **Memory Slab**: Arena-based allocation for graph data
- **Witness Chain**: Hash-linked chain of `ContainerWitnessReceipt` for deterministic verification
- **Cognitive Container**: Full orchestration with snapshot/restore support

## Usage

```rust
use ruvector_cognitive_container::{CognitiveContainer, ContainerConfig, Delta};

let config = ContainerConfig::default();
let mut container = CognitiveContainer::new(config).unwrap();

let deltas = vec![
    Delta::EdgeAdd { u: 0, v: 1, weight: 1.0 },
    Delta::Observation { node: 0, value: 0.8 },
];

let result = container.tick(&deltas).unwrap();
println!("Min-cut: {}", result.min_cut_value);

// Verify witness chain integrity
let verification = container.verify_chain();
```

## License

MIT
