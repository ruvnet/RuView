# ruqu-exotic

[![Crates.io](https://img.shields.io/crates/v/ruqu-exotic.svg)](https://crates.io/crates/ruqu-exotic)
[![Documentation](https://docs.rs/ruqu-exotic/badge.svg)](https://docs.rs/ruqu-exotic)
[![License](https://img.shields.io/crates/l/ruqu-exotic.svg)](https://github.com/ruvnet/ruvector)

**Experimental quantum-classical hybrid algorithms** — quantum memory decay, interference-based search, reasoning error correction, swarm interference, syndrome diagnosis, and reversible memory for AI systems.

## Algorithms

| Module | Description | Application |
|--------|-------------|-------------|
| **Quantum Decay** | Temporal coherence loss modeling | Memory systems, caching |
| **Interference Search** | Quantum-inspired amplitude interference | Vector similarity |
| **Reasoning QEC** | Error correction for AI reasoning chains | LLM reliability |
| **Swarm Interference** | Multi-agent quantum coordination | Distributed AI |
| **Syndrome Diagnosis** | Error pattern detection | System health |
| **Reversible Memory** | Quantum-reversible state management | Undo/redo systems |

## Installation

```bash
cargo add ruqu-exotic
```

## Quantum Memory Decay

Model temporal coherence loss in memory systems:

```rust
use ruqu_exotic::quantum_decay::{DecayModel, MemoryState};

let model = DecayModel::new()
    .t1(100.0)      // Amplitude decay time (μs)
    .t2(50.0)       // Phase decay time (μs)
    .temperature(0.02);  // Thermal noise

let state = MemoryState::from_embedding(embedding);
let decayed = model.evolve(state, time: 10.0)?;

println!("Fidelity after 10μs: {:.2}%", decayed.fidelity() * 100.0);
```

## Interference Search

Quantum-inspired amplitude interference for similarity search:

```rust
use ruqu_exotic::interference_search::{InterferenceIndex, Query};

let mut index = InterferenceIndex::new(dimension: 384);
index.add_vectors(&embeddings)?;

// Constructive interference amplifies similar vectors
let query = Query::new(query_embedding)
    .interference_rounds(3)
    .phase_kickback(true);

let results = index.search(query, k: 10)?;
```

## Reasoning Error Correction

Detect and correct errors in AI reasoning chains:

```rust
use ruqu_exotic::reasoning_qec::{ReasoningCode, LogicalChain};

let code = ReasoningCode::new()
    .redundancy(3)           // Triple modular redundancy
    .syndrome_bits(2);       // Error detection bits

let chain = LogicalChain::from_steps(&[
    "Premise: All A are B",
    "Premise: X is A",
    "Conclusion: X is B"
]);

let protected = code.encode(chain)?;
let (decoded, errors) = code.decode_and_correct(protected)?;
println!("Detected {} logical errors", errors.len());
```

## Swarm Interference

Coordinate multi-agent systems with quantum interference:

```rust
use ruqu_exotic::swarm_interference::{SwarmState, Agent};

let mut swarm = SwarmState::new(n_agents: 8);

// Agents interfere constructively on consensus
for round in 0..10 {
    swarm.apply_interference()?;
    swarm.measure_partial()?;  // Partial collapse
}

let consensus = swarm.final_state()?;
```

## Syndrome Diagnosis

Detect error patterns in distributed systems:

```rust
use ruqu_exotic::syndrome_diagnosis::{Diagnostics, Pattern};

let diag = Diagnostics::new()
    .stabilizers(&["XXXX", "ZZZZ"])
    .measurement_noise(0.01);

let syndromes = diag.measure(&system_state)?;
let errors = diag.decode_syndromes(syndromes)?;

for error in errors {
    println!("Error at {:?}: {:?}", error.location, error.type_);
}
```

## Reversible Memory

Quantum-reversible operations for undo/redo:

```rust
use ruqu_exotic::reversible_memory::{ReversibleStore, Operation};

let mut store = ReversibleStore::new();

store.apply(Operation::Insert { key: "a", value: vec![1,2,3] })?;
store.apply(Operation::Update { key: "a", value: vec![4,5,6] })?;

// Perfect reversal via uncompute
store.reverse_last()?;  // Back to [1,2,3]
store.reverse_last()?;  // Back to empty
```

## Integration with RuVector

These algorithms integrate with the [RuVector](https://github.com/ruvnet/ruvector) vector database for quantum-enhanced AI:

```rust
use ruvector_core::Index;
use ruqu_exotic::interference_search::InterferenceIndex;

// Wrap RuVector index with interference search
let base_index = Index::new(config)?;
let quantum_index = InterferenceIndex::wrap(base_index)?;
```

## Related Crates

- [`ruqu-core`](https://crates.io/crates/ruqu-core) — Quantum circuit simulator
- [`ruqu-algorithms`](https://crates.io/crates/ruqu-algorithms) — VQE, Grover, QAOA, Surface Code
- [`ruqu-wasm`](https://crates.io/crates/ruqu-wasm) — WebAssembly bindings

## Documentation

- [Exotic Discoveries (ADR-QE-014)](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-014-exotic-discoveries.md)
- [MinCut Coherence (ADR-QE-012)](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-012-mincut-coherence-integration.md)

## License

MIT OR Apache-2.0
