# ruqu-core

[![Crates.io](https://img.shields.io/crates/v/ruqu-core.svg)](https://crates.io/crates/ruqu-core)
[![Documentation](https://docs.rs/ruqu-core/badge.svg)](https://docs.rs/ruqu-core)
[![License](https://img.shields.io/crates/l/ruqu-core.svg)](https://github.com/ruvnet/ruvector)

**Quantum Execution Intelligence Engine in pure Rust** — 5 simulation backends with automatic routing, noise models, error mitigation, OpenQASM 3.0 export, and cryptographic witness logging.

## Features

- **5 Simulation Backends** — StateVector (exact, up to 32 qubits), Stabilizer (millions of qubits), Clifford+T (moderate T-count), TensorNetwork (MPS-based), Hardware (device profiles)
- **Cost-Model Planner** — Automatically routes circuits to the optimal backend based on qubit count, gate mix, and T-count
- **Universal Gate Set** — H, X, Y, Z, CNOT, CZ, Toffoli, Rx, Ry, Rz, Phase, SWAP, and custom unitaries
- **QEC Control Plane** — Union-find decoder with O(n*a(n)) amortized time, sub-polynomial decoders, QEC scheduling, control theory integration
- **OpenQASM 3.0** — Full circuit export to standard quantum assembly format
- **Noise & Mitigation** — Depolarizing, amplitude/phase damping, custom Kraus operators, zero-noise extrapolation, probabilistic error cancellation
- **SIMD Acceleration** — AVX2/NEON vectorized gate application for 2-4x speedup
- **Multi-Threading** — Rayon-based parallelism for large qubit counts
- **Cryptographic Witnesses** — Tamper-evident execution logs for reproducibility and verification
- **Transpiler** — Gate decomposition, routing, and hardware-aware optimization
- **Mixed Precision** — Configurable f32/f64 simulation for speed vs accuracy tradeoff

## Installation

```bash
cargo add ruqu-core
```

With optional features:

```bash
cargo add ruqu-core --features parallel,simd
```

## Quick Start

```rust
use ruqu_core::prelude::*;

// Create a Bell state |00> + |11>
let mut circuit = QuantumCircuit::new(2);
circuit.h(0).cnot(0, 1);

let result = Simulator::run(&circuit)?;
let probs = result.state.probabilities();
// probs ~= [0.5, 0.0, 0.0, 0.5]
```

## Simulation Backends

| Backend | Qubits | Best For |
|---------|--------|----------|
| **StateVector** | Up to 32 | Exact simulation, small circuits |
| **Stabilizer** | Millions | Clifford-only circuits (Gottesman-Knill) |
| **Clifford+T** | Moderate | Circuits with low T-count |
| **TensorNetwork** | Variable | Shallow/structured circuits (MPS) |
| **Hardware** | Device-dependent | Real device profiles and constraints |

The cost-model planner automatically selects the best backend:

```rust
use ruqu_core::planner::CostModelPlanner;

let planner = CostModelPlanner::new();
let backend = planner.select(&circuit); // Auto-routes to optimal backend
```

## OpenQASM 3.0 Export

```rust
use ruqu_core::qasm::to_qasm3;

let qasm = to_qasm3(&circuit);
println!("{}", qasm);
// OPENQASM 3.0;
// qubit[2] q;
// h q[0];
// cx q[0], q[1];
```

## Quantum Gates

| Gate | Description | Matrix |
|------|-------------|--------|
| `H` | Hadamard | Creates superposition |
| `X` | Pauli-X (NOT) | Bit flip |
| `Y` | Pauli-Y | Bit + phase flip |
| `Z` | Pauli-Z | Phase flip |
| `CNOT` | Controlled-NOT | Two-qubit entanglement |
| `CZ` | Controlled-Z | Controlled phase |
| `Rx(θ)` | X-rotation | Rotate around X-axis |
| `Ry(θ)` | Y-rotation | Rotate around Y-axis |
| `Rz(θ)` | Z-rotation | Rotate around Z-axis |
| `SWAP` | Swap qubits | Exchange qubit states |
| `Toffoli` | CCX | Three-qubit AND gate |

## Performance

Benchmarks on Apple M2 (single-threaded):

| Qubits | Gates | Time |
|--------|-------|------|
| 10 | 100 | 0.3ms |
| 15 | 100 | 8ms |
| 20 | 100 | 250ms |
| 25 | 100 | 8s |

With `--features parallel` on 8 cores, 20+ qubits see 3-5x speedup.

## Noise Simulation

```rust
use ruqu_core::noise::{NoiseModel, Depolarizing};

let noise = NoiseModel::new()
    .add_single_qubit(Depolarizing::new(0.01))  // 1% error rate
    .add_two_qubit(Depolarizing::new(0.02));    // 2% for CNOT

let noisy_state = simulator.run_noisy(&circuit, &noise)?;
```

## Related Crates

- [`ruqu-algorithms`](https://crates.io/crates/ruqu-algorithms) — VQE, Grover, QAOA, Surface Code
- [`ruqu-exotic`](https://crates.io/crates/ruqu-exotic) — Quantum-classical hybrid algorithms
- [`ruqu-wasm`](https://crates.io/crates/ruqu-wasm) — WebAssembly bindings

## Architecture

Part of the [RuVector](https://github.com/ruvnet/ruvector) quantum ecosystem. See [ADR-QE-001](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-001-quantum-engine-core-architecture.md) for core architecture and [ADR-QE-015](https://github.com/ruvnet/ruvector/blob/main/docs/adr/quantum-engine/ADR-QE-015-quantum-execution-intelligence.md) for the execution intelligence engine design.

## License

MIT OR Apache-2.0
