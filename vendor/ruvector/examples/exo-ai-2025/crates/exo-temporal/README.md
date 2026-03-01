# exo-temporal

Temporal memory coordinator with causal structure for the EXO-AI cognitive
substrate. Manages how memories form, persist, and decay using
physically-inspired decoherence models.

## Features

- **Causal timeline tracking** -- maintains a directed acyclic graph of
  events with Lamport-style logical clocks for strict causal ordering.
- **Quantum decay memory eviction** -- models memory lifetime using T1
  (energy relaxation) and T2 (dephasing) decoherence times, evicting
  stale entries probabilistically.
- **Anticipation engine** -- predicts future states by extrapolating
  causal trajectories, enabling proactive cognition.
- **Transfer timeline** -- records cross-domain knowledge transfers with
  full provenance so temporal reasoning spans substrate boundaries.

## Quick Start

Add the dependency to your `Cargo.toml`:

```toml
[dependencies]
exo-temporal = "0.1"
```

Basic usage:

```rust
use exo_temporal::{TemporalMemory, TemporalConfig, Pattern, Metadata};

// Create temporal memory
let memory = TemporalMemory::new(TemporalConfig::default());

// Store a pattern with causal context
let pattern = Pattern::new(vec![1.0, 2.0, 3.0], Metadata::new());
let id = memory.store(pattern, &[]).unwrap();

// Causal cone query
let results = memory.causal_query(
    &query,
    SubstrateTime::now(),
    CausalConeType::Past,
);

// Trigger consolidation and strategic forgetting
let consolidation = memory.consolidate();
memory.forget();
```

## Crate Layout

| Module           | Purpose                                  |
|------------------|------------------------------------------|
| `timeline`       | Core DAG and logical clock management    |
| `decay`          | T1/T2 decoherence eviction policies      |
| `anticipation`   | Trajectory extrapolation engine           |
| `consolidation`  | Salience-based memory consolidation       |
| `transfer`       | Cross-domain timeline provenance          |

## Requirements

- Rust 1.78+
- Depends on `exo-core`

## Links

- [GitHub](https://github.com/ruvnet/ruvector)
- [EXO-AI Documentation](https://github.com/ruvnet/ruvector/tree/main/examples/exo-ai-2025)

## License

MIT OR Apache-2.0
