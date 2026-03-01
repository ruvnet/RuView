# exo-core

Core traits and types for the EXO-AI cognitive substrate. Provides IIT
(Integrated Information Theory) consciousness measurement and Landauer
thermodynamics primitives that every other EXO crate builds upon.

## Features

- **SubstrateBackend trait** -- unified interface for pluggable compute
  backends (classical, quantum, hybrid).
- **IIT Phi measurement** -- quantifies integrated information across
  cognitive graph partitions.
- **Landauer free energy tracking** -- monitors thermodynamic cost of
  irreversible bit erasure during inference.
- **Coherence routing** -- directs information flow to maximise substrate
  coherence scores.
- **Plasticity engine (SONA EWC++)** -- continual learning with elastic
  weight consolidation to prevent catastrophic forgetting.
- **Genomic integration** -- encodes and decodes cognitive parameters as
  compact genomic sequences for evolution-based search.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-core = "0.1"
```

Basic usage:

```rust
use exo_core::consciousness::{ConsciousnessSubstrate, IITConfig};
use exo_core::thermodynamics::CognitiveThermometer;

// Measure integrated information (Phi)
let substrate = ConsciousnessSubstrate::new(IITConfig::default());
substrate.add_pattern(pattern);
let phi = substrate.compute_phi();

// Track computational thermodynamics
let thermo = CognitiveThermometer::new(300.0); // Kelvin
let cost = thermo.landauer_cost_bits(1024);
println!("Landauer cost for 1024 bits: {:.6} kT", cost);
```

## Crate Layout

| Module        | Purpose                                |
|---------------|----------------------------------------|
| `backend`     | SubstrateBackend trait and helpers      |
| `iit`         | Phi computation and partition analysis  |
| `thermo`      | Landauer energy and entropy bookkeeping |
| `coherence`   | Routing and coherence scoring           |
| `plasticity`  | SONA EWC++ continual-learning engine    |
| `genomic`     | Genome encoding / decoding utilities    |

## Requirements

- Rust 1.78+
- No required system dependencies

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
