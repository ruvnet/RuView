# Quantum Simulation Engine: Domain-Driven Design - Strategic Design

**Version**: 0.1
**Date**: 2026-02-06
**Status**: Draft

---

## Domain Vision

The Quantum Simulation Engine provides **on-device quantum algorithm experimentation** within ruVector's always-on, agentic environment. It enables hybrid classical-quantum research on edge devices, allowing agents to leverage quantum algorithms (VQE, Grover, QAOA, QEC) without cloud services.

> **This is not a cloud quantum API.** The engine answers: "What does this quantum circuit produce?" entirely on the local device, using classical state-vector simulation with SIMD acceleration.

The engine follows ruVector's event-driven model: **inert when idle, activated on demand, resources released immediately**. A 20-qubit simulation allocates 16 MiB of state vector on activation and frees it the moment the circuit completes. No background threads, no persistent memory, no warm pools.

### The Universal Simulation Object

The power lies in a **single underlying state-vector engine** inside ruqu-sim. Once the linear algebra is fixed, everything else becomes interpretation:

| Domain | Qubits Become | Gates Become | Measurement Becomes | Circuit Becomes |
|--------|---------------|--------------|---------------------|-----------------|
| **Chemistry** | Molecular orbitals | Fermionic operators | Energy estimates | VQE ansatz |
| **Optimization** | Decision variables | Mixing/cost ops | Cut values | QAOA circuit |
| **Search** | Database indices | Oracle + diffusion | Found element | Grover iterations |
| **Error Correction** | Data + ancilla qubits | Stabilizer checks | Syndrome bits | QEC cycle |
| **Cryptography** | Key register bits | Quantum Fourier transform | Period estimate | Shor subroutine |
| **Machine Learning** | Feature dimensions | Parameterized rotations | Classification | Quantum kernel |

**Same linear algebra, different interpretations. Same state vector = superposition. Same measurement = probabilistic collapse with Born rule.**

---

## Strategic Design

### Core Domain

**Quantum State Simulation** - The heart of the system, managing quantum state vectors, applying unitary gate operations, and performing projective measurements. This is where the primary complexity and innovation reside. **Most circuits run in a single fast pass; only large entangled states or iterative variational loops require sustained computation.**

### Supporting Domains

1. **Circuit Construction** - Building, validating, and optimizing quantum circuits
2. **State Management** - State vector lifecycle, entanglement tracking, memory gating
3. **Measurement & Observation** - Projective measurement, expectation values, syndrome extraction
4. **Algorithm Execution** - High-level quantum algorithm implementations (VQE, Grover, QAOA, QEC)
5. **Optimization & Backend** - SIMD acceleration, gate fusion, tensor network backends
6. **Deployment & Integration** - WASM compilation, agent bridge, coherence bridge to ruQu

### Generic Domains

1. **Linear Algebra** - Complex number math, matrix-vector products, Kronecker products (via `ruvector-math`)
2. **Random Sampling** - Measurement outcome sampling, noise injection (via `rand` crate)
3. **Logging/Tracing** - Event recording, performance metrics (via `tracing` crate + `ruvector-metrics`)

### Application Evolution

| Timeline | Capabilities | Key Value |
|----------|-------------|-----------|
| **Phase 1 (Now)** | State vector sim, basic gates, VQE/Grover/QAOA | Local quantum experimentation without cloud |
| **Phase 2 (6mo)** | Tensor networks, noise models, surface code cycles | Error correction research on edge devices |
| **Phase 3 (12mo)** | GPU acceleration, OpenQASM 3.0 import, 30+ qubits | Production-grade quantum algorithm research |
| **Phase 4 (24mo)** | Quantum hardware bridge, hybrid cloud-local execution | Real quantum device integration |

> **Edge-First Quantum**: The system eventually enables agents to reason about quantum algorithms without any network dependency.

---

## Ecosystem Integration Map

```
+---------------------------------------------------------------------------+
|                        QUANTUM SIMULATION ENGINE                          |
|                                                                           |
|  +-------------------------------------------------------------------+   |
|  |                  CIRCUIT CONSTRUCTION DOMAIN                       |   |
|  |  QuantumCircuit | Gate | GateSchedule | CircuitOptimizer          |   |
|  |  Parameterized templates (VQE ansatz, QAOA mixer, Grover oracle)  |   |
|  +-------------------------------------------------------------------+   |
|                              |                                            |
|                              v                                            |
|  +-----------------------------+  +-----------------------------+        |
|  | CORE: QUANTUM STATE         |  | STATE MANAGEMENT            |        |
|  | SIMULATION                  |<-| DOMAIN                      |        |
|  |                             |  |                             |        |
|  | * State vector engine       |  | * Allocation / deallocation |        |
|  | * Gate application (SIMD)   |  | * Entanglement tracking     |        |
|  | * Unitary evolution         |  | * Memory gating (zero-idle) |        |
|  | * Tensor contraction        |  | * State checkpointing       |        |
|  +-----------------------------+  +-----------------------------+        |
|           |            |                      |                           |
|           v            v                      v                           |
|  +-----------------------------+  +-----------------------------+        |
|  | MEASUREMENT &               |  | ALGORITHM EXECUTION         |        |
|  | OBSERVATION DOMAIN          |  | DOMAIN                      |        |
|  |                             |  |                             |        |
|  | * Projective measurement    |  | * VQE + classical optimizer |        |
|  | * Expectation values        |  | * Grover auto-iteration     |        |
|  | * Shot-based sampling       |  | * QAOA graph-based circuits |        |
|  | * Syndrome extraction       |  | * Surface code + decoder    |        |
|  +-----------------------------+  +-----------------------------+        |
|                                            |                              |
|                                            v                              |
|  +-----------------------------+  +-----------------------------+        |
|  | OPTIMIZATION &              |  | DEPLOYMENT &                |        |
|  | BACKEND DOMAIN              |  | INTEGRATION DOMAIN          |        |
|  |                             |  |                             |        |
|  | * SIMD dispatch             |  | * WASM bindings (ruqu-wasm) |        |
|  | * Gate fusion               |  | * Agent bridge (activation) |        |
|  | * Tensor network backend    |  | * Observability / metrics   |        |
|  | * Cache-local strategies    |  | * Coherence bridge (ruQu)   |        |
|  +-----------------------------+  +-----------------------------+        |
|                                                                           |
+---------------------------------------------------------------------------+
                              |
         +--------------------+---------------------+
         |                    |                      |
         v                    v                      v
  +--------------+   +-----------------+   +------------------+
  | ruvector-    |   | ruvector-       |   | ruQu             |
  | math (SIMD)  |   | metrics         |   | (decoder bridge) |
  +--------------+   +-----------------+   +------------------+
         |                                          |
         v                                          v
  +--------------+   +-----------------+   +------------------+
  | ruvector-    |   | ruvector-       |   | cognitum-gate-   |
  | graph        |   | nervous-system  |   | kernel (tiles)   |
  +--------------+   +-----------------+   +------------------+
         |                    |
         v                    v
  +--------------+   +-----------------+
  | ruvector-    |   | sona (adaptive  |
  | mincut       |   |  learning)      |
  +--------------+   +-----------------+
```

### Crate-to-Context Mapping

| Bounded Context | Primary Crate | Supporting Crates |
|-----------------|---------------|-------------------|
| Circuit Construction | `ruqu-sim` (new) | - |
| Quantum State Simulation (Core) | `ruqu-sim` (new) | `ruvector-math` |
| State Management | `ruqu-sim` (new) | - |
| Measurement & Observation | `ruqu-sim` (new) | `rand` |
| Algorithm Execution | `ruqu-sim` (new) | `ruvector-graph` (QAOA) |
| Optimization & Backend | `ruqu-sim` (new) | `ruvector-math` (SIMD) |
| Deployment & Integration | `ruqu-wasm` (new) | `ruqu`, `ruvector-metrics`, `ruvector-nervous-system` |

---

## Context Map

```
+-----------------------------------------------------------------------+
|                     QUANTUM ENGINE CONTEXT MAP                         |
|                                                                        |
|                     [Published Language]                                |
|                     OpenQASM 3.0 format                                |
|                            |                                           |
|                            v                                           |
|   +------------------+         +------------------+                    |
|   |                  | Shared  |                  |                    |
|   |  CIRCUIT         | Kernel  |  STATE           |                    |
|   |  CONSTRUCTION    |<------->|  MANAGEMENT      |                    |
|   |                  | (Gate,  |                  |                    |
|   |  Builds circuits | QubitIdx|  Allocates and   |                    |
|   |  Validates gates |  types) |  tracks state    |                    |
|   +--------+---------+         +--------+---------+                    |
|            |                            |                              |
|            | Customer                   | Customer                     |
|            | Supplier                   | Supplier                     |
|            v                            v                              |
|   +------------------+         +------------------+                    |
|   |                  |         |                  |                    |
|   |  MEASUREMENT &   |-------->|  ALGORITHM       |                    |
|   |  OBSERVATION     |Supplier |  EXECUTION       |                    |
|   |                  |Customer |                  |                    |
|   |  Measures states |         |  Runs VQE/QAOA/  |                    |
|   |  Extracts syndr. |         |  Grover/QEC      |                    |
|   +--------+---------+         +--------+---------+                    |
|            |                            |                              |
|            +------------+---------------+                              |
|                         |                                              |
|                         v                                              |
|            +------------------+         +------------------+           |
|            |                  |         |                  |           |
|            |  OPTIMIZATION &  |         |  DEPLOYMENT &    |           |
|            |  BACKEND         |         |  INTEGRATION     |           |
|            |                  |         |                  |           |
|            |  SIMD, fusion,   |         |  WASM, agents,   |           |
|            |  tensor networks |         |  ruQu bridge     |           |
|            +------------------+         +--------+---------+           |
|                                                  |                     |
|                                    Conformist    | Anti-Corruption     |
|                                    (ruVector     | Layer               |
|                                     APIs)        | (ruQu decoder)     |
|                                                  |                     |
+--------------------------------------------------+---------------------+
                                                   |
                                                   v
                                     [Existing ruVector Ecosystem]

Context Relationships:
  <-------> Shared Kernel (shared types across boundary)
  -------> Customer-Supplier (downstream depends on upstream)
  Conformist: Deployment conforms to existing ruVector APIs
  ACL: CoherenceBridge wraps ruQu decoder behind anti-corruption layer
  Published Language: OpenQASM 3.0 for circuit interchange
  Open Host Service: ruqu-wasm exposes JS API
```

### Relationship Summary

| Upstream | Downstream | Pattern | Shared Types |
|----------|------------|---------|-------------|
| Circuit Construction | State Management | **Shared Kernel** | `Gate`, `QubitIndex`, `GateMatrix` |
| Measurement & Observation | Algorithm Execution | **Customer-Supplier** | `MeasurementOutcome`, `ExpectationValue` |
| State Management | Algorithm Execution | **Customer-Supplier** | `QuantumState`, `StateCheckpoint` |
| State Management | Measurement & Observation | **Customer-Supplier** | `QuantumState`, `Amplitude` |
| Optimization & Backend | Core Simulation | **Partnership** | `FusedGateMatrix`, `OptimizationHint` |
| Existing ruVector APIs | Deployment & Integration | **Conformist** | ruVector event types, metric types |
| ruQu decoder API | Deployment & Integration | **Anti-Corruption Layer** | Isolated behind `CoherenceBridge` |
| Circuit Construction | External tools | **Published Language** | OpenQASM 3.0 circuit format |
| Deployment & Integration | JS consumers | **Open Host Service** | `ruqu-wasm` JS API |

---

## Ubiquitous Language

### Quantum Fundamentals

| Term | Definition |
|------|------------|
| **Qubit** | Fundamental unit of quantum information existing in superposition of |0> and |1> basis states |
| **Amplitude** | Complex number representing probability amplitude of a basis state; measurement probability is its squared modulus |
| **State Vector** | Array of 2^n complex amplitudes representing the full quantum state of an n-qubit register |
| **Basis State** | One of 2^n classical bit-string configurations; each has an associated amplitude |
| **Superposition** | State where multiple basis states have nonzero amplitude |
| **Entanglement** | Quantum correlation preventing independent per-qubit factorization of the joint state |
| **Born Rule** | Measurement probability equals squared modulus of amplitude: P(x) = |alpha_x|^2 |

### Circuit Model

| Term | Definition |
|------|------------|
| **Gate** | Unitary matrix operation acting on 1 or 2 qubits; transforms state via matrix-vector multiply |
| **Circuit** | Ordered sequence of gates applied to a qubit register; the program of a quantum computation |
| **Gate Matrix** | Unitary matrix defining gate action; must satisfy U * U_dagger = I |
| **Qubit Index** | Zero-based integer identifying a qubit; determines which amplitude pairs a gate addresses |
| **Circuit Depth** | Maximum sequential gate layers; primary determinant of simulation time |
| **Parameterized Gate** | Gate whose matrix depends on continuous real parameters (e.g., Ry(theta)) |
| **Gate Fusion** | Combining adjacent gates on same qubits into a single matrix multiply |
| **Gate Schedule** | Topologically sorted gate-to-timestep assignment respecting qubit-sharing constraints |

### Measurement & Algorithms

| Term | Definition |
|------|------------|
| **Measurement** | Projective observation collapsing superposition to a basis state per the Born rule |
| **Mid-Circuit Measurement** | Measurement during (not only at end of) circuit execution |
| **Shot** | Single circuit execution + measurement; repeated shots build statistics |
| **Expectation Value** | Observable average over quantum state: <psi|H|psi> |
| **Pauli String** | Tensor product of per-qubit Pauli operators (I/X/Y/Z) with coefficient |
| **Hamiltonian** | Hermitian operator (weighted sum of Pauli strings) representing total energy |
| **Syndrome** | Classical bits from ancilla measurements indicating error presence and location |
| **Ansatz** | Parameterized circuit template encoding the variational search space |
| **VQE** | Variational Quantum Eigensolver; iteratively minimizes Hamiltonian expectation |
| **QAOA** | Quantum Approximate Optimization Algorithm; alternating cost/mixer unitaries |
| **Grover Search** | Amplitude amplification finding marked items in O(sqrt(N)) queries |
| **Oracle** | Black-box gate marking target states by phase flip |
| **Surface Code** | 2D topological QEC code with stabilizer checks on lattice faces/vertices |
| **Logical Error Rate** | Undetected logical error probability per QEC cycle |
| **Decoder** | Classical algorithm mapping syndromes to corrections; bridge to ruQu |

### Simulation Infrastructure

| Term | Definition |
|------|------------|
| **State Allocator** | On-demand allocation/deallocation enforcing zero-idle policy |
| **Memory Estimate** | Predicted bytes: 2^n * 16; gating threshold for allocation |
| **Entanglement Tracker** | Tracks qubit correlations enabling subsystem splitting |
| **State Checkpoint** | Serialized state snapshot for mid-circuit save/restore |
| **Tensor Network** | Alternative representation via contracted tensor factors; efficient for low entanglement |
| **Contraction Path** | Tensor contraction order minimizing total FLOPs |

---

## Bounded Context Details

### Context 1: Circuit Construction Domain

**Purpose**: Language for expressing quantum computations. Validation, scheduling, optimization, OpenQASM interchange.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **QuantumCircuit** | Aggregate Root | Ordered gate collection with register metadata |
| **Gate** | Entity | Single unitary with target qubits and optional parameters |
| **GateSchedule** | Entity | Time-step assignment for parallel execution analysis |
| **CircuitOptimizer** | Domain Service | Fusion, cancellation, and commutation rules |
| GateId, QubitIndex, GateMatrix, ParameterBinding, GateType | Value Objects | Immutable circuit building blocks |

**Events**: `CircuitCreated`, `GateAppended`, `CircuitOptimized`, `CircuitValidated`, `ParametersBound`

**Invariants**: (1) Gate unitarity. (2) Qubit indices within bounds. (3) No duplicate targets per gate. (4) All parameters bound before execution.

---

### Context 2: State Management Domain

**Purpose**: State vector lifecycle following zero-idle model. Entanglement tracking. Memory gating.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **QuantumState** | Aggregate Root | Owns the 2^n complex amplitude array |
| **EntanglementTracker** | Entity | Bipartite entanglement graph for subsystem analysis |
| **StateAllocator** | Domain Service | On-demand allocation, immediate deallocation |
| Amplitude, QubitCount, MemoryEstimate, StateCheckpoint | Value Objects | State representation primitives |

**Events**: `StateAllocated`, `StateDeallocated`, `EntanglementDetected`, `SubsystemSplit`, `CheckpointCreated`, `MemoryLimitExceeded`

**Invariants**: (1) Normalization preserved. (2) Zero-idle: no state persists beyond execution. (3) Allocation gated by device capacity. (4) Checkpoint restore reproduces exact amplitudes.

---

### Context 3: Measurement & Observation Domain

**Purpose**: Projective measurement with collapse. Analytical expectation values. Syndrome extraction for QEC.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **MeasurementEngine** | Aggregate Root | Born-rule sampling and state collapse |
| **ExpectationCalculator** | Entity | Analytical <psi|H|psi> from Pauli decomposition |
| **SyndromeExtractor** | Entity | Ancilla measurement and classical bit extraction |
| MeasurementOutcome, PauliString, Hamiltonian, SyndromeBits, ShotResult | Value Objects | Measurement data types |

**Events**: `MeasurementPerformed`, `ExpectationComputed`, `SyndromeExtracted`, `ShotsCompleted`

**Invariants**: (1) Born rule: probabilities sum to 1.0. (2) Post-measurement collapse to definite state. (3) Hamiltonian Hermiticity. (4) Syndrome bit count matches code.

---

### Context 4: Algorithm Execution Domain

**Purpose**: High-level quantum algorithms as orchestrated loops over circuits, states, and measurements.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **VQERunner** | Entity | Iterative ansatz parameter optimization to minimize energy |
| **GroverSearch** | Entity | Oracle + diffusion with auto-computed iteration count |
| **QAOASolver** | Entity | Graph-based cost/mixer circuit construction and angle optimization |
| **SurfaceCodeSimulator** | Entity | Stabilizer cycles, syndrome extraction, decoder invocation |
| AlgorithmResult, OptimizationTrace, CutValue, LogicalErrorRate, ConvergenceCriteria | Value Objects | Algorithm output types |

**Events**: `VQEIterationCompleted`, `VQEConverged`, `GroverSearchCompleted`, `QAOARoundCompleted`, `SurfaceCodeCycleCompleted`, `LogicalErrorDetected`

**Invariants**: (1) Grover iteration count = floor(pi/4 * sqrt(N/M)). (2) VQE energy is upper bound on ground state. (3) QAOA cost/mixer alternate with correct parameter count. (4) Surface code distance matches lattice.

---

### Context 5: Optimization & Backend Domain

**Purpose**: Performance backends that accelerate simulation without altering semantics. SIMD, fusion, tensor networks.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **SimulationBackend** | Aggregate Root | Selects optimal execution strategy |
| **GateFuser** | Entity | Combines compatible gate sequences into single operations |
| **TensorContractor** | Entity | Tensor network decomposition for low-entanglement states |
| **SIMDDispatcher** | Entity | Platform detection and optimized kernel dispatch |
| OptimizationHint, ContractionPath, FusedGateMatrix, PlatformCapabilities | Value Objects | Backend selection metadata |

**Events**: `BackendSelected`, `GatesFused`, `TensorNetworkContracted`, `SIMDKernelDispatched`

**Invariants**: (1) Fused gates produce identical results to sequential. (2) Tensor contraction matches state-vector. (3) SIMD falls back to scalar if unavailable. (4) Intermediates stay within memory budget.

---

### Context 6: Deployment & Integration Domain

**Purpose**: WASM compilation, agent activation bridge, ruQu decoder anti-corruption layer, observability.

| Entity / Value Object | Type | Responsibility |
|----------------------|------|---------------|
| **WASMBindings** | Entity | Open Host Service via wasm-bindgen JS API |
| **AgentBridge** | Entity | ruvector-nervous-system integration for context-triggered activation |
| **MetricsReporter** | Entity | Publishes SimulationMetrics to ruvector-metrics |
| **CoherenceBridge** | Entity | ACL translating syndromes to ruQu's DetectorBitmap/SyndromeRound |
| PlatformCapabilities, QubitLimit, SimulationMetrics, DecoderResult | Value Objects | Integration data types |

**Events**: `SimulationRequested`, `SimulationCompleted`, `ResourcesReleased`, `DecoderInvoked`, `MetricsPublished`

**Integration Patterns**:
- **Anti-Corruption Layer**: CoherenceBridge isolates engine from ruQu's internal DDD model
- **Conformist**: Deployment conforms to existing ruVector event types and metric schemas
- **Open Host Service**: ruqu-wasm exposes clean JS/TS API for browser experimentation
- **Published Language**: OpenQASM 3.0 for circuit interchange with external tools

---

## Cross-Cutting Concerns

### Zero-Idle Resource Model

```
IDLE (0 bytes) --> ACTIVATE (allocate 2^n * 16 bytes) --> COMPUTE --> RELEASE (0 bytes)
```

No warm pools, no pre-allocated buffers, no background threads.

### Memory Gating

| Qubits | State Vector Size | Decision |
|--------|-------------------|----------|
| 10 | 16 KiB | Always permit |
| 15 | 512 KiB | Always permit |
| 20 | 16 MiB | Permit on most devices |
| 25 | 512 MiB | Gate: check available RAM |
| 30 | 16 GiB | Gate: likely refuse on edge |
| 35+ | 512 GiB+ | Always refuse (state vector); consider tensor network |

### Error Model

| Context | Error | Severity | Recovery |
|---------|-------|----------|----------|
| Circuit Construction | Non-unitary gate | Fatal | Reject circuit |
| State Management | Memory limit exceeded | Recoverable | Try tensor network or refuse |
| State Management | Normalization drift | Warning | Renormalize |
| Measurement | Zero-probability outcome | Warning | Return uniform |
| Algorithm Execution | VQE non-convergence | Recoverable | Return best-so-far |
| Deployment | WASM memory limit | Fatal | Report to agent |
| Deployment | ruQu decoder unavailable | Recoverable | Skip correction, log |

### Observability

All simulation runs produce `SimulationMetrics` (circuit name, qubit count, gate count, depth, shots, backend type, wall time, peak memory, SIMD utilization) flowing through `ruvector-metrics` for unified dashboard integration.

### Security

| Concern | Mitigation |
|---------|------------|
| Timing side channels in measurement | Constant-time sampling via rejection method |
| Memory contents after deallocation | Zero-fill on deallocation (SecureAllocator mode) |
| Denial-of-service via large qubit counts | Memory gating with hard upper bound per request |
| Untrusted OpenQASM input | Parser validates unitarity and qubit bounds before execution |
| WASM sandbox escape | No file I/O, no network; pure computation within WASM sandbox |

---

## Module Structure

```
crates/ruqu-sim/src/
+-- lib.rs                     # Public API
+-- circuit/                   # Circuit Construction context
|   +-- quantum_circuit.rs     # QuantumCircuit aggregate
|   +-- gate.rs                # Gate entity, GateType enum
|   +-- schedule.rs            # GateSchedule
|   +-- optimizer.rs           # CircuitOptimizer (fusion, cancel)
|   +-- openqasm.rs            # OpenQASM 3.0 import/export
+-- state/                     # State Management context
|   +-- quantum_state.rs       # QuantumState aggregate
|   +-- allocator.rs           # StateAllocator (zero-idle)
|   +-- entanglement.rs        # EntanglementTracker
|   +-- checkpoint.rs          # StateCheckpoint
+-- measurement/               # Measurement & Observation context
|   +-- engine.rs              # MeasurementEngine
|   +-- expectation.rs         # ExpectationCalculator
|   +-- syndrome.rs            # SyndromeExtractor
+-- algorithms/                # Algorithm Execution context
|   +-- vqe.rs, grover.rs      # VQERunner, GroverSearch
|   +-- qaoa.rs                # QAOASolver
|   +-- surface_code.rs        # SurfaceCodeSimulator
+-- backend/                   # Optimization & Backend context
|   +-- simulation_backend.rs  # SimulationBackend
|   +-- gate_fuser.rs          # GateFuser
|   +-- tensor_network.rs      # TensorContractor
|   +-- simd_dispatch.rs       # SIMDDispatcher
|   +-- kernels/               # avx2.rs, avx512.rs, neon.rs, wasm_simd.rs, scalar.rs
+-- types.rs, events.rs, error.rs

crates/ruqu-wasm/src/
+-- lib.rs                     # wasm-bindgen entry
+-- js_api.rs                  # JS-facing API
+-- agent_bridge.rs            # ruvector-nervous-system integration
+-- coherence_bridge.rs        # ACL for ruQu decoder
+-- metrics.rs                 # ruvector-metrics export
```

### Dependency Graph

```
ruqu-sim
+-- ruvector-math           (SIMD kernels, complex math)
+-- rand                    (measurement sampling)
+-- ruvector-graph          (QAOA graph input)

ruqu-wasm
+-- ruqu-sim                (core simulation)
+-- ruqu                    (coherence bridge ACL)
+-- ruvector-metrics        (observability)
+-- ruvector-nervous-system (agent activation)
+-- wasm-bindgen            (JS bindings)
```

---

## Performance Targets

| Metric | Target |
|--------|--------|
| Single-gate (1q, 20-qubit register) | < 50 us |
| Full circuit (100 gates, 15 qubits) | < 10 ms |
| Hamiltonian expectation (10q, 50 terms) | < 1 ms |
| SIMD speedup over scalar | > 3x (AVX2), > 6x (AVX-512) |
| Grover (20 qubits, 1 target) | < 500 ms |
| VQE convergence (H2, 4 qubits) | < 5s, < 100 iterations |
| State allocation/deallocation | < 10 us / < 1 us |
| WASM circuit (10 qubits, 50 gates) | < 50 ms |

---

## References

1. Evans, E. (2003). "Domain-Driven Design: Tackling Complexity in the Heart of Software."
2. Vernon, V. (2013). "Implementing Domain-Driven Design."
3. Nielsen, M. A. & Chuang, I. L. (2010). "Quantum Computation and Quantum Information."
4. Peruzzo, A. et al. (2014). "A variational eigenvalue solver on a photonic quantum processor."
5. Farhi, E. et al. (2014). "A Quantum Approximate Optimization Algorithm."
6. Fowler, A. G. et al. (2012). "Surface codes: Towards practical large-scale quantum computation."
7. ruQu crate: Existing coherence assessment and syndrome processing in ruVector.
8. Coherence Engine DDD: `/docs/architecture/coherence-engine-ddd.md`
