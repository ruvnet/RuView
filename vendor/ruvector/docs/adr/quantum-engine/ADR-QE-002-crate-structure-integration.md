# ADR-QE-002: Crate Structure & ruVector Integration

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Context

### Problem Statement

The quantum engine must fit within the ruVector workspace, which currently
comprises 73+ crates following a consistent modular architecture. The existing
`ruQu` crate handles classical coherence monitoring -- specifically min-cut
analysis and MWPM (Minimum Weight Perfect Matching) decoding for error
correction analysis. The new quantum simulation capability requires clear
separation from this classical functionality while integrating deeply with
ruVector's shared infrastructure.

### Existing Workspace Patterns

The ruVector workspace follows established conventions that the quantum engine
must respect:

```
ruvector/
  crates/
    ruvector-math/          # SIMD-optimized linear algebra
    ruvector-hnsw/          # Vector similarity search
    ruvector-metrics/       # Observability and telemetry
    ruvector-router-wasm/   # WASM bindings for routing
    ruQu/                   # Classical coherence (min-cut, MWPM)
    ...73+ crates
  Cargo.toml                # Workspace root
```

Key conventions observed:

- **`no_std` + `alloc`** for maximum portability
- **Feature flags** for optional capabilities (parallel, gpu, etc.)
- **Separate WASM crates** for browser-facing bindings (e.g., `ruvector-router-wasm`)
- **Metrics integration** via `ruvector-metrics` for observability
- **SIMD reuse** via `ruvector-math` for hot-path computations

### Integration Points

The quantum engine must interact with several existing subsystems:

```
                    +-------------------+
                    |  Agent Framework  |
                    +--------+----------+
                             |
                    trigger circuit execution
                             |
                    +--------v----------+
                    |   ruqu-core       |
                    | (quantum sim)     |
                    +---+------+--------+
                        |      |
             +----------+      +----------+
             |                            |
    +--------v--------+       +-----------v---------+
    | ruvector-math   |       | ruvector-metrics    |
    | (SIMD, linalg)  |       | (telemetry)         |
    +-----------------+       +---------------------+
             |
    +--------v--------+
    | ruQu (existing) |
    | (min-cut, MWPM) |
    +-----------------+
```

## Decision

Adopt a **three-crate architecture** for the quantum engine, each with a
clearly defined responsibility boundary.

### Crate 1: `ruqu-core` -- Pure Rust Simulation Library

The core simulation engine, containing all quantum computation logic.

**Responsibilities**:
- `QuantumCircuit`: Circuit representation and manipulation
- `QuantumState`: State-vector storage and operations
- `Gate` enum: Full gate set (Pauli, Hadamard, CNOT, Toffoli, parametric rotations, etc.)
- Measurement operations (computational basis, Pauli basis, mid-circuit)
- Circuit optimization passes (gate fusion, cancellation)
- Noise model application (optional)
- Entanglement tracking for state splitting

**Design constraints**:
- `#![no_std]` with `alloc` for embedded/WASM portability
- Zero required external dependencies beyond `alloc`
- All platform-specific code behind feature flags

**Feature flags**:

| Flag | Default | Description |
|------|---------|-------------|
| `std` | off | Enable std library features (file I/O, advanced error types) |
| `parallel` | off | Enable Rayon-based multi-threaded gate application |
| `gpu` | off | Enable wgpu-based GPU acceleration for large states |
| `tensor-network` | off | Enable tensor network backend for shallow circuits |
| `noise-model` | off | Enable depolarizing, amplitude damping, and custom noise channels |
| `f32` | off | Use f32 precision instead of f64 (halves memory, reduces accuracy) |
| `serde` | off | Enable serialization of circuits and states |

**Module structure**:

```
ruqu-core/
  src/
    lib.rs              # Crate root, feature flag gating
    state.rs            # QuantumState: amplitude storage, initialization
    circuit.rs          # QuantumCircuit: gate sequence, metadata
    gates/
      mod.rs            # Gate enum and dispatch
      single.rs         # Single-qubit gates (H, X, Y, Z, S, T, Rx, Ry, Rz, U3)
      two.rs            # Two-qubit gates (CNOT, CZ, SWAP, Rxx, Ryy, Rzz)
      multi.rs          # Multi-qubit gates (Toffoli, Fredkin, custom unitaries)
      parametric.rs     # Parameterized gate support for variational algorithms
    execution/
      mod.rs            # Execution engine dispatch
      statevector.rs    # Full state-vector simulation engine
      tensor.rs         # Tensor network backend (feature-gated)
      noise.rs          # Noise channel application (feature-gated)
    measurement.rs      # Measurement: sampling, expectation values
    optimize/
      mod.rs            # Circuit optimization pipeline
      fusion.rs         # Gate fusion pass
      cancel.rs         # Gate cancellation (HH=I, XX=I, etc.)
      commute.rs        # Commutation-based reordering
    entanglement.rs     # Entanglement tracking and state splitting
    types.rs            # Complex number types, precision configuration
    error.rs            # Error types (QubitOverflow, InvalidGate, etc.)
  Cargo.toml
  benches/
    statevector.rs      # Criterion benchmarks for core operations
```

**Public API surface**:

```rust
// Core types
pub struct QuantumState { /* ... */ }
pub struct QuantumCircuit { /* ... */ }
pub enum Gate { H, X, Y, Z, S, T, CNOT, CZ, Rx(f64), Ry(f64), Rz(f64), /* ... */ }

// Circuit construction
impl QuantumCircuit {
    pub fn new(num_qubits: usize) -> Result<Self, QubitOverflow>;
    pub fn gate(&mut self, gate: Gate, targets: &[usize]) -> &mut Self;
    pub fn measure(&mut self, qubit: usize) -> &mut Self;
    pub fn measure_all(&mut self) -> &mut Self;
    pub fn barrier(&mut self) -> &mut Self;
    pub fn depth(&self) -> usize;
    pub fn gate_count(&self) -> usize;
    pub fn optimize(&mut self) -> &mut Self;
}

// Execution
impl QuantumState {
    pub fn new(num_qubits: usize) -> Result<Self, QubitOverflow>;
    pub fn execute(&mut self, circuit: &QuantumCircuit) -> ExecutionResult;
    pub fn sample(&self, shots: usize) -> Vec<BitString>;
    pub fn expectation(&self, observable: &Observable) -> f64;
    pub fn probabilities(&self) -> Vec<f64>;
    pub fn amplitude(&self, basis_state: usize) -> Complex<f64>;
}
```

### Crate 2: `ruqu-wasm` -- WebAssembly Bindings

WASM-specific bindings exposing the quantum engine to JavaScript environments.

**Responsibilities**:
- wasm-bindgen annotated wrapper types
- JavaScript-friendly API (string-based circuit construction, JSON results)
- Memory limit enforcement (reject circuits exceeding WASM address space)
- Optional multi-threading via wasm-bindgen-rayon

**Design constraints**:
- Mirrors the `ruvector-router-wasm` crate pattern
- Thin wrapper; all logic delegated to `ruqu-core`
- TypeScript type definitions auto-generated

**Module structure**:

```
ruqu-wasm/
  src/
    lib.rs              # wasm-bindgen entry points
    circuit.rs          # JS-facing QuantumCircuit wrapper
    state.rs            # JS-facing QuantumState wrapper
    types.rs            # JS-compatible type conversions
    limits.rs           # WASM memory limit checks
  Cargo.toml
  pkg/                  # wasm-pack output (generated)
  tests/
    web.rs              # wasm-bindgen-test browser tests
```

**JavaScript API**:

```javascript
import { QuantumCircuit, QuantumState } from 'ruqu-wasm';

// Construct circuit
const circuit = new QuantumCircuit(4);
circuit.h(0);
circuit.cnot(0, 1);
circuit.cnot(1, 2);
circuit.cnot(2, 3);
circuit.measureAll();

// Execute
const state = new QuantumState(4);
const result = state.execute(circuit);

// Sample measurement outcomes
const counts = state.sample(1024);
console.log(counts);  // { "0000": 512, "1111": 512 }

// Get probabilities
const probs = state.probabilities();
```

**Memory limit enforcement**:

```rust
const WASM_MAX_QUBITS: usize = 25;
const WASM_MAX_STATE_BYTES: usize = 1 << 30; // 1 GB

pub fn check_wasm_limits(num_qubits: usize) -> Result<(), WasmLimitError> {
    if num_qubits > WASM_MAX_QUBITS {
        return Err(WasmLimitError::QubitOverflow {
            requested: num_qubits,
            maximum: WASM_MAX_QUBITS,
            estimated_bytes: 16 * (1usize << num_qubits),
        });
    }
    Ok(())
}
```

### Crate 3: `ruqu-algorithms` -- High-Level Algorithm Implementations

Quantum algorithm implementations built on top of `ruqu-core`.

**Responsibilities**:
- VQE (Variational Quantum Eigensolver) with classical optimizer integration
- Grover's search with oracle construction helpers
- QAOA (Quantum Approximate Optimization Algorithm)
- Quantum error correction (surface codes, stabilizer codes)
- Hamiltonian simulation primitives (Trotterization)

**Module structure**:

```
ruqu-algorithms/
  src/
    lib.rs
    vqe/
      mod.rs            # VQE orchestration
      ansatz.rs         # Parameterized ansatz circuits (UCCSD, HEA)
      hamiltonian.rs    # Hamiltonian representation and decomposition
      optimizer.rs      # Classical optimizer trait + implementations
    grover/
      mod.rs            # Grover's algorithm orchestration
      oracle.rs         # Oracle construction utilities
      diffusion.rs      # Diffusion operator
    qaoa/
      mod.rs            # QAOA orchestration
      mixer.rs          # Mixer Hamiltonian circuits
      cost.rs           # Cost function encoding
    qec/
      mod.rs            # QEC framework
      surface.rs        # Surface code implementation
      stabilizer.rs     # Stabilizer formalism
      decoder.rs        # Bridge to ruQu's MWPM decoder
    trotter.rs          # Trotterization for Hamiltonian simulation
    utils.rs            # Shared utilities (state preparation, etc.)
  Cargo.toml
```

**VQE example**:

```rust
use ruqu_core::{QuantumCircuit, QuantumState};
use ruqu_algorithms::vqe::{VqeSolver, Hamiltonian, HardwareEfficientAnsatz};

let hamiltonian = Hamiltonian::from_pauli_sum(&[
    (0.5, "ZZ", &[0, 1]),
    (0.3, "X",  &[0]),
    (0.3, "X",  &[1]),
]);

let ansatz = HardwareEfficientAnsatz::new(2, depth: 3);

let solver = VqeSolver::new(hamiltonian, ansatz)
    .optimizer(NelderMead::default())
    .max_iterations(200)
    .convergence_threshold(1e-6);

let result = solver.solve();
println!("Ground state energy: {:.6}", result.energy);
```

### Integration Points

#### Agent Activation

Quantum circuits are triggered via the ruVector agent context system. An agent
can invoke simulation through graph query extensions:

```
Agent Query: "Simulate VQE for H2 molecule at bond length 0.74 A"
    |
    v
Agent Framework --> ruqu-algorithms::vqe::VqeSolver
    |                    |
    |                    +--> ruqu-core (multiple circuit executions)
    |                    |
    |<-- VqeResult ------+
    |
    v
Agent Response: { energy: -1.137, parameters: [...], iterations: 47 }
```

#### Memory Gating

Following ruVector's memory discipline (ADR-006):

- State vectors allocated exclusively within `QuantumState::new()` scope
- All amplitudes dropped when `QuantumState` goes out of scope
- No lazy or cached allocations persist between simulations
- Peak memory tracked and reported via `ruvector-metrics`

#### Observability

Every simulation reports metrics through the existing `ruvector-metrics` pipeline:

| Metric | Type | Description |
|--------|------|-------------|
| `ruqu.simulation.qubits` | Gauge | Number of qubits in current simulation |
| `ruqu.simulation.gates` | Counter | Total gates applied |
| `ruqu.simulation.depth` | Gauge | Circuit depth after optimization |
| `ruqu.simulation.duration_ns` | Histogram | Wall-clock simulation time |
| `ruqu.simulation.peak_memory_bytes` | Gauge | Peak memory during simulation |
| `ruqu.optimization.gates_eliminated` | Counter | Gates removed by optimization passes |
| `ruqu.measurement.shots` | Counter | Total measurement shots taken |

#### Coherence Bridge

The existing `ruQu` crate's min-cut analysis and MWPM decoders remain in place
and become accessible from `ruqu-algorithms` for quantum error correction:

```
ruqu-algorithms::qec::surface
    |
    +-- build syndrome graph
    |
    +-- invoke ruQu::mwpm::decode(syndrome)
    |
    +-- apply corrections to ruqu-core::QuantumState
```

This avoids duplicating decoding logic and leverages the existing, tested
classical infrastructure.

#### Math Reuse

`ruqu-core` depends on `ruvector-math` for SIMD-optimized operations:

- Complex number arithmetic (add, multiply, conjugate) using SIMD lanes
- Aligned memory allocation for state vectors
- Batch operations on amplitude arrays
- Norm calculation for state normalization

```rust
// In ruqu-core, gate application uses ruvector-math SIMD utilities
use ruvector_math::simd::{complex_mul_f64x4, complex_add_f64x4};

fn apply_single_qubit_gate(
    state: &mut [Complex<f64>],
    target: usize,
    matrix: [[Complex<f64>; 2]; 2],
) {
    let step = 1 << target;
    for block in (0..state.len()).step_by(2 * step) {
        for i in block..block + step {
            let (a, b) = (state[i], state[i + step]);
            state[i]        = matrix[0][0] * a + matrix[0][1] * b;
            state[i + step] = matrix[1][0] * a + matrix[1][1] * b;
        }
    }
}
```

### Dependency Graph

```
ruqu-algorithms
    |
    +---> ruqu-core
    |        |
    |        +---> ruvector-math (SIMD utilities)
    |        +---> ruvector-metrics (optional, behind "metrics" feature)
    |
    +---> ruQu (existing, for MWPM decoders in QEC)

ruqu-wasm
    |
    +---> ruqu-core
    +---> wasm-bindgen
    +---> wasm-bindgen-rayon (optional, behind "threads" feature)
```

### Workspace Cargo.toml Additions

```toml
[workspace]
members = [
    # ... existing 73+ crates ...
    "crates/ruqu-core",
    "crates/ruqu-wasm",
    "crates/ruqu-algorithms",
]
```

## Consequences

### Positive

- **Clean separation of concerns**: Each crate has a single, well-defined
  responsibility -- simulation, WASM bindings, and algorithms respectively
- **Independent testing**: Each crate can be tested in isolation with its own
  benchmark suite
- **Minimal WASM surface**: `ruqu-wasm` remains a thin wrapper, keeping the
  compiled `.wasm` module small
- **Reuse of infrastructure**: SIMD, metrics, and classical decoders are shared,
  not duplicated
- **Follows workspace conventions**: Same patterns as existing crates, reducing
  onboarding friction for contributors

### Negative

- **Three crates to maintain**: Each requires its own CI, documentation, and
  version management
- **Cross-crate API stabilization**: Changes to `ruqu-core`'s public API affect
  both `ruqu-wasm` and `ruqu-algorithms`
- **Feature flag combinatorics**: Multiple feature flags across three crates
  create a testing matrix that must be validated

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| API churn in ruqu-core destabilizing dependents | Semver discipline; stabilize core types before 1.0 |
| Feature flag combinations causing compilation failures | CI matrix testing all supported flag combinations |
| Coherence bridge creating tight coupling with ruQu | Trait-based decoder interface; ruQu dependency optional |
| WASM crate size exceeding 2MB target | Regular binary size audits; aggressive dead code elimination |

## References

- [ADR-QE-001: Quantum Engine Core Architecture](./ADR-QE-001-quantum-engine-core-architecture.md)
- [ADR-QE-003: WASM Compilation Strategy](./ADR-QE-003-wasm-compilation-strategy.md)
- [ADR-QE-004: Performance Optimization & Benchmarks](./ADR-QE-004-performance-optimization-benchmarks.md)
- [Workspace Cargo.toml](/Cargo.toml)
- [ruvector-router-wasm pattern](/crates/ruvector-router-wasm/)
- [ruQu crate](/crates/ruQu/)
- [ruvector-math crate](/crates/ruvector-math/)
- [ruvector-metrics crate](/crates/ruvector-metrics/)
