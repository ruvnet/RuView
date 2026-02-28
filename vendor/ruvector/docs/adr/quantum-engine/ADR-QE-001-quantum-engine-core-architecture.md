# ADR-QE-001: Quantum Engine Core Architecture

**Status**: Proposed
**Date**: 2026-02-06
**Authors**: ruv.io, RuVector Team
**Deciders**: Architecture Review Board

## Context

### Problem Statement

ruVector needs a quantum simulation engine for on-device quantum algorithm
experimentation. The platform runs on distributed edge systems, primarily
targeting Cognitum's 256-core low-power processors, and emphasizes ultra-low-power
event-driven computing. Quantum simulation is a natural extension of ruVector's
mathematical computation capabilities: the same SIMD-optimized linear algebra
that powers vector search and neural inference can drive state-vector manipulation
for quantum circuits.

### Requirements

The engine must support gate-model quantum circuit simulation up to approximately
25 qubits, covering the following algorithm families:

| Algorithm Family | Use Case | Typical Qubits | Gate Depth |
|------------------|----------|-----------------|------------|
| VQE (Variational Quantum Eigensolver) | Molecular simulation, optimization | 8-20 | 50-500 per iteration |
| Grover's Search | Unstructured database search | 8-25 | O(sqrt(2^n)) |
| QAOA (Quantum Approximate Optimization) | Combinatorial optimization | 10-25 | O(p * edges) |
| Quantum Error Correction | Surface code, stabilizer circuits | 9-25 (logical + ancilla) | Repetitive syndrome rounds |

### Memory Scaling Analysis

Quantum state-vector simulation stores the full amplitude vector of 2^n complex
numbers. Each amplitude is a pair of f64 values (real + imaginary = 16 bytes).
Memory grows exponentially:

```
Qubits  Amplitudes       State Size     With Scratch Buffer
------  -----------      ----------     -------------------
10      1,024            16 KB          32 KB
15      32,768           512 KB         1 MB
20      1,048,576        16 MB          32 MB
22      4,194,304        64 MB          128 MB
24      16,777,216       256 MB         512 MB
25      33,554,432       512 MB         1.07 GB
26      67,108,864       1.07 GB        2.14 GB
28      268,435,456      4.29 GB        8.59 GB
30      1,073,741,824    17.18 GB       34.36 GB
```

At 25 qubits the state vector requires approximately 512 MB (1.07 GB with a
scratch buffer for intermediate calculations). This is the practical ceiling
for WebAssembly's 32-bit address space. Native execution with sufficient RAM
can push to 30+ qubits.

### Edge Computing Constraints

Cognitum's 256-core processors operate under strict power and memory budgets:

- **Power envelope**: Event-driven activation; cores idle at near-zero draw
- **Memory**: Shared pool, typically 2-8 GB per node
- **Interconnect**: Low-latency mesh between cores, suitable for parallel simulation
- **Workload model**: Burst computation triggered by agent events, not continuous

The quantum engine must respect this model: allocate state only when a simulation
is triggered, execute the circuit, return results, and immediately release all
memory.

## Decision

Implement a **pure Rust state-vector quantum simulator** as a new crate family
(`ruQu` quantum engine) within the ruVector workspace. The following architectural
decisions define the engine.

### 1. Pure Rust Implementation (No C/C++ FFI)

The entire simulation engine is written in Rust with no foreign function interface
dependencies. This ensures:

- Compilation to `wasm32-unknown-unknown` without emscripten or C toolchains
- Memory safety guarantees throughout the simulation pipeline
- Unified build system via Cargo across all targets
- No external library version conflicts or platform-specific linking issues

### 2. State-Vector Simulation as Primary Backend

The engine uses explicit full-amplitude state-vector representation as its
primary simulation mode. Each gate application transforms the full 2^n
amplitude vector via matrix-vector multiplication.

```
Circuit Execution Model:

  |psi_0> ──[H]──[CNOT]──[Rz(theta)]──[Measure]── classical bits
     |          |            |              |
     v          v            v              v
  [init]    [apply_H]   [apply_CNOT]   [apply_Rz]   [sample]
     |          |            |              |           |
  2^n f64   2^n f64      2^n f64        2^n f64     collapse
  complex   complex      complex        complex     to basis
```

Gate application follows the standard decomposition:

- **Single-qubit gates**: Iterate amplitude pairs (i, i XOR 2^target), apply 2x2
  unitary. O(2^n) operations per gate.
- **Two-qubit gates**: Iterate amplitude quadruples, apply 4x4 unitary.
  O(2^n) operations per gate.
- **Multi-qubit gates**: Decompose into single and two-qubit gates, or apply
  directly via 2^k x 2^k matrix on k target qubits.

### 3. Qubit Limits and Precision

| Parameter | WASM Target | Native Target |
|-----------|-------------|---------------|
| Max qubits (default) | 25 | 30+ (RAM-dependent) |
| Max qubits (hard limit) | 26 (with f32) | Memory-limited |
| Precision (default) | Complex f64 | Complex f64 |
| Precision (optional) | Complex f32 | Complex f32 |
| State size at max | ~1.07 GB | ~17 GB at 30 qubits |

Complex f64 is the default precision, providing approximately 15 decimal digits
of accuracy -- sufficient for quantum chemistry applications and deep circuits
where accumulated floating-point error matters. An optional f32 mode halves
memory usage at the cost of precision, suitable for shallow circuits and
approximate optimization.

### 4. Event-Driven Activation Model

The engine follows ruVector's event-driven philosophy:

```
Agent Context          ruQu Engine              Memory
     |                      |                      |
     |-- trigger(circuit) ->|                      |
     |                      |-- allocate(2^n) ---->|
     |                      |<---- state_ptr ------|
     |                      |                      |
     |                      |-- [execute gates] -->|
     |                      |-- [measure] -------->|
     |                      |                      |
     |<-- results ---------|                      |
     |                      |-- deallocate() ----->|
     |                      |                      |
   (idle)                (inert)               (freed)
```

- **Inert by default**: No background threads, no persistent allocations
- **Allocate on demand**: State vector created when circuit execution begins
- **Free immediately**: All simulation memory released upon result delivery
- **No global state**: Multiple concurrent simulations supported via independent
  state handles (no shared mutable global)

### 5. Dual-Target Compilation

The crate supports two compilation targets from a single codebase:

```
                    ruqu-core
                       |
            +----------+----------+
            |                     |
    [native target]       [wasm32-unknown-unknown]
            |                     |
    - Full SIMD (AVX2,      - WASM SIMD128
      AVX-512, NEON)        - 4GB address limit
    - Rayon threading        - Optional SharedArrayBuffer
    - Optional GPU (wgpu)    - No GPU
    - 30+ qubits             - 25 qubit ceiling
    - Full OS integration    - Sandboxed
```

Conditional compilation via Cargo feature flags controls target-specific code
paths. The public API surface is identical across targets.

### 6. Optional Tensor Network Mode

For circuits with limited entanglement (e.g., shallow QAOA, certain VQE
ansatze), the engine offers an optional tensor network backend:

- Represents the quantum state as a network of tensors rather than a single
  exponential vector
- Memory scales as O(n * chi^2) where chi is the bond dimension (maximum
  entanglement width)
- Efficient for circuits where entanglement grows slowly or remains bounded
- Falls back to full state-vector when bond dimension exceeds threshold
- Enabled via the `tensor-network` feature flag

## Alternatives Considered

### Alternative 1: Qukit (Rust, WASM-ready)

A pre-1.0 Rust quantum simulator with WASM support.

| Criterion | Assessment |
|-----------|------------|
| Maturity | Pre-1.0, limited community |
| WASM support | Present but untested at scale |
| Optimization | Basic; no SIMD, no gate fusion |
| Integration | Would require adapter layer |
| Maintenance | External dependency risk |

**Rejected**: Insufficient optimization depth and maturity for production use.

### Alternative 2: QuantRS2 (Rust, Python-focused)

A Rust quantum simulator primarily targeting Python bindings via PyO3.

| Criterion | Assessment |
|-----------|------------|
| Performance | Good benchmarks on native |
| WASM support | Not a design target |
| Dependencies | Heavy; Python-oriented build |
| API design | Python-first, Rust API secondary |
| Integration | Significant impedance mismatch |

**Rejected**: Python-centric design creates unnecessary weight and integration
friction for a Rust-native edge system.

### Alternative 3: roqoqo + QuEST (Rust frontend, C backend)

roqoqo provides a Rust circuit description layer; QuEST is a high-performance
C/C++ state-vector simulator.

| Criterion | Assessment |
|-----------|------------|
| Performance | Excellent (QuEST is highly optimized) |
| WASM support | QuEST's C code breaks WASM compilation |
| Maintenance | External C library maintenance burden |
| Memory safety | C backend outside Rust safety guarantees |

**Rejected**: C dependency is incompatible with WASM target requirement.

### Alternative 4: Quant-Iron (Rust + OpenCL)

A Rust simulator leveraging OpenCL for GPU acceleration.

| Criterion | Assessment |
|-----------|------------|
| Performance | Excellent on GPU-equipped hardware |
| WASM support | OpenCL incompatible with WASM |
| Edge deployment | Most edge nodes lack discrete GPUs |
| Complexity | OpenCL runtime adds operational burden |

**Rejected**: OpenCL dependency incompatible with WASM and edge deployment model.

### Alternative 5: No Simulator (Cloud Quantum APIs)

Delegate all quantum computation to cloud-based quantum simulators or hardware.

| Criterion | Assessment |
|-----------|------------|
| Performance | Network-bound latency |
| Offline support | None; requires connectivity |
| Cost | Per-execution charges |
| Privacy | Circuit data sent to third party |
| Edge philosophy | Violates offline-first design |

**Rejected**: Fundamentally incompatible with ruVector's offline-first edge
computing philosophy.

## Consequences

### Positive

- **Full control**: Complete ownership of the simulation pipeline, enabling
  deep integration with ruVector's math, SIMD, and memory subsystems
- **WASM portable**: Single codebase compiles to any WASM runtime, enabling
  browser-based quantum experimentation
- **No external dependencies**: Eliminates supply chain risk from C/C++ or
  Python library dependencies
- **Edge-aligned**: Event-driven activation model matches Cognitum's power
  architecture
- **Extensible**: Gate set, noise models, and backends can evolve independently

### Negative

- **Development effort**: Building a competitive quantum simulator from scratch
  requires significant engineering investment
- **Maintenance burden**: Team must benchmark, optimize, and maintain the
  simulation engine alongside the rest of ruVector
- **Classical simulation limits**: Exponential scaling is a fundamental physics
  constraint; the engine cannot exceed ~30 qubits on practical hardware

### Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Performance below competitors | Medium | High | Benchmark-driven development against QuantRS2/Qukit |
| Floating-point accuracy drift | Low | Medium | Comprehensive numerical tests, optional f64 enforcement |
| WASM memory exhaustion | Medium | Medium | Hard qubit limit with clear error messages (ADR-QE-003) |
| Scope creep into hardware simulation | Low | Low | Strict scope: gate-model only, no analog/pulse simulation |

## References

- [ADR-005: WASM Runtime Integration](/docs/adr/ADR-005-wasm-runtime-integration.md)
- [ADR-003: SIMD Optimization Strategy](/docs/adr/ADR-003-simd-optimization-strategy.md)
- [ADR-006: Memory Management](/docs/adr/ADR-006-memory-management.md)
- [ADR-014: Coherence Engine](/docs/adr/ADR-014-coherence-engine.md)
- [ADR-QE-002: Crate Structure & Integration](./ADR-QE-002-crate-structure-integration.md)
- [ADR-QE-003: WASM Compilation Strategy](./ADR-QE-003-wasm-compilation-strategy.md)
- [ADR-QE-004: Performance Optimization & Benchmarks](./ADR-QE-004-performance-optimization-benchmarks.md)
- Nielsen & Chuang, "Quantum Computation and Quantum Information" (2010)
- Aaronson & Gottesman, "Improved simulation of stabilizer circuits" (2004)
